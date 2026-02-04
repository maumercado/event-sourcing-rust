//! Order aggregate implementation.

use std::collections::HashMap;

use common::AggregateId;
use event_store::Version;
use serde::{Deserialize, Serialize};

use crate::aggregate::{Aggregate, SnapshotCapable};

use super::{
    CustomerId, Money, OrderError, OrderEvent, OrderItem, OrderState, ProductId,
    events::{ItemAddedData, ItemQuantityUpdatedData, OrderCreatedData},
};

/// Order aggregate root.
///
/// Represents an order in the system with its full lifecycle from creation
/// to completion or cancellation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Order {
    /// Unique order identifier.
    id: Option<AggregateId>,

    /// Current version for optimistic concurrency.
    #[serde(default)]
    version: Version,

    /// Customer who placed the order.
    customer_id: Option<CustomerId>,

    /// Current state of the order.
    state: OrderState,

    /// Items in the order, keyed by product ID.
    items: HashMap<ProductId, OrderItem>,

    /// Total amount of the order.
    total_amount: Money,
}

impl Aggregate for Order {
    type Event = OrderEvent;
    type Error = OrderError;

    fn aggregate_type() -> &'static str {
        "Order"
    }

    fn id(&self) -> Option<AggregateId> {
        self.id
    }

    fn version(&self) -> Version {
        self.version
    }

    fn set_version(&mut self, version: Version) {
        self.version = version;
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            OrderEvent::OrderCreated(data) => self.apply_order_created(data),
            OrderEvent::ItemAdded(data) => self.apply_item_added(data),
            OrderEvent::ItemRemoved(data) => self.apply_item_removed(data.product_id),
            OrderEvent::ItemQuantityUpdated(data) => self.apply_item_quantity_updated(data),
            OrderEvent::OrderSubmitted(_) => {
                // State transition happens in OrderReserved
            }
            OrderEvent::OrderReserved(_) => {
                self.state = OrderState::Reserved;
            }
            OrderEvent::OrderProcessing(_) => {
                self.state = OrderState::Processing;
            }
            OrderEvent::OrderCompleted(_) => {
                self.state = OrderState::Completed;
            }
            OrderEvent::OrderCancelled(_) => {
                self.state = OrderState::Cancelled;
            }
        }
    }
}

impl SnapshotCapable for Order {
    fn snapshot_interval() -> usize {
        50 // Snapshot every 50 events
    }
}

// Query methods
impl Order {
    /// Returns the customer ID.
    pub fn customer_id(&self) -> Option<CustomerId> {
        self.customer_id
    }

    /// Returns the current state.
    pub fn state(&self) -> OrderState {
        self.state
    }

    /// Returns all items in the order.
    pub fn items(&self) -> impl Iterator<Item = &OrderItem> {
        self.items.values()
    }

    /// Returns an item by product ID.
    pub fn get_item(&self, product_id: &ProductId) -> Option<&OrderItem> {
        self.items.get(product_id)
    }

    /// Returns the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns the total quantity of all items.
    pub fn total_quantity(&self) -> u32 {
        self.items.values().map(|item| item.quantity).sum()
    }

    /// Returns the total amount.
    pub fn total_amount(&self) -> Money {
        self.total_amount
    }

    /// Returns true if the order has items.
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Returns true if the order is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }
}

// Command methods (return events)
impl Order {
    /// Creates a new order for a customer.
    pub fn create(
        &self,
        order_id: AggregateId,
        customer_id: CustomerId,
    ) -> Result<Vec<OrderEvent>, OrderError> {
        if self.id.is_some() {
            return Err(OrderError::AlreadyCreated);
        }

        Ok(vec![OrderEvent::order_created(order_id, customer_id)])
    }

