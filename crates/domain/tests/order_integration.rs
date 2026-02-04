//! Integration tests for the Order aggregate.
//!
//! These tests verify the full order lifecycle including event persistence,
//! aggregate reconstruction, and concurrency handling.

use common::AggregateId;
use domain::{
    AddItem, Aggregate, CancelOrder, CompleteOrder, CreateOrder, CustomerId, DomainError,
    DomainEvent, MarkReserved, Money, OrderError, OrderEvent, OrderItem, OrderService, OrderState,
    ProductId, StartProcessing, SubmitOrder,
};
use event_store::{EventStore, EventStoreError, InMemoryEventStore, Version};

/// Helper to create a test order service
fn create_service() -> OrderService<InMemoryEventStore> {
    OrderService::new(InMemoryEventStore::new())
}

mod order_lifecycle {
    use super::*;

    #[tokio::test]
    async fn complete_order_lifecycle() {
        let service = create_service();

        // Create order
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;

        let result = service.create_order(cmd).await.unwrap();
        assert_eq!(result.aggregate.state(), OrderState::Draft);
        assert_eq!(result.new_version, Version::first());

        // Add multiple items
        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget A", 2, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        let result = service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-002", "Widget B", 1, Money::from_cents(500)),
            ))
            .await
            .unwrap();

        assert_eq!(result.aggregate.item_count(), 2);
        assert_eq!(result.aggregate.total_amount().cents(), 2500);
        assert_eq!(result.new_version, Version::new(3));

        // Submit order
        let result = service
            .submit_order(SubmitOrder::new(order_id))
            .await
            .unwrap();
        assert_eq!(result.aggregate.state(), OrderState::Draft);

        // Reserve inventory
        let result = service
            .mark_reserved(MarkReserved::new(order_id, Some("RES-123".to_string())))
            .await
            .unwrap();
        assert_eq!(result.aggregate.state(), OrderState::Reserved);

        // Start processing
        let result = service
            .start_processing(StartProcessing::new(order_id, Some("PAY-456".to_string())))
            .await
            .unwrap();
        assert_eq!(result.aggregate.state(), OrderState::Processing);

        // Complete order
        let result = service
            .complete_order(CompleteOrder::new(order_id, Some("TRACK-789".to_string())))
            .await
            .unwrap();

        assert_eq!(result.aggregate.state(), OrderState::Completed);
        assert!(result.aggregate.is_terminal());
    }

    #[tokio::test]
    async fn cancel_order_at_various_stages() {
        let service = create_service();

        // Test cancelling draft order
        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();
        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 1, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        let result = service
            .cancel_order(CancelOrder::new(order_id, "Customer changed mind", None))
            .await
            .unwrap();

        assert_eq!(result.aggregate.state(), OrderState::Cancelled);

        // Test cancelling reserved order
        let order_id2 = AggregateId::new();
        service
            .create_order(CreateOrder::new(order_id2, customer_id))
            .await
            .unwrap();

        service
            .add_item(AddItem::new(
                order_id2,
                OrderItem::new("SKU-001", "Widget", 1, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        service
            .submit_order(SubmitOrder::new(order_id2))
            .await
            .unwrap();

        service
            .mark_reserved(MarkReserved::new(order_id2, None))
            .await
            .unwrap();

        let result = service
            .cancel_order(CancelOrder::new(order_id2, "Out of stock", None))
            .await
            .unwrap();

        assert_eq!(result.aggregate.state(), OrderState::Cancelled);
    }

    #[tokio::test]
    async fn aggregate_reconstruction_from_events() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store.clone());

        // Create and modify order
        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 3, Money::from_cents(999)),
            ))
            .await
            .unwrap();

        service
            .submit_order(SubmitOrder::new(order_id))
            .await
            .unwrap();

        service
            .mark_reserved(MarkReserved::new(order_id, None))
            .await
            .unwrap();

        // Load and verify aggregate is correctly reconstructed
        let order = service.get_order(order_id).await.unwrap().unwrap();

        assert_eq!(order.id(), Some(order_id));
        assert_eq!(order.customer_id(), Some(customer_id));
        assert_eq!(order.state(), OrderState::Reserved);
        assert_eq!(order.item_count(), 1);
        assert_eq!(order.total_amount().cents(), 2997);

        let item = order.get_item(&ProductId::new("SKU-001")).unwrap();
        assert_eq!(item.quantity, 3);
        assert_eq!(item.unit_price.cents(), 999);
    }
}

mod concurrency {
    use super::*;
    use event_store::{AppendOptions, EventEnvelope};

