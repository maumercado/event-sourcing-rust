//! Integration tests for the saga pattern implementation.

use common::AggregateId;
use domain::{
    AddItem, Aggregate, CreateOrder, CustomerId, Money, OrderItem, OrderService, OrderState,
};
use event_store::InMemoryEventStore;
use saga::{
    InMemoryInventoryService, InMemoryPaymentService, InMemoryShippingService, SagaCoordinator,
    SagaState,
};

type TestCoordinator = SagaCoordinator<
    InMemoryEventStore,
    InMemoryInventoryService,
    InMemoryPaymentService,
    InMemoryShippingService,
>;

struct TestHarness {
    coordinator: TestCoordinator,
    order_service: OrderService<InMemoryEventStore>,
    inventory: InMemoryInventoryService,
    payment: InMemoryPaymentService,
    shipping: InMemoryShippingService,
}

impl TestHarness {
    fn new() -> Self {
        let store = InMemoryEventStore::new();
        let inventory = InMemoryInventoryService::new();
        let payment = InMemoryPaymentService::new();
        let shipping = InMemoryShippingService::new();

        let coordinator = SagaCoordinator::new(
            store.clone(),
            inventory.clone(),
            payment.clone(),
            shipping.clone(),
        );
        let order_service = OrderService::new(store);

        Self {
            coordinator,
            order_service,
            inventory,
            payment,
            shipping,
        }
    }

    async fn create_order(&self) -> AggregateId {
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;
        self.order_service.create_order(cmd).await.unwrap();

        self.order_service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        self.order_service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-002", "Gadget", 1, Money::from_cents(2500)),
            ))
            .await
            .unwrap();

        order_id
    }
}

#[tokio::test]
async fn test_happy_path_full_order_fulfillment() {
    let h = TestHarness::new();
    let order_id = h.create_order().await;

    // Execute the saga
    let saga_id = h.coordinator.execute_saga(order_id).await.unwrap();

    // Verify saga completed
    let saga = h.coordinator.get_saga(saga_id).await.unwrap().unwrap();
    assert_eq!(saga.id(), Some(saga_id));
    assert_eq!(saga.order_id(), Some(order_id));
    assert_eq!(saga.state(), SagaState::Completed);
    assert_eq!(saga.saga_type(), "OrderFulfillment");
    assert_eq!(saga.completed_steps().len(), 3);
    assert_eq!(
        saga.completed_steps(),
        &["reserve_inventory", "process_payment", "create_shipment"]
    );

    // Verify context was accumulated
    assert!(saga.reservation_id().is_some());
    assert!(saga.payment_id().is_some());
    assert!(saga.tracking_number().is_some());

    // Verify order reached terminal state
    let order = h.order_service.get_order(order_id).await.unwrap().unwrap();
    assert_eq!(order.state(), OrderState::Completed);

    // Verify external services have records
    assert_eq!(h.inventory.reservation_count(), 1);
    assert_eq!(h.payment.payment_count(), 1);
    assert_eq!(h.shipping.shipment_count(), 1);
}

#[tokio::test]
async fn test_inventory_failure_no_compensation_needed() {
    let h = TestHarness::new();
    let order_id = h.create_order().await;

    h.inventory.set_fail_on_reserve(true);

    let saga_id = h.coordinator.execute_saga(order_id).await.unwrap();

    // Saga should be Failed
    let saga = h.coordinator.get_saga(saga_id).await.unwrap().unwrap();
    assert_eq!(saga.state(), SagaState::Failed);
    assert!(saga.completed_steps().is_empty());

    // Order should be cancelled
    let order = h.order_service.get_order(order_id).await.unwrap().unwrap();
    assert_eq!(order.state(), OrderState::Cancelled);

    // No external service records should remain
    assert_eq!(h.inventory.reservation_count(), 0);
    assert_eq!(h.payment.payment_count(), 0);
    assert_eq!(h.shipping.shipment_count(), 0);
}

#[tokio::test]
async fn test_payment_failure_releases_inventory() {
    let h = TestHarness::new();
    let order_id = h.create_order().await;

    h.payment.set_fail_on_charge(true);

    let saga_id = h.coordinator.execute_saga(order_id).await.unwrap();

    // Saga should be Failed
    let saga = h.coordinator.get_saga(saga_id).await.unwrap().unwrap();
    assert_eq!(saga.state(), SagaState::Failed);
    assert_eq!(saga.completed_steps(), &["reserve_inventory"]);
    assert!(saga.reservation_id().is_some());
    assert!(saga.payment_id().is_none());

    // Order should be cancelled
    let order = h.order_service.get_order(order_id).await.unwrap().unwrap();
    assert_eq!(order.state(), OrderState::Cancelled);

    // Inventory reservation should be released (compensated)
    assert_eq!(h.inventory.reservation_count(), 0);
    assert_eq!(h.payment.payment_count(), 0);
    assert_eq!(h.shipping.shipment_count(), 0);
}

