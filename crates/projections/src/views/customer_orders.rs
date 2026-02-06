//! Customer orders read model — per-customer order statistics.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use common::AggregateId;
use domain::{CustomerId, Money, OrderEvent, ProductId};
use event_store::EventEnvelope;
use tokio::sync::RwLock;

use crate::Result;
use crate::projection::{Projection, ProjectionPosition};
use crate::read_model::ReadModel;

/// Per-customer order statistics.
#[derive(Debug, Clone)]
pub struct CustomerOrdersSummary {
    pub customer_id: CustomerId,
    pub total_orders: u64,
    pub active_orders: u64,
    pub completed_orders: u64,
    pub cancelled_orders: u64,
    pub total_spent: Money,
    pub order_ids: Vec<AggregateId>,
}

/// Tracks per-order item totals for computing total_spent on completion.
#[derive(Debug, Clone)]
struct OrderItemTracker {
    items: HashMap<ProductId, (u32, Money)>, // (quantity, unit_price)
}

impl OrderItemTracker {
    fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    fn total(&self) -> Money {
        self.items
            .values()
            .fold(Money::zero(), |acc, (qty, price)| {
                acc + price.multiply(*qty)
            })
    }
}

/// Internal state for the customer orders view.
struct CustomerOrdersState {
    customers: HashMap<CustomerId, CustomerOrdersSummary>,
    /// Maps order_id -> customer_id for lookups.
    order_to_customer: HashMap<AggregateId, CustomerId>,
    /// Tracks items per order for computing totals.
    order_items: HashMap<AggregateId, OrderItemTracker>,
    position: ProjectionPosition,
}

/// Read model view for per-customer order statistics.
///
/// Tracks order counts, spending, and order IDs per customer.
#[derive(Clone)]
pub struct CustomerOrdersView {
    state: Arc<RwLock<CustomerOrdersState>>,
}

impl CustomerOrdersView {
    /// Creates a new empty customer orders view.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(CustomerOrdersState {
                customers: HashMap::new(),
                order_to_customer: HashMap::new(),
                order_items: HashMap::new(),
                position: ProjectionPosition::zero(),
            })),
        }
    }

    /// Gets statistics for a specific customer.
    pub async fn get_customer(&self, customer_id: CustomerId) -> Option<CustomerOrdersSummary> {
        self.state.read().await.customers.get(&customer_id).cloned()
    }

    /// Gets all customer statistics.
    pub async fn get_all_customers(&self) -> Vec<CustomerOrdersSummary> {
        self.state
            .read()
            .await
            .customers
            .values()
            .cloned()
            .collect()
    }

    /// Gets the top customers by total spent, limited to `limit` results.
    pub async fn get_top_customers(&self, limit: usize) -> Vec<CustomerOrdersSummary> {
        let state = self.state.read().await;
        let mut customers: Vec<_> = state.customers.values().cloned().collect();
        customers.sort_by(|a, b| b.total_spent.cents().cmp(&a.total_spent.cents()));
        customers.truncate(limit);
        customers
    }
}

impl Default for CustomerOrdersView {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Projection for CustomerOrdersView {
    fn name(&self) -> &'static str {
        "CustomerOrdersView"
    }