    /// Adds an item to the order.
    ///
    /// If the item already exists, updates the quantity instead.
    pub fn add_item(&self, item: OrderItem) -> Result<Vec<OrderEvent>, OrderError> {
        if !self.state.can_modify_items() {
            return Err(OrderError::InvalidStateTransition {
                current_state: self.state,
                action: "add item",
            });
        }

        if item.quantity == 0 {
            return Err(OrderError::InvalidQuantity {
                quantity: item.quantity,
            });
        }

        if !item.unit_price.is_positive() {
            return Err(OrderError::InvalidPrice {
                price: item.unit_price.cents(),
            });
        }

        // Check if item already exists
        if let Some(existing) = self.items.get(&item.product_id) {
            let new_quantity = existing.quantity + item.quantity;
            Ok(vec![OrderEvent::item_quantity_updated(
                item.product_id,
                existing.quantity,
                new_quantity,
            )])
        } else {
            Ok(vec![OrderEvent::item_added(&item)])
        }
    }

    /// Removes an item from the order.
    pub fn remove_item(&self, product_id: ProductId) -> Result<Vec<OrderEvent>, OrderError> {
        if !self.state.can_modify_items() {
            return Err(OrderError::InvalidStateTransition {
                current_state: self.state,
                action: "remove item",
            });
        }

        if !self.items.contains_key(&product_id) {
            return Err(OrderError::ItemNotFound {
                product_id: product_id.to_string(),
            });
        }

        Ok(vec![OrderEvent::item_removed(product_id)])
    }

    /// Updates the quantity of an existing item.
    pub fn update_item_quantity(
        &self,
        product_id: ProductId,
        new_quantity: u32,
    ) -> Result<Vec<OrderEvent>, OrderError> {
        if !self.state.can_modify_items() {
            return Err(OrderError::InvalidStateTransition {
                current_state: self.state,
                action: "update item quantity",
            });
        }

        let existing = self
            .items
            .get(&product_id)
            .ok_or_else(|| OrderError::ItemNotFound {
                product_id: product_id.to_string(),
            })?;

        if new_quantity == 0 {
            // Remove the item if quantity is 0
            Ok(vec![OrderEvent::item_removed(product_id)])
        } else if new_quantity != existing.quantity {
            Ok(vec![OrderEvent::item_quantity_updated(
                product_id,
                existing.quantity,
                new_quantity,
            )])
        } else {
            // No change
            Ok(vec![])
        }
    }

    /// Submits the order for processing.
    pub fn submit(&self) -> Result<Vec<OrderEvent>, OrderError> {
        if !self.state.can_submit() {
            return Err(OrderError::InvalidStateTransition {
                current_state: self.state,
                action: "submit",
            });
        }

        if !self.has_items() {
            return Err(OrderError::NoItems);
        }

        Ok(vec![OrderEvent::order_submitted(
            self.total_amount,
            self.items.len(),
        )])
    }

    /// Marks inventory as reserved.
    pub fn mark_reserved(
        &self,
        reservation_id: Option<String>,
    ) -> Result<Vec<OrderEvent>, OrderError> {
        if !self.state.can_reserve() {
            return Err(OrderError::InvalidStateTransition {
                current_state: self.state,
                action: "mark reserved",
            });
        }

        Ok(vec![OrderEvent::order_reserved(reservation_id)])
    }

    /// Starts processing the order.
    pub fn start_processing(
        &self,
        payment_id: Option<String>,
    ) -> Result<Vec<OrderEvent>, OrderError> {
        if !self.state.can_start_processing() {
            return Err(OrderError::InvalidStateTransition {
                current_state: self.state,
                action: "start processing",
            });
        }

        Ok(vec![OrderEvent::order_processing(payment_id)])
    }

    /// Completes the order.
    pub fn complete(&self, tracking_number: Option<String>) -> Result<Vec<OrderEvent>, OrderError> {
        if !self.state.can_complete() {
            return Err(OrderError::InvalidStateTransition {
                current_state: self.state,
                action: "complete",
            });
        }

        Ok(vec![OrderEvent::order_completed(tracking_number)])
    }

    /// Cancels the order.
    pub fn cancel(
        &self,
        reason: impl Into<String>,
        cancelled_by: Option<String>,
    ) -> Result<Vec<OrderEvent>, OrderError> {
        if !self.state.can_cancel() {
            return Err(OrderError::InvalidStateTransition {
                current_state: self.state,
                action: "cancel",
            });
        }

        Ok(vec![OrderEvent::order_cancelled(reason, cancelled_by)])
    }
}

