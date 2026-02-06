//! Integration tests: OrderService commands → ProjectionProcessor → all four views.

use common::AggregateId;
use domain::{
    AddItem, CancelOrder, CompleteOrder, CreateOrder, CustomerId, MarkReserved, Money,
    OrderService, OrderState, ProductId, StartProcessing, SubmitOrder,
};
use event_store::InMemoryEventStore;
use projections::{
    CurrentOrdersView, CustomerOrdersView, InventoryView, OrderHistoryView, ProjectionProcessor,
};

/// Helper to set up service, processor, and all views.
fn setup() -> (
    OrderService<InMemoryEventStore>,
    ProjectionProcessor<InMemoryEventStore>,
    CurrentOrdersView,
    OrderHistoryView,
    CustomerOrdersView,
    InventoryView,
) {
    let store = InMemoryEventStore::new();
    let service = OrderService::new(store.clone());

    let current = CurrentOrdersView::new();
    let history = OrderHistoryView::new();
    let customers = CustomerOrdersView::new();
    let inventory = InventoryView::new();

    let mut processor = ProjectionProcessor::new(store);
    processor.register(Box::new(current.clone()));
    processor.register(Box::new(history.clone()));
    processor.register(Box::new(customers.clone()));
    processor.register(Box::new(inventory.clone()));

    (service, processor, current, history, customers, inventory)
}

#[tokio::test]
async fn test_full_order_lifecycle_across_all_views() {
    let (service, processor, current, history, customers, inventory) = setup();

    let customer_id = CustomerId::new();

    // Create order
    let cmd = CreateOrder::for_customer(customer_id);
    let order_id = cmd.order_id;
    service.create_order(cmd).await.unwrap();

    // Add items
    service
        .add_item(AddItem::with_details(
            order_id,
            "SKU-001",
            "Widget",
            3,
            Money::from_cents(1000),
        ))
        .await
        .unwrap();
    service
        .add_item(AddItem::with_details(
            order_id,
            "SKU-002",
            "Gadget",
            1,
            Money::from_cents(2500),
        ))
        .await
        .unwrap();

    // Submit, reserve, process
    service
        .submit_order(SubmitOrder::new(order_id))
        .await
        .unwrap();
    service
        .mark_reserved(MarkReserved::new(order_id, Some("RES-100".to_string())))
        .await
        .unwrap();
    service
        .start_processing(StartProcessing::new(order_id, Some("PAY-200".to_string())))
        .await
        .unwrap();

    // Complete
    service
        .complete_order(CompleteOrder::new(order_id, Some("TRACK-300".to_string())))
        .await
        .unwrap();

    // Catch-up: feed all events to projections
    processor.run_catch_up().await.unwrap();

    // -- CurrentOrdersView: completed order should NOT be here
    assert!(current.get_order(order_id).await.is_none());
    assert_eq!(current.get_all_orders().await.len(), 0);

    // -- OrderHistoryView: completed order should be here
    let hist = history.get_order(order_id).await.unwrap();
    assert_eq!(hist.state, OrderState::Completed);
    assert_eq!(hist.item_count, 2);
    assert_eq!(hist.total_amount.cents(), 5500); // 3*10 + 1*25
    assert_eq!(hist.tracking_number, Some("TRACK-300".to_string()));

    // -- CustomerOrdersView
    let cust = customers.get_customer(customer_id).await.unwrap();
    assert_eq!(cust.total_orders, 1);
    assert_eq!(cust.completed_orders, 1);
    assert_eq!(cust.active_orders, 0);
    assert_eq!(cust.total_spent.cents(), 5500);

    // -- InventoryView
    let widget = inventory
        .get_product(&ProductId::new("SKU-001"))
        .await
        .unwrap();
    assert_eq!(widget.quantity_completed, 3);
    assert_eq!(widget.total_revenue.cents(), 3000);

    let gadget = inventory
        .get_product(&ProductId::new("SKU-002"))
        .await
        .unwrap();
    assert_eq!(gadget.quantity_completed, 1);
    assert_eq!(gadget.total_revenue.cents(), 2500);
}

#[tokio::test]
async fn test_cancelled_order_across_views() {
    let (service, processor, current, history, customers, inventory) = setup();

    let customer_id = CustomerId::new();
    let cmd = CreateOrder::for_customer(customer_id);
    let order_id = cmd.order_id;
    service.create_order(cmd).await.unwrap();

    service
        .add_item(AddItem::with_details(
            order_id,
            "SKU-001",
            "Widget",
            2,
            Money::from_cents(1000),
        ))
        .await
        .unwrap();

    service
        .cancel_order(CancelOrder::new(order_id, "Customer changed mind", None))
        .await
        .unwrap();

    processor.run_catch_up().await.unwrap();

    // CurrentOrders: gone
    assert!(current.get_order(order_id).await.is_none());

    // History: present as cancelled
    let hist = history.get_order(order_id).await.unwrap();
    assert_eq!(hist.state, OrderState::Cancelled);
    assert_eq!(
        hist.cancellation_reason,
        Some("Customer changed mind".to_string())
    );

    // Customer: cancelled
    let cust = customers.get_customer(customer_id).await.unwrap();
    assert_eq!(cust.cancelled_orders, 1);
    assert_eq!(cust.active_orders, 0);
    assert_eq!(cust.total_spent, Money::zero());

    // Inventory: demand removed
    let widget = inventory
        .get_product(&ProductId::new("SKU-001"))
        .await
        .unwrap();
    assert_eq!(widget.total_quantity_ordered, 0);
    assert_eq!(widget.order_count, 0);
}

