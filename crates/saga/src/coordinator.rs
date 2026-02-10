//! Saga coordinator for orchestrating multi-step sagas.

use common::AggregateId;
use domain::{
    Aggregate, CancelOrder, CompleteOrder, DomainEvent, MarkReserved, OrderService, OrderState,
    StartProcessing, SubmitOrder,
};
use event_store::{AppendOptions, EventEnvelope, EventStore, Version};

use crate::aggregate::SagaInstance;
use crate::error::SagaError;
use crate::events::SagaEvent;
use crate::order_fulfillment;
use crate::services::inventory::{InventoryService, ReservationItem};
use crate::services::payment::PaymentService;
use crate::services::shipping::ShippingService;

/// Orchestrates the execution of order fulfillment sagas.
///
/// The coordinator drives a 3-step saga (inventory → payment → shipping)
/// with compensating transactions on failure. The saga itself is event-sourced.
pub struct SagaCoordinator<S, I, P, Sh>
where
    S: EventStore,
    I: InventoryService,
    P: PaymentService,
    Sh: ShippingService,
{
    store: S,
    order_service: OrderService<S>,
    inventory: I,
    payment: P,
    shipping: Sh,
}

impl<S, I, P, Sh> SagaCoordinator<S, I, P, Sh>
where
    S: EventStore + Clone,
    I: InventoryService,
    P: PaymentService,
    Sh: ShippingService,
{
    /// Creates a new saga coordinator.
    pub fn new(store: S, inventory: I, payment: P, shipping: Sh) -> Self {
        let order_service = OrderService::new(store.clone());
        Self {
            store,
            order_service,
            inventory,
            payment,
            shipping,
        }
    }

    /// Executes an order fulfillment saga for the given order.
    ///
    /// The order must be in Draft state with at least one item.
    /// Returns the saga instance ID on success.
    #[tracing::instrument(skip(self), fields(saga_type = "OrderFulfillment"))]
    pub async fn execute_saga(&self, order_id: AggregateId) -> Result<AggregateId, SagaError> {
        metrics::counter!("saga_executions_total").increment(1);
        let saga_start = std::time::Instant::now();
        // 1. Load and validate the order
        let order = self
            .order_service
            .get_order(order_id)
            .await?
            .ok_or(SagaError::OrderNotFound(order_id))?;

        if order.state() != OrderState::Draft {
            return Err(SagaError::OrderNotReady(format!(
                "Order is in {} state, expected Draft",
                order.state()
            )));
        }

        if !order.has_items() {
            return Err(SagaError::OrderNotReady("Order has no items".to_string()));
        }

        let customer_id = order
            .customer_id()
            .ok_or_else(|| SagaError::OrderNotReady("Order has no customer ID".to_string()))?;
        let total_amount = order.total_amount();
        let items: Vec<ReservationItem> = order
            .items()
            .map(|item| ReservationItem {
                product_id: item.product_id.clone(),
                product_name: item.product_name.clone(),
                quantity: item.quantity,
            })
            .collect();

        // 2. Submit the order (stays Draft, records OrderSubmitted)
        self.order_service
            .submit_order(SubmitOrder::new(order_id))
            .await?;

        // 3. Create the saga
        let saga_id = AggregateId::new();
        let mut version = Version::initial();

        let started_event =
            SagaEvent::saga_started(saga_id, order_id, order_fulfillment::SAGA_TYPE);
        version = self
            .append_saga_event(saga_id, version, &started_event)
            .await?;

        // Build saga state for compensation tracking
        let mut saga = SagaInstance::default();
        saga.apply(started_event);

        // 4. Step 1: Reserve Inventory
        tracing::info!(
            step = order_fulfillment::STEP_RESERVE_INVENTORY,
            "saga step started"
        );
        let step1_started = SagaEvent::step_started(order_fulfillment::STEP_RESERVE_INVENTORY);
        version = self
            .append_saga_event(saga_id, version, &step1_started)
            .await?;
        saga.apply(step1_started);

        match self.inventory.reserve(order_id, items).await {
            Ok(result) => {
                let reservation_id = result.reservation_id.clone();
                let step1_completed = SagaEvent::step_completed(
                    order_fulfillment::STEP_RESERVE_INVENTORY,
                    Some(reservation_id.clone()),
                    None,
                    None,
                );
                version = self
                    .append_saga_event(saga_id, version, &step1_completed)
                    .await?;
                saga.apply(step1_completed);

                // Advance order state to Reserved
                self.order_service
                    .mark_reserved(MarkReserved::new(order_id, Some(reservation_id)))
                    .await?;
            }
            Err(e) => {
                let step1_failed = SagaEvent::step_failed(
                    order_fulfillment::STEP_RESERVE_INVENTORY,
                    e.to_string(),
                );
                version = self
                    .append_saga_event(saga_id, version, &step1_failed)
                    .await?;
                saga.apply(step1_failed);

                self.compensate(&mut saga, saga_id, &mut version, order_id)
                    .await?;
                metrics::histogram!("saga_duration_seconds")
                    .record(saga_start.elapsed().as_secs_f64());
                return Ok(saga_id);
            }
        }

        // 5. Step 2: Process Payment
        tracing::info!(
            step = order_fulfillment::STEP_PROCESS_PAYMENT,
            "saga step started"
        );
        let step2_started = SagaEvent::step_started(order_fulfillment::STEP_PROCESS_PAYMENT);
        version = self
            .append_saga_event(saga_id, version, &step2_started)
            .await?;
        saga.apply(step2_started);

        match self
            .payment
            .charge(order_id, customer_id, total_amount)
            .await
        {
            Ok(result) => {
                let payment_id = result.payment_id.clone();
                let step2_completed = SagaEvent::step_completed(
                    order_fulfillment::STEP_PROCESS_PAYMENT,
                    None,
                    Some(payment_id.clone()),
                    None,
                );
                version = self
                    .append_saga_event(saga_id, version, &step2_completed)
                    .await?;
                saga.apply(step2_completed);

                // Advance order state to Processing
                self.order_service
                    .start_processing(StartProcessing::new(order_id, Some(payment_id)))
                    .await?;
            }
            Err(e) => {
                let step2_failed =
                    SagaEvent::step_failed(order_fulfillment::STEP_PROCESS_PAYMENT, e.to_string());
                version = self
                    .append_saga_event(saga_id, version, &step2_failed)
                    .await?;
                saga.apply(step2_failed);

                self.compensate(&mut saga, saga_id, &mut version, order_id)
                    .await?;
                metrics::histogram!("saga_duration_seconds")
                    .record(saga_start.elapsed().as_secs_f64());
                return Ok(saga_id);
            }
        }

        // 6. Step 3: Create Shipment
        tracing::info!(
            step = order_fulfillment::STEP_CREATE_SHIPMENT,
            "saga step started"
        );
        let step3_started = SagaEvent::step_started(order_fulfillment::STEP_CREATE_SHIPMENT);
        version = self
            .append_saga_event(saga_id, version, &step3_started)
            .await?;
        saga.apply(step3_started);

        match self.shipping.create_shipment(order_id).await {
            Ok(result) => {
                let tracking_number = result.tracking_number.clone();
                let step3_completed = SagaEvent::step_completed(
                    order_fulfillment::STEP_CREATE_SHIPMENT,
                    None,
                    None,
                    Some(tracking_number.clone()),
                );
                version = self
                    .append_saga_event(saga_id, version, &step3_completed)
                    .await?;
                saga.apply(step3_completed);

                // Advance order state to Completed
                self.order_service
                    .complete_order(CompleteOrder::new(order_id, Some(tracking_number)))
                    .await?;
            }
            Err(e) => {
                let step3_failed =
                    SagaEvent::step_failed(order_fulfillment::STEP_CREATE_SHIPMENT, e.to_string());
                version = self
                    .append_saga_event(saga_id, version, &step3_failed)
                    .await?;
                saga.apply(step3_failed);

                self.compensate(&mut saga, saga_id, &mut version, order_id)
                    .await?;
                metrics::histogram!("saga_duration_seconds")
                    .record(saga_start.elapsed().as_secs_f64());
                return Ok(saga_id);
            }
        }

        // 7. Saga completed
        let completed_event = SagaEvent::saga_completed();
        self.append_saga_event(saga_id, version, &completed_event)
            .await?;

        let duration = saga_start.elapsed().as_secs_f64();
        metrics::histogram!("saga_duration_seconds").record(duration);
        metrics::counter!("saga_completed").increment(1);
        tracing::info!(%saga_id, duration, "saga completed successfully");

        Ok(saga_id)
    }

    /// Runs compensating transactions in reverse order of completed steps.
    #[tracing::instrument(skip(self, saga))]
    async fn compensate(
        &self,
        saga: &mut SagaInstance,
        saga_id: AggregateId,
        version: &mut Version,
        order_id: AggregateId,
    ) -> Result<(), SagaError> {
        let failed_step = saga.failure_reason().unwrap_or("unknown").to_string();

        let comp_started = SagaEvent::compensation_started(&failed_step);
        *version = self
            .append_saga_event(saga_id, *version, &comp_started)
            .await?;
        saga.apply(comp_started);

        // Compensate in reverse order of completed steps
        let completed: Vec<String> = saga.completed_steps().to_vec();
        for step in completed.iter().rev() {
            match step.as_str() {
                order_fulfillment::STEP_CREATE_SHIPMENT => {
                    if let Some(tracking_number) = saga.tracking_number() {
                        let tn = tracking_number.to_string();
                        match self.shipping.cancel_shipment(&tn).await {
                            Ok(()) => {
                                let event = SagaEvent::compensation_step_completed(step);
                                *version =
                                    self.append_saga_event(saga_id, *version, &event).await?;
                                saga.apply(event);
                            }
                            Err(e) => {
                                let event =
                                    SagaEvent::compensation_step_failed(step, e.to_string());
                                *version =
                                    self.append_saga_event(saga_id, *version, &event).await?;
                                saga.apply(event);
                            }
                        }
                    }
                }
                order_fulfillment::STEP_PROCESS_PAYMENT => {
                    if let Some(payment_id) = saga.payment_id() {
                        let pid = payment_id.to_string();
                        match self.payment.refund(&pid).await {
                            Ok(()) => {
                                let event = SagaEvent::compensation_step_completed(step);
                                *version =
                                    self.append_saga_event(saga_id, *version, &event).await?;
                                saga.apply(event);
                            }
                            Err(e) => {
                                let event =
                                    SagaEvent::compensation_step_failed(step, e.to_string());
                                *version =
                                    self.append_saga_event(saga_id, *version, &event).await?;
                                saga.apply(event);
                            }
                        }
                    }
                }
                order_fulfillment::STEP_RESERVE_INVENTORY => {
                    if let Some(reservation_id) = saga.reservation_id() {
                        let rid = reservation_id.to_string();
                        match self.inventory.release(&rid).await {
                            Ok(()) => {
                                let event = SagaEvent::compensation_step_completed(step);
                                *version =
                                    self.append_saga_event(saga_id, *version, &event).await?;
                                saga.apply(event);
                            }
                            Err(e) => {
                                let event =
                                    SagaEvent::compensation_step_failed(step, e.to_string());
                                *version =
                                    self.append_saga_event(saga_id, *version, &event).await?;
                                saga.apply(event);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Cancel the order
        self.order_service
            .cancel_order(CancelOrder::new(
                order_id,
                format!("Saga failed: {}", failed_step),
                Some("saga_coordinator".to_string()),
            ))
            .await?;

        // Record saga failure
        let failed_event = SagaEvent::saga_failed(format!("Step failed: {}", failed_step));
        *version = self
            .append_saga_event(saga_id, *version, &failed_event)
            .await?;
        saga.apply(failed_event);

        metrics::counter!("saga_failed").increment(1);
        tracing::warn!(%saga_id, %order_id, reason = %failed_step, "saga failed");

        Ok(())
    }

    /// Loads a saga instance by ID from the event store.
    pub async fn get_saga(&self, saga_id: AggregateId) -> Result<Option<SagaInstance>, SagaError> {
        let events = self.store.get_events_for_aggregate(saga_id).await?;

        if events.is_empty() {
            return Ok(None);
        }

        let mut saga = SagaInstance::default();
        for envelope in events {
            let event: SagaEvent = serde_json::from_value(envelope.payload)?;
            saga.apply(event);
        }
        Ok(Some(saga))
    }

    /// Appends a single saga event to the event store.
    async fn append_saga_event(
        &self,
        saga_id: AggregateId,
        current_version: Version,
        event: &SagaEvent,
    ) -> Result<Version, SagaError> {
        let next_version = current_version.next();

        let envelope = EventEnvelope::builder()
            .event_type(event.event_type())
            .aggregate_id(saga_id)
            .aggregate_type(SagaInstance::aggregate_type())
            .version(next_version)
            .payload(event)?
            .build();

        let new_version = self
            .store
            .append(
                vec![envelope],
                AppendOptions::expect_version(current_version),
            )
            .await?;

        Ok(new_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::inventory::InMemoryInventoryService;
    use crate::services::payment::InMemoryPaymentService;
    use crate::services::shipping::InMemoryShippingService;
    use domain::{AddItem, CreateOrder, CustomerId, Money, OrderItem};
    use event_store::InMemoryEventStore;

    async fn setup() -> (
        SagaCoordinator<
            InMemoryEventStore,
            InMemoryInventoryService,
            InMemoryPaymentService,
            InMemoryShippingService,
        >,
        OrderService<InMemoryEventStore>,
        InMemoryInventoryService,
        InMemoryPaymentService,
        InMemoryShippingService,
    ) {
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

        (coordinator, order_service, inventory, payment, shipping)
    }

    async fn create_order_with_items(service: &OrderService<InMemoryEventStore>) -> AggregateId {
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;
        service.create_order(cmd).await.unwrap();

        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000)),
            ))
            .await
            .unwrap();

        service
            .add_item(AddItem::new(
                order_id,
                OrderItem::new("SKU-002", "Gadget", 1, Money::from_cents(2500)),
            ))
            .await
            .unwrap();

        order_id
    }

    #[tokio::test]
    async fn test_happy_path() {
        let (coordinator, order_service, inventory, payment, shipping) = setup().await;
        let order_id = create_order_with_items(&order_service).await;

        let saga_id = coordinator.execute_saga(order_id).await.unwrap();

        // Verify saga state
        let saga = coordinator.get_saga(saga_id).await.unwrap().unwrap();
        assert_eq!(saga.state(), crate::state::SagaState::Completed);
        assert_eq!(saga.completed_steps().len(), 3);
        assert!(saga.reservation_id().is_some());
        assert!(saga.payment_id().is_some());
        assert!(saga.tracking_number().is_some());

        // Verify order state
        let order = order_service.get_order(order_id).await.unwrap().unwrap();
        assert_eq!(order.state(), OrderState::Completed);

        // Verify external services
        assert_eq!(inventory.reservation_count(), 1);
        assert_eq!(payment.payment_count(), 1);
        assert_eq!(shipping.shipment_count(), 1);
    }

    #[tokio::test]
    async fn test_inventory_failure() {
        let (coordinator, order_service, inventory, payment, shipping) = setup().await;
        let order_id = create_order_with_items(&order_service).await;

        inventory.set_fail_on_reserve(true);

        let saga_id = coordinator.execute_saga(order_id).await.unwrap();

        // Verify saga failed
        let saga = coordinator.get_saga(saga_id).await.unwrap().unwrap();
        assert_eq!(saga.state(), crate::state::SagaState::Failed);
        assert!(saga.completed_steps().is_empty());

        // Verify order cancelled
        let order = order_service.get_order(order_id).await.unwrap().unwrap();
        assert_eq!(order.state(), OrderState::Cancelled);

        // No external service calls should remain
        assert_eq!(inventory.reservation_count(), 0);
        assert_eq!(payment.payment_count(), 0);
        assert_eq!(shipping.shipment_count(), 0);
    }

    #[tokio::test]
    async fn test_payment_failure() {
        let (coordinator, order_service, inventory, payment, shipping) = setup().await;
        let order_id = create_order_with_items(&order_service).await;

        payment.set_fail_on_charge(true);

        let saga_id = coordinator.execute_saga(order_id).await.unwrap();

        // Verify saga failed
        let saga = coordinator.get_saga(saga_id).await.unwrap().unwrap();
        assert_eq!(saga.state(), crate::state::SagaState::Failed);
        assert_eq!(saga.completed_steps(), &["reserve_inventory"]);

        // Verify order cancelled
        let order = order_service.get_order(order_id).await.unwrap().unwrap();
        assert_eq!(order.state(), OrderState::Cancelled);

        // Inventory reservation should be released
        assert_eq!(inventory.reservation_count(), 0);
        assert_eq!(payment.payment_count(), 0);
        assert_eq!(shipping.shipment_count(), 0);
    }

    #[tokio::test]
    async fn test_shipping_failure() {
        let (coordinator, order_service, inventory, payment, shipping) = setup().await;
        let order_id = create_order_with_items(&order_service).await;

        shipping.set_fail_on_create(true);

        let saga_id = coordinator.execute_saga(order_id).await.unwrap();

        // Verify saga failed
        let saga = coordinator.get_saga(saga_id).await.unwrap().unwrap();
        assert_eq!(saga.state(), crate::state::SagaState::Failed);
        assert_eq!(
            saga.completed_steps(),
            &["reserve_inventory", "process_payment"]
        );

        // Verify order cancelled
        let order = order_service.get_order(order_id).await.unwrap().unwrap();
        assert_eq!(order.state(), OrderState::Cancelled);

        // Both inventory and payment should be compensated
        assert_eq!(inventory.reservation_count(), 0);
        assert_eq!(payment.payment_count(), 0);
        assert_eq!(shipping.shipment_count(), 0);
    }

    #[tokio::test]
    async fn test_order_not_found() {
        let (coordinator, _, _, _, _) = setup().await;
        let fake_id = AggregateId::new();

        let result = coordinator.execute_saga(fake_id).await;
        assert!(matches!(result, Err(SagaError::OrderNotFound(_))));
    }

    #[tokio::test]
    async fn test_order_without_items() {
        let (coordinator, order_service, _, _, _) = setup().await;

        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;
        order_service.create_order(cmd).await.unwrap();

        let result = coordinator.execute_saga(order_id).await;
        assert!(matches!(result, Err(SagaError::OrderNotReady(_))));
    }

    #[tokio::test]
    async fn test_saga_event_sourced_recovery() {
        let (coordinator, order_service, _, _, _) = setup().await;
        let order_id = create_order_with_items(&order_service).await;

        let saga_id = coordinator.execute_saga(order_id).await.unwrap();

        // Load saga from event store
        let saga = coordinator.get_saga(saga_id).await.unwrap().unwrap();

        assert_eq!(saga.id(), Some(saga_id));
        assert_eq!(saga.order_id(), Some(order_id));
        assert_eq!(saga.state(), crate::state::SagaState::Completed);
        assert_eq!(saga.saga_type(), order_fulfillment::SAGA_TYPE);
    }

    #[tokio::test]
    async fn test_nonexistent_saga() {
        let (coordinator, _, _, _, _) = setup().await;
        let result = coordinator.get_saga(AggregateId::new()).await.unwrap();
        assert!(result.is_none());
    }
}