#[tokio::test]
async fn test_shipping_failure_refunds_payment_releases_inventory() {
    let h = TestHarness::new();
    let order_id = h.create_order().await;

    h.shipping.set_fail_on_create(true);

    let saga_id = h.coordinator.execute_saga(order_id).await.unwrap();

    // Saga should be Failed
    let saga = h.coordinator.get_saga(saga_id).await.unwrap().unwrap();
    assert_eq!(saga.state(), SagaState::Failed);
    assert_eq!(
        saga.completed_steps(),
        &["reserve_inventory", "process_payment"]
    );
    assert!(saga.reservation_id().is_some());
    assert!(saga.payment_id().is_some());
    assert!(saga.tracking_number().is_none());

    // Order should be cancelled
    let order = h.order_service.get_order(order_id).await.unwrap().unwrap();
    assert_eq!(order.state(), OrderState::Cancelled);

    // Both inventory and payment should be compensated
    assert_eq!(h.inventory.reservation_count(), 0);
    assert_eq!(h.payment.payment_count(), 0);
    assert_eq!(h.shipping.shipment_count(), 0);
}

#[tokio::test]
async fn test_saga_event_sourced_can_reload_from_store() {
    let h = TestHarness::new();
    let order_id = h.create_order().await;

    let saga_id = h.coordinator.execute_saga(order_id).await.unwrap();

    // Load saga twice — both loads should produce identical state
    let saga1 = h.coordinator.get_saga(saga_id).await.unwrap().unwrap();
    let saga2 = h.coordinator.get_saga(saga_id).await.unwrap().unwrap();

    assert_eq!(saga1.id(), saga2.id());
    assert_eq!(saga1.state(), saga2.state());
    assert_eq!(saga1.order_id(), saga2.order_id());
    assert_eq!(saga1.completed_steps(), saga2.completed_steps());
    assert_eq!(saga1.reservation_id(), saga2.reservation_id());
    assert_eq!(saga1.payment_id(), saga2.payment_id());
    assert_eq!(saga1.tracking_number(), saga2.tracking_number());
}

#[tokio::test]
async fn test_multiple_independent_sagas() {
    let h = TestHarness::new();

    // Create two separate orders
    let order_id_1 = h.create_order().await;
    let order_id_2 = h.create_order().await;

    // Execute sagas for both
    let saga_id_1 = h.coordinator.execute_saga(order_id_1).await.unwrap();
    let saga_id_2 = h.coordinator.execute_saga(order_id_2).await.unwrap();

    // Both should complete independently
    let saga1 = h.coordinator.get_saga(saga_id_1).await.unwrap().unwrap();
    let saga2 = h.coordinator.get_saga(saga_id_2).await.unwrap().unwrap();

    assert_eq!(saga1.state(), SagaState::Completed);
    assert_eq!(saga2.state(), SagaState::Completed);
    assert_ne!(saga_id_1, saga_id_2);
    assert_eq!(saga1.order_id(), Some(order_id_1));
    assert_eq!(saga2.order_id(), Some(order_id_2));

    // External services should have records for both
    assert_eq!(h.inventory.reservation_count(), 2);
    assert_eq!(h.payment.payment_count(), 2);
    assert_eq!(h.shipping.shipment_count(), 2);
}

#[tokio::test]
async fn test_one_saga_fails_other_succeeds() {
    let h = TestHarness::new();

    let order_id_1 = h.create_order().await;
    let order_id_2 = h.create_order().await;

    // First saga succeeds
    let saga_id_1 = h.coordinator.execute_saga(order_id_1).await.unwrap();

    // Second saga fails at payment
    h.payment.set_fail_on_charge(true);
    let saga_id_2 = h.coordinator.execute_saga(order_id_2).await.unwrap();

    let saga1 = h.coordinator.get_saga(saga_id_1).await.unwrap().unwrap();
    let saga2 = h.coordinator.get_saga(saga_id_2).await.unwrap().unwrap();

    assert_eq!(saga1.state(), SagaState::Completed);
    assert_eq!(saga2.state(), SagaState::Failed);

    let order1 = h
        .order_service
        .get_order(order_id_1)
        .await
        .unwrap()
        .unwrap();
    let order2 = h
        .order_service
        .get_order(order_id_2)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(order1.state(), OrderState::Completed);
    assert_eq!(order2.state(), OrderState::Cancelled);

    // First saga's services remain; second saga's are compensated
    // (1 from saga1 + 0 from saga2 compensation = 1)
    assert_eq!(h.inventory.reservation_count(), 1);
    assert_eq!(h.payment.payment_count(), 1);
    assert_eq!(h.shipping.shipment_count(), 1);
}