    #[tokio::test]
    async fn concurrent_modifications_detected() {
        let store = InMemoryEventStore::new();

        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        // Create order
        let event = OrderEvent::order_created(order_id, customer_id);
        let envelope = EventEnvelope::builder()
            .aggregate_id(order_id)
            .aggregate_type("Order")
            .event_type(event.event_type())
            .version(Version::first())
            .payload(&event)
            .unwrap()
            .build();

        store
            .append(vec![envelope], AppendOptions::expect_new())
            .await
            .unwrap();

        // Simulate two concurrent writes both expecting version 1
        // First write succeeds
        let event1 = OrderEvent::item_added(&OrderItem::new(
            "SKU-001",
            "Widget",
            1,
            Money::from_cents(1000),
        ));
        let envelope1 = EventEnvelope::builder()
            .aggregate_id(order_id)
            .aggregate_type("Order")
            .event_type(event1.event_type())
            .version(Version::new(2))
            .payload(&event1)
            .unwrap()
            .build();

        store
            .append(
                vec![envelope1],
                AppendOptions::expect_version(Version::first()),
            )
            .await
            .unwrap();

        // Second write should fail - same expected version but data has changed
        let event2 = OrderEvent::item_added(&OrderItem::new(
            "SKU-002",
            "Gadget",
            1,
            Money::from_cents(500),
        ));
        let envelope2 = EventEnvelope::builder()
            .aggregate_id(order_id)
            .aggregate_type("Order")
            .event_type(event2.event_type())
            .version(Version::new(2))
            .payload(&event2)
            .unwrap()
            .build();

        let result = store
            .append(
                vec![envelope2],
                AppendOptions::expect_version(Version::first()),
            )
            .await;

        // Should fail due to concurrency conflict
        assert!(matches!(
            result,
            Err(EventStoreError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn retry_after_concurrency_conflict() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store);

        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        // Create order
        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        // First add succeeds
        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 1, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        // Second add also succeeds (no conflict since we reload)
        let result = service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-002", "Gadget", 1, Money::from_cents(500)),
            ))
            .await
            .unwrap();

        assert_eq!(result.aggregate.item_count(), 2);
        assert_eq!(result.aggregate.total_amount().cents(), 1500);
    }
}

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn cannot_add_item_after_order_reserved() {
        let service = create_service();

        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 1, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        service
            .submit_order(SubmitOrder::new(order_id))
            .await
            .unwrap();

        service
            .mark_reserved(MarkReserved::new(order_id, None))
            .await
            .unwrap();

        // Try to add item after reservation
        let result = service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-002", "Gadget", 1, Money::from_cents(500)),
            ))
            .await;

        assert!(matches!(
            result,
            Err(DomainError::Order(
                OrderError::InvalidStateTransition { .. }
            ))
        ));
    }

    #[tokio::test]
    async fn cannot_complete_non_processing_order() {
        let service = create_service();

        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 1, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        // Try to complete without going through proper flow
        let result = service
            .complete_order(CompleteOrder::new(order_id, None))
            .await;

        assert!(matches!(
            result,
            Err(DomainError::Order(
                OrderError::InvalidStateTransition { .. }
            ))
        ));
    }

    #[tokio::test]
    async fn cannot_cancel_completed_order() {
        let service = create_service();

        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        // Go through entire lifecycle
        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 1, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        service
            .submit_order(SubmitOrder::new(order_id))
            .await
            .unwrap();

        service
            .mark_reserved(MarkReserved::new(order_id, None))
            .await
            .unwrap();

        service
            .start_processing(StartProcessing::new(order_id, None))
            .await
            .unwrap();

        service
            .complete_order(CompleteOrder::new(order_id, None))
            .await
            .unwrap();

        // Try to cancel completed order
        let result = service
            .cancel_order(CancelOrder::new(order_id, "Too late", None))
            .await;

        assert!(matches!(
            result,
            Err(DomainError::Order(
                OrderError::InvalidStateTransition { .. }
            ))
        ));
    }

    #[tokio::test]
    async fn cannot_submit_empty_order() {
        let service = create_service();

        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        // Try to submit without items
        let result = service.submit_order(SubmitOrder::new(order_id)).await;

        assert!(matches!(
            result,
            Err(DomainError::Order(OrderError::NoItems))
        ));
    }
}

mod item_management {
    use super::*;
    use domain::UpdateItemQuantity;

    #[tokio::test]
    async fn adding_same_product_increases_quantity() {
        let service = create_service();

        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        // Add item twice
        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        let result = service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 3, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        // Should have combined quantity
        assert_eq!(result.aggregate.item_count(), 1);
        let item = result
            .aggregate
            .get_item(&ProductId::new("SKU-001"))
            .unwrap();
        assert_eq!(item.quantity, 5);
        assert_eq!(result.aggregate.total_amount().cents(), 5000);
    }

    #[tokio::test]
    async fn update_quantity_to_zero_removes_item() {
        let service = create_service();

        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 5, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        let result = service
            .update_item_quantity(UpdateItemQuantity::new(order_id, "SKU-001", 0))
            .await
            .unwrap();

        assert_eq!(result.aggregate.item_count(), 0);
        assert_eq!(result.aggregate.total_amount().cents(), 0);
    }

    #[tokio::test]
    async fn total_calculation_with_multiple_items() {
        let service = create_service();

        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        service
            .create_order(CreateOrder::new(order_id, customer_id))
            .await
            .unwrap();

        // Add items: 2 x $10.00 = $20.00
        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget A", 2, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        // Add items: 3 x $5.50 = $16.50
        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-002", "Widget B", 3, Money::from_cents(550)),
            ))
            .await
            .unwrap();

        // Add items: 1 x $25.99 = $25.99
        let result = service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-003", "Widget C", 1, Money::from_cents(2599)),
            ))
            .await
            .unwrap();

        // Total: $20.00 + $16.50 + $25.99 = $62.49 = 6249 cents
        assert_eq!(result.aggregate.total_amount().cents(), 6249);
        assert_eq!(result.aggregate.total_quantity(), 6);
    }
}
