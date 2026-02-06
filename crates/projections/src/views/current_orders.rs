//! Current orders read model — active (non-terminal) orders.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::AggregateId;
use domain::{CustomerId, Money, OrderEvent, OrderState, ProductId};
use event_store::EventEnvelope;
use tokio::sync::RwLock;

use crate::Result;
use crate::projection::{Projection, ProjectionPosition};
use crate::read_model::ReadModel;

/// Summary of an active order item.
#[derive(Debug, Clone)]
pub struct OrderItemSummary {
    pub product_id: ProductId,
    pub product_name: String,
    pub quantity: u32,
    pub unit_price: Money,
}

/// Summary of an active order in the current orders view.
#[derive(Debug, Clone)]
pub struct CurrentOrderSummary {
    pub order_id: AggregateId,
    pub customer_id: CustomerId,
    pub state: OrderState,
    pub item_count: usize,
    pub total_amount: Money,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub items: HashMap<ProductId, OrderItemSummary>,
}

impl CurrentOrderSummary {
    fn recalculate_totals(&mut self) {
        self.item_count = self.items.len();
        self.total_amount = self.items.values().fold(Money::zero(), |acc, item| {
            acc + item.unit_price.multiply(item.quantity)
        });
    }
}

/// Read model view for active (non-terminal) orders.
///
/// Orders are removed from this view when they reach a terminal state
/// (Completed or Cancelled).
#[derive(Clone)]
pub struct CurrentOrdersView {
    orders: Arc<RwLock<HashMap<AggregateId, CurrentOrderSummary>>>,
    position: Arc<RwLock<ProjectionPosition>>,
}

impl CurrentOrdersView {
    /// Creates a new empty current orders view.
    pub fn new() -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
            position: Arc::new(RwLock::new(ProjectionPosition::zero())),
        }
    }

    /// Gets a summary of a specific order.
    pub async fn get_order(&self, order_id: AggregateId) -> Option<CurrentOrderSummary> {
        self.orders.read().await.get(&order_id).cloned()
    }

    /// Gets all active orders.
    pub async fn get_all_orders(&self) -> Vec<CurrentOrderSummary> {
        self.orders.read().await.values().cloned().collect()
    }

    /// Gets active orders filtered by state.
    pub async fn get_orders_by_state(&self, state: OrderState) -> Vec<CurrentOrderSummary> {
        self.orders
            .read()
            .await
            .values()
            .filter(|o| o.state == state)
            .cloned()
            .collect()
    }

    /// Gets active orders for a specific customer.
    pub async fn get_orders_by_customer(
        &self,
        customer_id: CustomerId,
    ) -> Vec<CurrentOrderSummary> {
        self.orders
            .read()
            .await
            .values()
            .filter(|o| o.customer_id == customer_id)
            .cloned()
            .collect()
    }
}

impl Default for CurrentOrdersView {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Projection for CurrentOrdersView {
    fn name(&self) -> &'static str {
        "CurrentOrdersView"
    }

