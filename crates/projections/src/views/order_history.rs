//! Order history read model — completed and cancelled orders.

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

/// An item in a historical order.
#[derive(Debug, Clone)]
pub struct HistoryItemSummary {
    pub product_id: ProductId,
    pub product_name: String,
    pub quantity: u32,
    pub unit_price: Money,
}

/// Summary of a completed or cancelled order.
#[derive(Debug, Clone)]
pub struct OrderHistorySummary {
    pub order_id: AggregateId,
    pub customer_id: CustomerId,
    pub state: OrderState,
    pub item_count: usize,
    pub total_amount: Money,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub tracking_number: Option<String>,
    pub cancellation_reason: Option<String>,
    pub items: HashMap<ProductId, HistoryItemSummary>,
}

/// Staging data for an order being built up before it reaches terminal state.
#[derive(Debug, Clone)]
struct StagingOrder {
    customer_id: CustomerId,
    created_at: DateTime<Utc>,
    items: HashMap<ProductId, HistoryItemSummary>,
}

impl StagingOrder {
    fn total_amount(&self) -> Money {
        self.items.values().fold(Money::zero(), |acc, item| {
            acc + item.unit_price.multiply(item.quantity)
        })
    }
}

/// Internal state for the order history view.
struct OrderHistoryState {
    staging: HashMap<AggregateId, StagingOrder>,
    history: HashMap<AggregateId, OrderHistorySummary>,
    position: ProjectionPosition,
}

/// Read model view for completed and cancelled orders.
///
/// Orders are staged while in progress and moved to history when they
/// reach a terminal state (Completed or Cancelled).
#[derive(Clone)]
pub struct OrderHistoryView {
    state: Arc<RwLock<OrderHistoryState>>,
}

impl OrderHistoryView {
    /// Creates a new empty order history view.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(OrderHistoryState {
                staging: HashMap::new(),
                history: HashMap::new(),
                position: ProjectionPosition::zero(),
            })),
        }
    }

    /// Gets a specific historical order.
    pub async fn get_order(&self, order_id: AggregateId) -> Option<OrderHistorySummary> {
        self.state.read().await.history.get(&order_id).cloned()
    }

    /// Gets all historical orders.
    pub async fn get_all_history(&self) -> Vec<OrderHistorySummary> {
        self.state.read().await.history.values().cloned().collect()
    }

    /// Gets all completed orders.
    pub async fn get_completed_orders(&self) -> Vec<OrderHistorySummary> {
        self.state
            .read()
            .await
            .history
            .values()
            .filter(|o| o.state == OrderState::Completed)
            .cloned()
            .collect()
    }

    /// Gets all cancelled orders.
    pub async fn get_cancelled_orders(&self) -> Vec<OrderHistorySummary> {
        self.state
            .read()
            .await
            .history
            .values()
            .filter(|o| o.state == OrderState::Cancelled)
            .cloned()
            .collect()
    }

    /// Gets historical orders for a specific customer.
    pub async fn get_history_by_customer(
        &self,
        customer_id: CustomerId,
    ) -> Vec<OrderHistorySummary> {
        self.state
            .read()
            .await
            .history
            .values()
            .filter(|o| o.customer_id == customer_id)
            .cloned()
            .collect()
    }
}

impl Default for OrderHistoryView {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Projection for OrderHistoryView {
    fn name(&self) -> &'static str {
        "OrderHistoryView"
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
                state.staging.insert(
                    order_id,
                    StagingOrder {
                        customer_id: data.customer_id,
                        created_at: data.created_at,
                        items: HashMap::new(),
                    },
                );
            }
            OrderEvent::ItemAdded(data) => {
                if let Some(staging) = state.staging.get_mut(&order_id) {
                    staging.items.insert(
                        data.product_id.clone(),
                        HistoryItemSummary {
                            product_id: data.product_id,
                            product_name: data.product_name,
                            quantity: data.quantity,
                            unit_price: data.unit_price,
                        },
                    );
                }
            }
            OrderEvent::ItemRemoved(data) => {
                if let Some(staging) = state.staging.get_mut(&order_id) {
                    staging.items.remove(&data.product_id);
                }
            }
            OrderEvent::ItemQuantityUpdated(data) => {
                if let Some(staging) = state.staging.get_mut(&order_id)
                    && let Some(item) = staging.items.get_mut(&data.product_id)
                {
                    item.quantity = data.new_quantity;
                }
            }
            OrderEvent::OrderCompleted(data) => {
                if let Some(staging) = state.staging.remove(&order_id) {
                    let total_amount = staging.total_amount();
                    state.history.insert(
                        order_id,
                        OrderHistorySummary {
                            order_id,
                            customer_id: staging.customer_id,
                            state: OrderState::Completed,
                            item_count: staging.items.len(),
                            total_amount,
                            created_at: staging.created_at,
                            completed_at: Some(data.completed_at),
                            cancelled_at: None,
                            tracking_number: data.tracking_number,
                            cancellation_reason: None,
                            items: staging.items,
                        },
                    );
                }
            }
            OrderEvent::OrderCancelled(data) => {
                if let Some(staging) = state.staging.remove(&order_id) {
                    let total_amount = staging.total_amount();
                    state.history.insert(
                        order_id,
                        OrderHistorySummary {
                            order_id,
                            customer_id: staging.customer_id,
                            state: OrderState::Cancelled,
                            item_count: staging.items.len(),
                            total_amount,
                            created_at: staging.created_at,
                            completed_at: None,
                            cancelled_at: Some(data.cancelled_at),
                            tracking_number: None,
                            cancellation_reason: Some(data.reason),
                            items: staging.items,
                        },
                    );
                }
            }
            // State transitions don't affect history staging
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
        state.staging.clear();
        state.history.clear();
        state.position = ProjectionPosition::zero();
        Ok(())
    }
}

