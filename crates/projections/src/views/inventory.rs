//! Inventory read model — product demand aggregated across orders.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use common::AggregateId;
use domain::{Money, OrderEvent, ProductId};
use event_store::EventEnvelope;
use tokio::sync::RwLock;

use crate::Result;
use crate::projection::{Projection, ProjectionPosition};
use crate::read_model::ReadModel;

/// Product demand summary aggregated across all orders.
#[derive(Debug, Clone)]
pub struct ProductDemand {
    pub product_id: ProductId,
    pub product_name: String,
    pub total_quantity_ordered: u64,
    pub quantity_in_active_orders: u64,
    pub quantity_reserved: u64,
    pub quantity_completed: u64,
    pub total_revenue: Money,
    pub order_count: u64,
}

/// Tracks the state of each order for proper accounting on terminal events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderStatus {
    Active,
    Reserved,
    Completed,
    Cancelled,
}

/// Internal state for the inventory view.
struct InventoryState {
    products: HashMap<ProductId, ProductDemand>,
    /// Per-order, per-product tracking: (quantity, unit_price).
    order_products: HashMap<AggregateId, HashMap<ProductId, (u32, Money)>>,
    /// Tracks which orders have which products for computing set membership.
    order_product_sets: HashMap<AggregateId, Vec<ProductId>>,
    /// Tracks order status for state transitions.
    order_status: HashMap<AggregateId, OrderStatus>,
    position: ProjectionPosition,
}

/// Read model view for product demand across orders.
///
/// Tracks how many units of each product are ordered, reserved, completed,
/// and the total revenue generated.
#[derive(Clone)]
pub struct InventoryView {
    state: Arc<RwLock<InventoryState>>,
}

impl InventoryView {
    /// Creates a new empty inventory view.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(InventoryState {
                products: HashMap::new(),
                order_products: HashMap::new(),
                order_product_sets: HashMap::new(),
                order_status: HashMap::new(),
                position: ProjectionPosition::zero(),
            })),
        }
    }

    /// Gets demand info for a specific product.
    pub async fn get_product(&self, product_id: &ProductId) -> Option<ProductDemand> {
        self.state.read().await.products.get(product_id).cloned()
    }

    /// Gets all products.
    pub async fn get_all_products(&self) -> Vec<ProductDemand> {
        self.state.read().await.products.values().cloned().collect()
    }

    /// Gets top products by total quantity ordered.
    pub async fn get_top_products_by_demand(&self, limit: usize) -> Vec<ProductDemand> {
        let state = self.state.read().await;
        let mut products: Vec<_> = state.products.values().cloned().collect();
        products.sort_by(|a, b| b.total_quantity_ordered.cmp(&a.total_quantity_ordered));
        products.truncate(limit);
        products
    }

    /// Gets top products by total revenue.
    pub async fn get_top_products_by_revenue(&self, limit: usize) -> Vec<ProductDemand> {
        let state = self.state.read().await;
        let mut products: Vec<_> = state.products.values().cloned().collect();
        products.sort_by(|a, b| b.total_revenue.cents().cmp(&a.total_revenue.cents()));
        products.truncate(limit);
        products
    }
}