    async fn handle(&self, event: &EventEnvelope) -> Result<()> {
        if event.aggregate_type != "Order" {
            let mut pos = self.position.write().await;
            *pos = pos.advance();
            return Ok(());
        }

        let order_event: OrderEvent = serde_json::from_value(event.payload.clone())?;
        let order_id = event.aggregate_id;

        let mut orders = self.orders.write().await;

        match order_event {
            OrderEvent::OrderCreated(data) => {
                orders.insert(
                    order_id,
                    CurrentOrderSummary {
                        order_id,
                        customer_id: data.customer_id,
                        state: OrderState::Draft,
                        item_count: 0,
                        total_amount: Money::zero(),
                        created_at: data.created_at,
                        updated_at: data.created_at,
                        items: HashMap::new(),
                    },
                );
            }
            OrderEvent::ItemAdded(data) => {
                if let Some(order) = orders.get_mut(&order_id) {
                    order.items.insert(
                        data.product_id.clone(),
                        OrderItemSummary {
                            product_id: data.product_id,
                            product_name: data.product_name,
                            quantity: data.quantity,
                            unit_price: data.unit_price,
                        },
                    );
                    order.recalculate_totals();
                    order.updated_at = event.timestamp;
                }
            }
            OrderEvent::ItemRemoved(data) => {
                if let Some(order) = orders.get_mut(&order_id) {
                    order.items.remove(&data.product_id);
                    order.recalculate_totals();
                    order.updated_at = event.timestamp;
                }
            }
            OrderEvent::ItemQuantityUpdated(data) => {
                if let Some(order) = orders.get_mut(&order_id) {
                    if let Some(item) = order.items.get_mut(&data.product_id) {
                        item.quantity = data.new_quantity;
                    }
                    order.recalculate_totals();
                    order.updated_at = event.timestamp;
                }
            }
            OrderEvent::OrderSubmitted(data) => {
                if let Some(order) = orders.get_mut(&order_id) {
                    order.state = OrderState::Draft; // Submitted is still pre-Reserved
                    order.updated_at = data.submitted_at;
                }
            }
            OrderEvent::OrderReserved(data) => {
                if let Some(order) = orders.get_mut(&order_id) {
                    order.state = OrderState::Reserved;
                    order.updated_at = data.reserved_at;
                }
            }
            OrderEvent::OrderProcessing(data) => {
                if let Some(order) = orders.get_mut(&order_id) {
                    order.state = OrderState::Processing;
                    order.updated_at = data.started_at;
                }
            }
            OrderEvent::OrderCompleted(_) | OrderEvent::OrderCancelled(_) => {
                orders.remove(&order_id);
            }
        }

        let mut pos = self.position.write().await;
        *pos = pos.advance();

        Ok(())
    }

    async fn position(&self) -> ProjectionPosition {
        *self.position.read().await
    }

    async fn reset(&self) -> Result<()> {
        self.orders.write().await.clear();
        *self.position.write().await = ProjectionPosition::zero();
        Ok(())
    }
}