    async fn handle(&self, event: &EventEnvelope) -> Result<()> {
        if event.aggregate_type != "Order" {
            let mut state = self.state.write().await;
            state.position = state.position.advance();
            return Ok(());
        }

        let order_event: OrderEvent = serde_json::from_value(event.payload.clone())?;
        let order_id = event.aggregate_id;

        let mut state = self.state.write().await;

        match order_event {
            OrderEvent::OrderCreated(data) => {
                let customer_id = data.customer_id;
                state.order_to_customer.insert(order_id, customer_id);
                state.order_items.insert(order_id, OrderItemTracker::new());

                let entry = state
                    .customers
                    .entry(customer_id)
                    .or_insert(CustomerOrdersSummary {
                        customer_id,
                        total_orders: 0,
                        active_orders: 0,
                        completed_orders: 0,
                        cancelled_orders: 0,
                        total_spent: Money::zero(),
                        order_ids: Vec::new(),
                    });
                entry.total_orders += 1;
                entry.active_orders += 1;
                entry.order_ids.push(order_id);
            }
            OrderEvent::ItemAdded(data) => {
                if let Some(tracker) = state.order_items.get_mut(&order_id) {
                    tracker
                        .items
                        .insert(data.product_id, (data.quantity, data.unit_price));
                }
            }
            OrderEvent::ItemRemoved(data) => {
                if let Some(tracker) = state.order_items.get_mut(&order_id) {
                    tracker.items.remove(&data.product_id);
                }
            }
            OrderEvent::ItemQuantityUpdated(data) => {
                if let Some(tracker) = state.order_items.get_mut(&order_id)
                    && let Some(entry) = tracker.items.get_mut(&data.product_id)
                {
                    entry.0 = data.new_quantity;
                }
            }
            OrderEvent::OrderCompleted(_) => {
                if let Some(&customer_id) = state.order_to_customer.get(&order_id) {
                    let order_total = state
                        .order_items
                        .get(&order_id)
                        .map(|t| t.total())
                        .unwrap_or(Money::zero());

                    if let Some(customer) = state.customers.get_mut(&customer_id) {
                        customer.active_orders = customer.active_orders.saturating_sub(1);
                        customer.completed_orders += 1;
                        customer.total_spent = customer.total_spent.add(order_total);
                    }
                }
            }
            OrderEvent::OrderCancelled(_) => {
                if let Some(&customer_id) = state.order_to_customer.get(&order_id)
                    && let Some(customer) = state.customers.get_mut(&customer_id)
                {
                    customer.active_orders = customer.active_orders.saturating_sub(1);
                    customer.cancelled_orders += 1;
                }
            }
            // State transitions don't affect customer stats
            OrderEvent::OrderSubmitted(_)
            | OrderEvent::OrderReserved(_)
            | OrderEvent::OrderProcessing(_) => {}
        }

        state.position = state.position.advance();
        Ok(())
    }

    async fn position(&self) -> ProjectionPosition {
        self.state.read().await.position
    }

    async fn reset(&self) -> Result<()> {
        let mut state = self.state.write().await;
        state.customers.clear();
        state.order_to_customer.clear();
        state.order_items.clear();
        state.position = ProjectionPosition::zero();
        Ok(())
    }
}