impl Default for InventoryView {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Projection for InventoryView {
    fn name(&self) -> &'static str {
        "InventoryView"
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
            OrderEvent::OrderCreated(_) => {
                state.order_products.insert(order_id, HashMap::new());
                state.order_product_sets.insert(order_id, Vec::new());
                state.order_status.insert(order_id, OrderStatus::Active);
            }
            OrderEvent::ItemAdded(data) => {
                let product_id = data.product_id.clone();

                // Track per-order
                state
                    .order_products
                    .entry(order_id)
                    .or_default()
                    .insert(product_id.clone(), (data.quantity, data.unit_price));

                // Track product sets for order_count
                if let Some(set) = state.order_product_sets.get_mut(&order_id)
                    && !set.contains(&product_id)
                {
                    set.push(product_id.clone());
                }

                // Update product demand
                let demand = state
                    .products
                    .entry(product_id.clone())
                    .or_insert(ProductDemand {
                        product_id,
                        product_name: data.product_name.clone(),
                        total_quantity_ordered: 0,
                        quantity_in_active_orders: 0,
                        quantity_reserved: 0,
                        quantity_completed: 0,
                        total_revenue: Money::zero(),
                        order_count: 0,
                    });
                demand.total_quantity_ordered += data.quantity as u64;
                demand.quantity_in_active_orders += data.quantity as u64;
                demand.order_count += 1;
            }
            OrderEvent::ItemRemoved(data) => {
                let order_status = state
                    .order_status
                    .get(&order_id)
                    .copied()
                    .unwrap_or(OrderStatus::Active);

                let removed = state
                    .order_products
                    .get_mut(&order_id)
                    .and_then(|m| m.remove(&data.product_id));

                if let Some((qty, _price)) = removed
                    && let Some(demand) = state.products.get_mut(&data.product_id)
                {
                    demand.total_quantity_ordered =
                        demand.total_quantity_ordered.saturating_sub(qty as u64);
                    match order_status {
                        OrderStatus::Active => {
                            demand.quantity_in_active_orders =
                                demand.quantity_in_active_orders.saturating_sub(qty as u64);
                        }
                        OrderStatus::Reserved => {
                            demand.quantity_reserved =
                                demand.quantity_reserved.saturating_sub(qty as u64);
                        }
                        _ => {}
                    }
                    demand.order_count = demand.order_count.saturating_sub(1);
                }

                // Remove from product set
                if let Some(set) = state.order_product_sets.get_mut(&order_id) {
                    set.retain(|p| *p != data.product_id);
                }
            }
            OrderEvent::ItemQuantityUpdated(data) => {
                let order_status = state
                    .order_status
                    .get(&order_id)
                    .copied()
                    .unwrap_or(OrderStatus::Active);

                let old_qty = state.order_products.get_mut(&order_id).and_then(|m| {
                    m.get_mut(&data.product_id).map(|entry| {
                        let old = entry.0;
                        entry.0 = data.new_quantity;
                        old
                    })
                });

                if let Some(old_qty) = old_qty
                    && let Some(demand) = state.products.get_mut(&data.product_id)
                {
                    demand.total_quantity_ordered =
                        (demand.total_quantity_ordered as i64 + data.new_quantity as i64
                            - old_qty as i64) as u64;

                    match order_status {
                        OrderStatus::Active => {
                            demand.quantity_in_active_orders =
                                (demand.quantity_in_active_orders as i64 + data.new_quantity as i64
                                    - old_qty as i64) as u64;
                        }
                        OrderStatus::Reserved => {
                            demand.quantity_reserved =
                                (demand.quantity_reserved as i64 + data.new_quantity as i64
                                    - old_qty as i64) as u64;
                        }
                        _ => {}
                    }
                }
            }
            OrderEvent::OrderReserved(_) => {
                state.order_status.insert(order_id, OrderStatus::Reserved);

                // Clone the order products to avoid borrow conflict
                let order_items: Vec<_> = state
                    .order_products
                    .get(&order_id)
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
                    .unwrap_or_default();

                for (product_id, (qty, _price)) in order_items {
                    if let Some(demand) = state.products.get_mut(&product_id) {
                        demand.quantity_in_active_orders =
                            demand.quantity_in_active_orders.saturating_sub(qty as u64);
                        demand.quantity_reserved += qty as u64;
                    }
                }
            }
            OrderEvent::OrderCompleted(_) => {
                let prev_status = state
                    .order_status
                    .get(&order_id)
                    .copied()
                    .unwrap_or(OrderStatus::Active);
                state.order_status.insert(order_id, OrderStatus::Completed);

                let order_items: Vec<_> = state
                    .order_products
                    .get(&order_id)
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
                    .unwrap_or_default();

                for (product_id, (qty, unit_price)) in order_items {
                    if let Some(demand) = state.products.get_mut(&product_id) {
                        match prev_status {
                            OrderStatus::Active => {
                                demand.quantity_in_active_orders =
                                    demand.quantity_in_active_orders.saturating_sub(qty as u64);
                            }
                            OrderStatus::Reserved => {
                                demand.quantity_reserved =
                                    demand.quantity_reserved.saturating_sub(qty as u64);
                            }
                            _ => {}
                        }
                        demand.quantity_completed += qty as u64;
                        demand.total_revenue = demand.total_revenue.add(unit_price.multiply(qty));
                    }
                }
            }
            OrderEvent::OrderCancelled(_) => {
                let prev_status = state
                    .order_status
                    .get(&order_id)
                    .copied()
                    .unwrap_or(OrderStatus::Active);
                state.order_status.insert(order_id, OrderStatus::Cancelled);

                let order_items: Vec<_> = state
                    .order_products
                    .get(&order_id)
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
                    .unwrap_or_default();

                for (product_id, (qty, _price)) in order_items {
                    if let Some(demand) = state.products.get_mut(&product_id) {
                        demand.total_quantity_ordered =
                            demand.total_quantity_ordered.saturating_sub(qty as u64);
                        match prev_status {
                            OrderStatus::Active => {
                                demand.quantity_in_active_orders =
                                    demand.quantity_in_active_orders.saturating_sub(qty as u64);
                            }
                            OrderStatus::Reserved => {
                                demand.quantity_reserved =
                                    demand.quantity_reserved.saturating_sub(qty as u64);
                            }
                            _ => {}
                        }
                        demand.order_count = demand.order_count.saturating_sub(1);
                    }
                }
            }
            // Submitted and Processing don't change inventory
            OrderEvent::OrderSubmitted(_) | OrderEvent::OrderProcessing(_) => {}
        }