// Apply event helpers
impl Order {
    fn apply_order_created(&mut self, data: OrderCreatedData) {
        self.id = Some(data.order_id);
        self.customer_id = Some(data.customer_id);
        self.state = OrderState::Draft;
    }

    fn apply_item_added(&mut self, data: ItemAddedData) {
        let item = OrderItem::new(
            data.product_id.clone(),
            data.product_name,
            data.quantity,
            data.unit_price,
        );
        self.total_amount += item.total_price();
        self.items.insert(data.product_id, item);
    }

    fn apply_item_removed(&mut self, product_id: ProductId) {
        if let Some(item) = self.items.remove(&product_id) {
            self.total_amount -= item.total_price();
        }
    }

    fn apply_item_quantity_updated(&mut self, data: ItemQuantityUpdatedData) {
        if let Some(item) = self.items.get_mut(&data.product_id) {
            // Subtract old total
            self.total_amount -= item.total_price();

            // Update quantity
            item.quantity = data.new_quantity;

            // Add new total
            self.total_amount += item.total_price();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::{Aggregate, DomainEvent};

    fn create_order() -> (Order, AggregateId) {
        let mut order = Order::default();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();
        let events = order.create(order_id, customer_id).unwrap();
        order.apply_events(events);
        (order, order_id)
    }

    #[test]
    fn test_create_order() {
        let (order, order_id) = create_order();
        assert_eq!(order.id(), Some(order_id));
        assert!(order.customer_id().is_some());
        assert_eq!(order.state(), OrderState::Draft);
        assert!(!order.has_items());
    }

    #[test]
    fn test_create_order_twice_fails() {
        let (order, _) = create_order();
        let result = order.create(AggregateId::new(), CustomerId::new());
        assert!(matches!(result, Err(OrderError::AlreadyCreated)));
    }

    #[test]
    fn test_add_item() {
        let (mut order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));

        let events = order.add_item(item).unwrap();
        order.apply_events(events);

        assert_eq!(order.item_count(), 1);
        assert_eq!(order.total_amount().cents(), 2000);
    }

    #[test]
    fn test_add_same_item_increases_quantity() {
        let (mut order, _) = create_order();
        let item1 = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
        let item2 = OrderItem::new("SKU-001", "Widget", 3, Money::from_cents(1000));

        let events = order.add_item(item1).unwrap();
        order.apply_events(events);

        let events = order.add_item(item2).unwrap();
        order.apply_events(events);

        assert_eq!(order.item_count(), 1);
        let item = order.get_item(&ProductId::new("SKU-001")).unwrap();
        assert_eq!(item.quantity, 5);
        assert_eq!(order.total_amount().cents(), 5000);
    }

    #[test]
    fn test_add_item_zero_quantity_fails() {
        let (order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 0, Money::from_cents(1000));
        let result = order.add_item(item);
        assert!(matches!(result, Err(OrderError::InvalidQuantity { .. })));
    }

    #[test]
    fn test_add_item_zero_price_fails() {
        let (order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 1, Money::zero());
        let result = order.add_item(item);
        assert!(matches!(result, Err(OrderError::InvalidPrice { .. })));
    }

    #[test]
    fn test_remove_item() {
        let (mut order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));

        let events = order.add_item(item).unwrap();
        order.apply_events(events);

        let events = order.remove_item(ProductId::new("SKU-001")).unwrap();
        order.apply_events(events);

        assert_eq!(order.item_count(), 0);
        assert_eq!(order.total_amount().cents(), 0);
    }

    #[test]
    fn test_remove_nonexistent_item_fails() {
        let (order, _) = create_order();
        let result = order.remove_item(ProductId::new("SKU-999"));
        assert!(matches!(result, Err(OrderError::ItemNotFound { .. })));
    }

    #[test]
    fn test_update_item_quantity() {
        let (mut order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));

        let events = order.add_item(item).unwrap();
        order.apply_events(events);

        let events = order
            .update_item_quantity(ProductId::new("SKU-001"), 5)
            .unwrap();
        order.apply_events(events);