#[tokio::test]
async fn test_multiple_customers_and_orders() {
    let (service, processor, current, _history, customers, inventory) = setup();

    let customer1 = CustomerId::new();
    let customer2 = CustomerId::new();

    // Customer 1: two orders
    let cmd1 = CreateOrder::for_customer(customer1);
    let order1 = cmd1.order_id;
    service.create_order(cmd1).await.unwrap();
    service
        .add_item(AddItem::with_details(
            order1,
            "SKU-001",
            "Widget",
            5,
            Money::from_cents(1000),
        ))
        .await
        .unwrap();

    let cmd2 = CreateOrder::for_customer(customer1);
    let order2 = cmd2.order_id;
    service.create_order(cmd2).await.unwrap();
    service
        .add_item(AddItem::with_details(
            order2,
            "SKU-002",
            "Gadget",
            3,
            Money::from_cents(2000),
        ))
        .await
        .unwrap();

    // Customer 2: one order
    let cmd3 = CreateOrder::for_customer(customer2);
    let order3 = cmd3.order_id;
    service.create_order(cmd3).await.unwrap();
    service
        .add_item(AddItem::with_details(
            order3,
            "SKU-001",
            "Widget",
            10,
            Money::from_cents(1000),
        ))
        .await
        .unwrap();

    processor.run_catch_up().await.unwrap();

    // CurrentOrders: 3 active orders
    assert_eq!(current.get_all_orders().await.len(), 3);
    assert_eq!(current.get_orders_by_customer(customer1).await.len(), 2);
    assert_eq!(current.get_orders_by_customer(customer2).await.len(), 1);

    // CustomerOrders: 2 customers
    let c1 = customers.get_customer(customer1).await.unwrap();
    assert_eq!(c1.total_orders, 2);
    assert_eq!(c1.active_orders, 2);

    let c2 = customers.get_customer(customer2).await.unwrap();
    assert_eq!(c2.total_orders, 1);

    // Inventory: Widget ordered 15 times across orders
    let widget = inventory
        .get_product(&ProductId::new("SKU-001"))
        .await
        .unwrap();
    assert_eq!(widget.total_quantity_ordered, 15);
    assert_eq!(widget.order_count, 2);
}

#[tokio::test]
async fn test_rebuild_produces_same_state() {
    let (service, processor, current, history, customers, inventory) = setup();

    let customer_id = CustomerId::new();

    // Create and complete an order
    let cmd = CreateOrder::for_customer(customer_id);
    let order_id = cmd.order_id;
    service.create_order(cmd).await.unwrap();
    service
        .add_item(AddItem::with_details(
            order_id,
            "SKU-001",
            "Widget",
            2,
            Money::from_cents(1000),
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

    // Create an active order
    let cmd2 = CreateOrder::for_customer(customer_id);
    let order2 = cmd2.order_id;
    service.create_order(cmd2).await.unwrap();
    service
        .add_item(AddItem::with_details(
            order2,
            "SKU-002",
            "Gadget",
            1,
            Money::from_cents(500),
        ))
        .await
        .unwrap();

    // First catch-up
    processor.run_catch_up().await.unwrap();

    // Capture state
    let current_count = current.get_all_orders().await.len();
    let history_count = history.get_all_history().await.len();
    let customer_stats = customers.get_customer(customer_id).await.unwrap();
    let widget = inventory.get_product(&ProductId::new("SKU-001")).await;
    let gadget = inventory.get_product(&ProductId::new("SKU-002")).await;

    // Rebuild
    processor.rebuild_all().await.unwrap();

    // Verify same state
    assert_eq!(current.get_all_orders().await.len(), current_count);
    assert_eq!(history.get_all_history().await.len(), history_count);

    let rebuilt_stats = customers.get_customer(customer_id).await.unwrap();
    assert_eq!(rebuilt_stats.total_orders, customer_stats.total_orders);
    assert_eq!(
        rebuilt_stats.completed_orders,
        customer_stats.completed_orders
    );
    assert_eq!(rebuilt_stats.active_orders, customer_stats.active_orders);
    assert_eq!(rebuilt_stats.total_spent, customer_stats.total_spent);

    assert_eq!(
        inventory
            .get_product(&ProductId::new("SKU-001"))
            .await
            .map(|d| d.quantity_completed),
        widget.map(|d| d.quantity_completed)
    );
    assert_eq!(
        inventory
            .get_product(&ProductId::new("SKU-002"))
            .await
            .map(|d| d.quantity_in_active_orders),
        gadget.map(|d| d.quantity_in_active_orders)
    );
}

#[tokio::test]
async fn test_process_event_delivers_to_all_projections() {
    let store = InMemoryEventStore::new();
    let service = OrderService::new(store.clone());

    let current = CurrentOrdersView::new();
    let history = OrderHistoryView::new();
    let customers = CustomerOrdersView::new();
    let inventory = InventoryView::new();

    let mut processor = ProjectionProcessor::new(store.clone());
    processor.register(Box::new(current.clone()));
    processor.register(Box::new(history.clone()));
    processor.register(Box::new(customers.clone()));
    processor.register(Box::new(inventory.clone()));

    let customer_id = CustomerId::new();
    let order_id = AggregateId::new();

    // Create order via service (generates events in the store)
    service
        .create_order(CreateOrder::new(order_id, customer_id))
        .await
        .unwrap();

    // Get the events and deliver them individually
    let events = store.get_events_for_aggregate(order_id).await.unwrap();

    for event in &events {
        processor.process_event(event).await.unwrap();
    }

    // Verify all views got the event
    assert!(current.get_order(order_id).await.is_some());
    assert_eq!(current.position().await.events_processed, 1);
    assert_eq!(history.position().await.events_processed, 1);
    assert!(customers.get_customer(customer_id).await.is_some());
    assert_eq!(inventory.position().await.events_processed, 1);
}

use event_store::EventStore;
use projections::Projection;