impl ReadModel for CustomerOrdersView {
    fn name(&self) -> &'static str {
        "CustomerOrdersView"
    }

    fn count(&self) -> usize {
        self.state
            .try_read()
            .map(|s| s.customers.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{DomainEvent, OrderItem};

    fn make_envelope(aggregate_id: AggregateId, version: i64, event: &OrderEvent) -> EventEnvelope {
        EventEnvelope::builder()
            .aggregate_id(aggregate_id)
            .aggregate_type("Order")
            .event_type(event.event_type())
            .version(event_store::Version::new(version))
            .payload(event)
            .unwrap()
            .build()
    }

    async fn create_order_with_items(
        view: &CustomerOrdersView,
        order_id: AggregateId,
        customer_id: CustomerId,
    ) {
        let event = OrderEvent::order_created(order_id, customer_id);
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
        let event = OrderEvent::item_added(&item);
        view.handle(&make_envelope(order_id, 2, &event))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_new_customer_on_order_created() {
        let view = CustomerOrdersView::new();
        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        let event = OrderEvent::order_created(order_id, customer_id);
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        let summary = view.get_customer(customer_id).await.unwrap();
        assert_eq!(summary.total_orders, 1);
        assert_eq!(summary.active_orders, 1);
        assert_eq!(summary.completed_orders, 0);
        assert_eq!(summary.cancelled_orders, 0);
        assert_eq!(summary.total_spent, Money::zero());
        assert_eq!(summary.order_ids.len(), 1);
    }

    #[tokio::test]
    async fn test_multiple_orders_for_customer() {
        let view = CustomerOrdersView::new();
        let customer_id = CustomerId::new();

        let order1 = AggregateId::new();
        let order2 = AggregateId::new();

        create_order_with_items(&view, order1, customer_id).await;
        create_order_with_items(&view, order2, customer_id).await;

        let summary = view.get_customer(customer_id).await.unwrap();
        assert_eq!(summary.total_orders, 2);
        assert_eq!(summary.active_orders, 2);
        assert_eq!(summary.order_ids.len(), 2);
    }

    #[tokio::test]
    async fn test_completed_order_updates_stats() {
        let view = CustomerOrdersView::new();
        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id, customer_id).await;

        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let summary = view.get_customer(customer_id).await.unwrap();
        assert_eq!(summary.active_orders, 0);
        assert_eq!(summary.completed_orders, 1);
        assert_eq!(summary.total_spent.cents(), 2000); // 2 x $10
    }

    #[tokio::test]
    async fn test_cancelled_order_updates_stats() {
        let view = CustomerOrdersView::new();
        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id, customer_id).await;

        let event = OrderEvent::order_cancelled("Changed mind", None);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let summary = view.get_customer(customer_id).await.unwrap();
        assert_eq!(summary.active_orders, 0);
        assert_eq!(summary.cancelled_orders, 1);
        assert_eq!(summary.total_spent, Money::zero()); // Not spent
    }

    #[tokio::test]
    async fn test_multiple_customers() {
        let view = CustomerOrdersView::new();
        let customer1 = CustomerId::new();
        let customer2 = CustomerId::new();

        create_order_with_items(&view, AggregateId::new(), customer1).await;
        create_order_with_items(&view, AggregateId::new(), customer2).await;

        assert_eq!(view.get_all_customers().await.len(), 2);
    }

    #[tokio::test]
    async fn test_top_customers() {
        let view = CustomerOrdersView::new();
        let customer1 = CustomerId::new();
        let customer2 = CustomerId::new();

        // Customer 1: one order, $20
        let order1 = AggregateId::new();
        create_order_with_items(&view, order1, customer1).await;
        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order1, 3, &event))
            .await
            .unwrap();

        // Customer 2: one order, $50
        let order2 = AggregateId::new();
        let event = OrderEvent::order_created(order2, customer2);
        view.handle(&make_envelope(order2, 1, &event))
            .await
            .unwrap();
        let item = OrderItem::new("SKU-002", "Expensive", 1, Money::from_cents(5000));
        let event = OrderEvent::item_added(&item);
        view.handle(&make_envelope(order2, 2, &event))
            .await
            .unwrap();
        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order2, 3, &event))
            .await
            .unwrap();

        let top = view.get_top_customers(1).await;
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].customer_id, customer2);
        assert_eq!(top[0].total_spent.cents(), 5000);
    }

    #[tokio::test]
    async fn test_item_quantity_update_affects_total_spent() {
        let view = CustomerOrdersView::new();
        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id, customer_id).await;

        // Update quantity from 2 to 5
        let event = OrderEvent::item_quantity_updated(ProductId::new("SKU-001"), 2, 5);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order_id, 4, &event))
            .await
            .unwrap();

        let summary = view.get_customer(customer_id).await.unwrap();
        assert_eq!(summary.total_spent.cents(), 5000); // 5 x $10
    }

    #[tokio::test]
    async fn test_item_removed_affects_total_spent() {
        let view = CustomerOrdersView::new();
        let customer_id = CustomerId::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id, customer_id).await;

        // Remove item before completing
        let event = OrderEvent::item_removed(ProductId::new("SKU-001"));
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order_id, 4, &event))
            .await
            .unwrap();

        let summary = view.get_customer(customer_id).await.unwrap();
        assert_eq!(summary.total_spent, Money::zero());
    }

    #[tokio::test]
    async fn test_reset() {
        let view = CustomerOrdersView::new();
        let customer_id = CustomerId::new();

        create_order_with_items(&view, AggregateId::new(), customer_id).await;

        view.reset().await.unwrap();

        assert!(view.get_customer(customer_id).await.is_none());
        assert_eq!(view.get_all_customers().await.len(), 0);
        assert_eq!(view.position().await.events_processed, 0);
    }
}