        let item = order.get_item(&ProductId::new("SKU-001")).unwrap();
        assert_eq!(item.quantity, 5);
        assert_eq!(order.total_amount().cents(), 5000);
    }

    #[test]
    fn test_update_item_quantity_to_zero_removes_item() {
        let (mut order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));

        let events = order.add_item(item).unwrap();
        order.apply_events(events);

        let events = order
            .update_item_quantity(ProductId::new("SKU-001"), 0)
            .unwrap();
        order.apply_events(events);

        assert_eq!(order.item_count(), 0);
    }

    #[test]
    fn test_submit_order() {
        let (mut order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));

        let events = order.add_item(item).unwrap();
        order.apply_events(events);

        let events = order.submit().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "OrderSubmitted");
    }

    #[test]
    fn test_submit_empty_order_fails() {
        let (order, _) = create_order();
        let result = order.submit();
        assert!(matches!(result, Err(OrderError::NoItems)));
    }

    #[test]
    fn test_full_order_lifecycle() {
        let (mut order, _) = create_order();

        // Add items
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
        order.apply_events(order.add_item(item).unwrap());

        // Submit
        order.apply_events(order.submit().unwrap());

        // Reserve
        let events = order.mark_reserved(Some("RES-123".to_string())).unwrap();
        order.apply_events(events);
        assert_eq!(order.state(), OrderState::Reserved);

        // Start processing
        let events = order.start_processing(Some("PAY-123".to_string())).unwrap();
        order.apply_events(events);
        assert_eq!(order.state(), OrderState::Processing);

        // Complete
        let events = order.complete(Some("TRACK-123".to_string())).unwrap();
        order.apply_events(events);
        assert_eq!(order.state(), OrderState::Completed);
        assert!(order.is_terminal());
    }

    #[test]
    fn test_cancel_order() {
        let (mut order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
        order.apply_events(order.add_item(item).unwrap());

        let events = order.cancel("Customer request", None).unwrap();
        order.apply_events(events);

        assert_eq!(order.state(), OrderState::Cancelled);
        assert!(order.is_terminal());
    }

    #[test]
    fn test_cannot_modify_after_reserved() {
        let (mut order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
        order.apply_events(order.add_item(item).unwrap());
        order.apply_events(order.submit().unwrap());
        order.apply_events(order.mark_reserved(None).unwrap());

        let item2 = OrderItem::new("SKU-002", "Gadget", 1, Money::from_cents(500));
        let result = order.add_item(item2);

        assert!(matches!(
            result,
            Err(OrderError::InvalidStateTransition { .. })
        ));
    }

    #[test]
    fn test_cannot_cancel_completed_order() {
        let (mut order, _) = create_order();
        let item = OrderItem::new("SKU-001", "Widget", 1, Money::from_cents(1000));
        order.apply_events(order.add_item(item).unwrap());
        order.apply_events(order.submit().unwrap());
        order.apply_events(order.mark_reserved(None).unwrap());
        order.apply_events(order.start_processing(None).unwrap());
        order.apply_events(order.complete(None).unwrap());

        let result = order.cancel("Too late", None);
        assert!(matches!(
            result,
            Err(OrderError::InvalidStateTransition { .. })
        ));
    }

    #[test]
    fn test_total_quantity() {
        let (mut order, _) = create_order();
        order.apply_events(
            order
                .add_item(OrderItem::new(
                    "SKU-001",
                    "Widget",
                    2,
                    Money::from_cents(1000),
                ))
                .unwrap(),
        );
        order.apply_events(
            order
                .add_item(OrderItem::new(
                    "SKU-002",
                    "Gadget",
                    3,
                    Money::from_cents(500),
                ))
                .unwrap(),
        );

        assert_eq!(order.total_quantity(), 5);
    }

    #[test]
    fn test_serialization() {
        let (mut order, order_id) = create_order();
        order.apply_events(
            order
                .add_item(OrderItem::new(
                    "SKU-001",
                    "Widget",
                    2,
                    Money::from_cents(1000),
                ))
                .unwrap(),
        );

        let json = serde_json::to_string(&order).unwrap();
        let deserialized: Order = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id(), Some(order_id));
        assert_eq!(deserialized.item_count(), 1);
        assert_eq!(deserialized.total_amount().cents(), 2000);
    }
}