impl ReadModel for OrderHistoryView {
    fn name(&self) -> &'static str {
        "OrderHistoryView"
    }

    fn count(&self) -> usize {
        self.state.try_read().map(|s| s.history.len()).unwrap_or(0)
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
        view: &OrderHistoryView,
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
    async fn test_completed_order_appears_in_history() {
        let view = OrderHistoryView::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        create_order_with_items(&view, order_id, customer_id).await;

        // Complete the order
        let event = OrderEvent::order_completed(Some("TRACK-123".to_string()));
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let history = view.get_order(order_id).await.unwrap();
        assert_eq!(history.state, OrderState::Completed);
        assert_eq!(history.customer_id, customer_id);
        assert_eq!(history.item_count, 1);
        assert_eq!(history.total_amount.cents(), 2000);
        assert_eq!(history.tracking_number, Some("TRACK-123".to_string()));
        assert!(history.completed_at.is_some());
        assert!(history.cancelled_at.is_none());
    }

    #[tokio::test]
    async fn test_cancelled_order_appears_in_history() {
        let view = OrderHistoryView::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        create_order_with_items(&view, order_id, customer_id).await;

        let event = OrderEvent::order_cancelled("Out of stock", Some("system".to_string()));
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let history = view.get_order(order_id).await.unwrap();
        assert_eq!(history.state, OrderState::Cancelled);
        assert_eq!(
            history.cancellation_reason,
            Some("Out of stock".to_string())
        );
        assert!(history.cancelled_at.is_some());
        assert!(history.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_active_order_not_in_history() {
        let view = OrderHistoryView::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        create_order_with_items(&view, order_id, customer_id).await;

        assert!(view.get_order(order_id).await.is_none());
        assert_eq!(view.get_all_history().await.len(), 0);
    }

    #[tokio::test]
    async fn test_filter_completed_and_cancelled() {
        let view = OrderHistoryView::new();
        let customer_id = CustomerId::new();

        let order1 = AggregateId::new();
        let order2 = AggregateId::new();

        create_order_with_items(&view, order1, customer_id).await;
        create_order_with_items(&view, order2, customer_id).await;

        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order1, 3, &event))
            .await
            .unwrap();

        let event = OrderEvent::order_cancelled("Cancelled", None);
        view.handle(&make_envelope(order2, 3, &event))
            .await
            .unwrap();

        assert_eq!(view.get_completed_orders().await.len(), 1);
        assert_eq!(view.get_cancelled_orders().await.len(), 1);
        assert_eq!(view.get_all_history().await.len(), 2);
    }

    #[tokio::test]
    async fn test_filter_by_customer() {
        let view = OrderHistoryView::new();
        let customer1 = CustomerId::new();
        let customer2 = CustomerId::new();

        let order1 = AggregateId::new();
        let order2 = AggregateId::new();

        create_order_with_items(&view, order1, customer1).await;
        create_order_with_items(&view, order2, customer2).await;

        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order1, 3, &event))
            .await
            .unwrap();
        view.handle(&make_envelope(order2, 3, &event))
            .await
            .unwrap();

        let c1_history = view.get_history_by_customer(customer1).await;
        assert_eq!(c1_history.len(), 1);
        assert_eq!(c1_history[0].order_id, order1);
    }

    #[tokio::test]
    async fn test_reset() {
        let view = OrderHistoryView::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id, CustomerId::new()).await;

        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        assert_eq!(view.get_all_history().await.len(), 1);

        view.reset().await.unwrap();

        assert_eq!(view.get_all_history().await.len(), 0);
        assert_eq!(view.position().await.events_processed, 0);
    }

    #[tokio::test]
    async fn test_item_quantity_update_reflected_in_history() {
        let view = OrderHistoryView::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        let event = OrderEvent::order_created(order_id, customer_id);
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
        let event = OrderEvent::item_added(&item);
        view.handle(&make_envelope(order_id, 2, &event))
            .await
            .unwrap();

        let event = OrderEvent::item_quantity_updated(ProductId::new("SKU-001"), 2, 5);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order_id, 4, &event))
            .await
            .unwrap();

        let history = view.get_order(order_id).await.unwrap();
        assert_eq!(history.total_amount.cents(), 5000);
    }
}