impl ReadModel for CurrentOrdersView {
    fn name(&self) -> &'static str {
        "CurrentOrdersView"
    }

    fn count(&self) -> usize {
        // Use try_read to avoid blocking; returns 0 if lock is held
        self.orders.try_read().map(|o| o.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::OrderEvent;

    fn make_envelope(aggregate_id: AggregateId, version: i64, event: &OrderEvent) -> EventEnvelope {
        EventEnvelope::builder()
            .aggregate_id(aggregate_id)
            .aggregate_type("Order")
            .event_type(domain::DomainEvent::event_type(event))
            .version(event_store::Version::new(version))
            .payload(event)
            .unwrap()
            .build()
    }

    #[tokio::test]
    async fn test_order_created() {
        let view = CurrentOrdersView::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        let event = OrderEvent::order_created(order_id, customer_id);
        let envelope = make_envelope(order_id, 1, &event);
        view.handle(&envelope).await.unwrap();

        let order = view.get_order(order_id).await.unwrap();
        assert_eq!(order.customer_id, customer_id);
        assert_eq!(order.state, OrderState::Draft);
        assert_eq!(order.item_count, 0);
        assert_eq!(order.total_amount, Money::zero());
    }

    #[tokio::test]
    async fn test_add_and_remove_items() {
        let view = CurrentOrdersView::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        // Create order
        let event = OrderEvent::order_created(order_id, customer_id);
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        // Add item
        let item = domain::OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
        let event = OrderEvent::item_added(&item);
        view.handle(&make_envelope(order_id, 2, &event))
            .await
            .unwrap();

        let order = view.get_order(order_id).await.unwrap();
        assert_eq!(order.item_count, 1);
        assert_eq!(order.total_amount.cents(), 2000);

        // Remove item
        let event = OrderEvent::item_removed(ProductId::new("SKU-001"));
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let order = view.get_order(order_id).await.unwrap();
        assert_eq!(order.item_count, 0);
        assert_eq!(order.total_amount, Money::zero());
    }

    #[tokio::test]
    async fn test_quantity_update() {
        let view = CurrentOrdersView::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        let event = OrderEvent::order_created(order_id, customer_id);
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        let item = domain::OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
        let event = OrderEvent::item_added(&item);
        view.handle(&make_envelope(order_id, 2, &event))
            .await
            .unwrap();

        let event = OrderEvent::item_quantity_updated(ProductId::new("SKU-001"), 2, 5);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let order = view.get_order(order_id).await.unwrap();
        assert_eq!(order.total_amount.cents(), 5000);
    }

    #[tokio::test]
    async fn test_terminal_states_remove_order() {
        let view = CurrentOrdersView::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        let event = OrderEvent::order_created(order_id, customer_id);
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        // Complete removes from view
        let event = OrderEvent::order_completed(Some("TRACK-123".to_string()));
        view.handle(&make_envelope(order_id, 2, &event))
            .await
            .unwrap();

        assert!(view.get_order(order_id).await.is_none());
        assert_eq!(view.get_all_orders().await.len(), 0);
    }

    #[tokio::test]
    async fn test_cancelled_removes_order() {
        let view = CurrentOrdersView::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        let event = OrderEvent::order_created(order_id, customer_id);
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        let event = OrderEvent::order_cancelled("Changed mind", None);
        view.handle(&make_envelope(order_id, 2, &event))
            .await
            .unwrap();

        assert!(view.get_order(order_id).await.is_none());
    }

    #[tokio::test]
    async fn test_filter_by_state() {
        let view = CurrentOrdersView::new();
        let customer_id = CustomerId::new();

        // Create two orders
        let order1 = AggregateId::new();
        let order2 = AggregateId::new();

        let event = OrderEvent::order_created(order1, customer_id);
        view.handle(&make_envelope(order1, 1, &event))
            .await
            .unwrap();

        let event = OrderEvent::order_created(order2, customer_id);
        view.handle(&make_envelope(order2, 1, &event))
            .await
            .unwrap();

        // Reserve order2
        let event = OrderEvent::order_reserved(None);
        view.handle(&make_envelope(order2, 2, &event))
            .await
            .unwrap();

        let drafts = view.get_orders_by_state(OrderState::Draft).await;
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].order_id, order1);

        let reserved = view.get_orders_by_state(OrderState::Reserved).await;
        assert_eq!(reserved.len(), 1);
        assert_eq!(reserved[0].order_id, order2);
    }

    #[tokio::test]
    async fn test_filter_by_customer() {
        let view = CurrentOrdersView::new();
        let customer1 = CustomerId::new();
        let customer2 = CustomerId::new();

        let order1 = AggregateId::new();
        let order2 = AggregateId::new();

        let event = OrderEvent::order_created(order1, customer1);
        view.handle(&make_envelope(order1, 1, &event))
            .await
            .unwrap();

        let event = OrderEvent::order_created(order2, customer2);
        view.handle(&make_envelope(order2, 1, &event))
            .await
            .unwrap();

        let c1_orders = view.get_orders_by_customer(customer1).await;
        assert_eq!(c1_orders.len(), 1);
        assert_eq!(c1_orders[0].order_id, order1);
    }

    #[tokio::test]
    async fn test_skips_non_order_events() {
        let view = CurrentOrdersView::new();

        let envelope = EventEnvelope::builder()
            .aggregate_id(AggregateId::new())
            .aggregate_type("Customer")
            .event_type("CustomerCreated")
            .version(event_store::Version::new(1))
            .payload_raw(serde_json::json!({"name": "test"}))
            .build();

        view.handle(&envelope).await.unwrap();
        assert_eq!(view.get_all_orders().await.len(), 0);
        assert_eq!(view.position().await.events_processed, 1);
    }

    #[tokio::test]
    async fn test_position_tracking() {
        let view = CurrentOrdersView::new();
        assert_eq!(view.position().await.events_processed, 0);

        let order_id = AggregateId::new();
        let event = OrderEvent::order_created(order_id, CustomerId::new());
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        assert_eq!(view.position().await.events_processed, 1);
    }

    #[tokio::test]
    async fn test_reset() {
        let view = CurrentOrdersView::new();
        let order_id = AggregateId::new();

        let event = OrderEvent::order_created(order_id, CustomerId::new());
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        assert_eq!(view.get_all_orders().await.len(), 1);

        view.reset().await.unwrap();

        assert_eq!(view.get_all_orders().await.len(), 0);
        assert_eq!(view.position().await.events_processed, 0);
    }
}