        state.position = state.position.advance();
        Ok(())
    }

    async fn position(&self) -> ProjectionPosition {
        self.state.read().await.position
    }

    async fn reset(&self) -> Result<()> {
        let mut state = self.state.write().await;
        state.products.clear();
        state.order_products.clear();
        state.order_product_sets.clear();
        state.order_status.clear();
        state.position = ProjectionPosition::zero();
        Ok(())
    }
}

impl ReadModel for InventoryView {
    fn name(&self) -> &'static str {
        "InventoryView"
    }

    fn count(&self) -> usize {
        self.state.try_read().map(|s| s.products.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{CustomerId, DomainEvent, OrderItem};

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

    async fn create_order_with_items(view: &InventoryView, order_id: AggregateId) {
        let event = OrderEvent::order_created(order_id, CustomerId::new());
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
    async fn test_item_added_creates_product_demand() {
        let view = InventoryView::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id).await;

        let demand = view.get_product(&ProductId::new("SKU-001")).await.unwrap();
        assert_eq!(demand.product_name, "Widget");
        assert_eq!(demand.total_quantity_ordered, 2);
        assert_eq!(demand.quantity_in_active_orders, 2);
        assert_eq!(demand.quantity_reserved, 0);
        assert_eq!(demand.quantity_completed, 0);
        assert_eq!(demand.order_count, 1);
        assert_eq!(demand.total_revenue, Money::zero());
    }

    #[tokio::test]
    async fn test_multiple_orders_same_product() {
        let view = InventoryView::new();

        let order1 = AggregateId::new();
        let order2 = AggregateId::new();

        create_order_with_items(&view, order1).await;
        create_order_with_items(&view, order2).await;

        let demand = view.get_product(&ProductId::new("SKU-001")).await.unwrap();
        assert_eq!(demand.total_quantity_ordered, 4);
        assert_eq!(demand.quantity_in_active_orders, 4);
        assert_eq!(demand.order_count, 2);
    }

    #[tokio::test]
    async fn test_reserved_moves_quantities() {
        let view = InventoryView::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id).await;

        let event = OrderEvent::order_reserved(None);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let demand = view.get_product(&ProductId::new("SKU-001")).await.unwrap();
        assert_eq!(demand.quantity_in_active_orders, 0);
        assert_eq!(demand.quantity_reserved, 2);
        assert_eq!(demand.total_quantity_ordered, 2);
    }

    #[tokio::test]
    async fn test_completed_updates_revenue() {
        let view = InventoryView::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id).await;

        // Reserve then complete
        let event = OrderEvent::order_reserved(None);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order_id, 4, &event))
            .await
            .unwrap();

        let demand = view.get_product(&ProductId::new("SKU-001")).await.unwrap();
        assert_eq!(demand.quantity_reserved, 0);
        assert_eq!(demand.quantity_completed, 2);
        assert_eq!(demand.total_revenue.cents(), 2000);
        assert_eq!(demand.total_quantity_ordered, 2);
    }

    #[tokio::test]
    async fn test_cancelled_removes_demand() {
        let view = InventoryView::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id).await;

        let event = OrderEvent::order_cancelled("Out of stock", None);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let demand = view.get_product(&ProductId::new("SKU-001")).await.unwrap();
        assert_eq!(demand.total_quantity_ordered, 0);
        assert_eq!(demand.quantity_in_active_orders, 0);
        assert_eq!(demand.order_count, 0);
    }

    #[tokio::test]
    async fn test_quantity_update() {
        let view = InventoryView::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id).await;

        let event = OrderEvent::item_quantity_updated(ProductId::new("SKU-001"), 2, 5);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let demand = view.get_product(&ProductId::new("SKU-001")).await.unwrap();
        assert_eq!(demand.total_quantity_ordered, 5);
        assert_eq!(demand.quantity_in_active_orders, 5);
    }

    #[tokio::test]
    async fn test_item_removed() {
        let view = InventoryView::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id).await;

        let event = OrderEvent::item_removed(ProductId::new("SKU-001"));
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let demand = view.get_product(&ProductId::new("SKU-001")).await.unwrap();
        assert_eq!(demand.total_quantity_ordered, 0);
        assert_eq!(demand.quantity_in_active_orders, 0);
        assert_eq!(demand.order_count, 0);
    }

    #[tokio::test]
    async fn test_top_products_by_demand() {
        let view = InventoryView::new();
        let order_id = AggregateId::new();

        let event = OrderEvent::order_created(order_id, CustomerId::new());
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        let item1 = OrderItem::new("SKU-001", "Widget", 10, Money::from_cents(1000));
        let event = OrderEvent::item_added(&item1);
        view.handle(&make_envelope(order_id, 2, &event))
            .await
            .unwrap();

        let item2 = OrderItem::new("SKU-002", "Gadget", 5, Money::from_cents(2000));
        let event = OrderEvent::item_added(&item2);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        let top = view.get_top_products_by_demand(1).await;
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].product_id, ProductId::new("SKU-001"));
        assert_eq!(top[0].total_quantity_ordered, 10);
    }

    #[tokio::test]
    async fn test_top_products_by_revenue() {
        let view = InventoryView::new();
        let order_id = AggregateId::new();

        let event = OrderEvent::order_created(order_id, CustomerId::new());
        view.handle(&make_envelope(order_id, 1, &event))
            .await
            .unwrap();

        // SKU-001: 10 x $10 = $100
        let item1 = OrderItem::new("SKU-001", "Widget", 10, Money::from_cents(1000));
        let event = OrderEvent::item_added(&item1);
        view.handle(&make_envelope(order_id, 2, &event))
            .await
            .unwrap();

        // SKU-002: 5 x $30 = $150
        let item2 = OrderItem::new("SKU-002", "Gadget", 5, Money::from_cents(3000));
        let event = OrderEvent::item_added(&item2);
        view.handle(&make_envelope(order_id, 3, &event))
            .await
            .unwrap();

        // Complete to generate revenue
        let event = OrderEvent::order_completed(None);
        view.handle(&make_envelope(order_id, 4, &event))
            .await
            .unwrap();

        let top = view.get_top_products_by_revenue(1).await;
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].product_id, ProductId::new("SKU-002"));
        assert_eq!(top[0].total_revenue.cents(), 15000);
    }

    #[tokio::test]
    async fn test_reset() {
        let view = InventoryView::new();
        let order_id = AggregateId::new();

        create_order_with_items(&view, order_id).await;
        assert_eq!(view.get_all_products().await.len(), 1);

        view.reset().await.unwrap();

        assert_eq!(view.get_all_products().await.len(), 0);
        assert_eq!(view.position().await.events_processed, 0);
    }
}
