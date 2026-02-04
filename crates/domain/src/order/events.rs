//! Order domain events.

use chrono::{DateTime, Utc};
use common::AggregateId;
use serde::{Deserialize, Serialize};

use crate::aggregate::DomainEvent;

use super::{CustomerId, Money, OrderItem, ProductId};

/// Events that can occur on an order aggregate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum OrderEvent {
    /// Order was created.
    OrderCreated(OrderCreatedData),

    /// Item was added to the order.
    ItemAdded(ItemAddedData),

    /// Item was removed from the order.
    ItemRemoved(ItemRemovedData),

    /// Item quantity was updated.
    ItemQuantityUpdated(ItemQuantityUpdatedData),

    /// Order was submitted for processing.
    OrderSubmitted(OrderSubmittedData),

    /// Inventory was reserved for the order.
    OrderReserved(OrderReservedData),

    /// Order payment was confirmed and processing started.
    OrderProcessing(OrderProcessingData),

    /// Order was completed/shipped.
    OrderCompleted(OrderCompletedData),

    /// Order was cancelled.
    OrderCancelled(OrderCancelledData),
}

impl DomainEvent for OrderEvent {
    fn event_type(&self) -> &'static str {
        match self {
            OrderEvent::OrderCreated(_) => "OrderCreated",
            OrderEvent::ItemAdded(_) => "ItemAdded",
            OrderEvent::ItemRemoved(_) => "ItemRemoved",
            OrderEvent::ItemQuantityUpdated(_) => "ItemQuantityUpdated",
            OrderEvent::OrderSubmitted(_) => "OrderSubmitted",
            OrderEvent::OrderReserved(_) => "OrderReserved",
            OrderEvent::OrderProcessing(_) => "OrderProcessing",
            OrderEvent::OrderCompleted(_) => "OrderCompleted",
            OrderEvent::OrderCancelled(_) => "OrderCancelled",
        }
    }
}

/// Data for OrderCreated event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedData {
    /// The unique order ID.
    pub order_id: AggregateId,

    /// The customer who created the order.
    pub customer_id: CustomerId,

    /// When the order was created.
    pub created_at: DateTime<Utc>,
}

/// Data for ItemAdded event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemAddedData {
    /// The product that was added.
    pub product_id: ProductId,

    /// Product name.
    pub product_name: String,

    /// Quantity added.
    pub quantity: u32,

    /// Unit price at the time of adding.
    pub unit_price: Money,
}

/// Data for ItemRemoved event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRemovedData {
    /// The product that was removed.
    pub product_id: ProductId,
}

/// Data for ItemQuantityUpdated event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemQuantityUpdatedData {
    /// The product whose quantity was updated.
    pub product_id: ProductId,

    /// Previous quantity.
    pub old_quantity: u32,

    /// New quantity.
    pub new_quantity: u32,
}

/// Data for OrderSubmitted event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSubmittedData {
    /// When the order was submitted.
    pub submitted_at: DateTime<Utc>,

    /// Total amount at submission time.
    pub total_amount: Money,

    /// Number of items in the order.
    pub item_count: usize,
}

/// Data for OrderReserved event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderReservedData {
    /// When the inventory was reserved.
    pub reserved_at: DateTime<Utc>,

    /// Reservation reference ID (from inventory service).
    pub reservation_id: Option<String>,
}

/// Data for OrderProcessing event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderProcessingData {
    /// When processing started.
    pub started_at: DateTime<Utc>,

    /// Payment reference ID.
    pub payment_id: Option<String>,
}

/// Data for OrderCompleted event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCompletedData {
    /// When the order was completed.
    pub completed_at: DateTime<Utc>,

    /// Shipment tracking number.
    pub tracking_number: Option<String>,
}

/// Data for OrderCancelled event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCancelledData {
    /// When the order was cancelled.
    pub cancelled_at: DateTime<Utc>,

    /// Reason for cancellation.
    pub reason: String,

    /// Who cancelled the order.
    pub cancelled_by: Option<String>,
}

// Convenience constructors for events
impl OrderEvent {
    /// Creates an OrderCreated event.
    pub fn order_created(order_id: AggregateId, customer_id: CustomerId) -> Self {
        OrderEvent::OrderCreated(OrderCreatedData {
            order_id,
            customer_id,
            created_at: Utc::now(),
        })
    }

    /// Creates an ItemAdded event.
    pub fn item_added(item: &OrderItem) -> Self {
        OrderEvent::ItemAdded(ItemAddedData {
            product_id: item.product_id.clone(),
            product_name: item.product_name.clone(),
            quantity: item.quantity,
            unit_price: item.unit_price,
        })
    }

    /// Creates an ItemRemoved event.
    pub fn item_removed(product_id: ProductId) -> Self {
        OrderEvent::ItemRemoved(ItemRemovedData { product_id })
    }

    /// Creates an ItemQuantityUpdated event.
    pub fn item_quantity_updated(
        product_id: ProductId,
        old_quantity: u32,
        new_quantity: u32,
    ) -> Self {
        OrderEvent::ItemQuantityUpdated(ItemQuantityUpdatedData {
            product_id,
            old_quantity,
            new_quantity,
        })
    }

    /// Creates an OrderSubmitted event.
    pub fn order_submitted(total_amount: Money, item_count: usize) -> Self {
        OrderEvent::OrderSubmitted(OrderSubmittedData {
            submitted_at: Utc::now(),
            total_amount,
            item_count,
        })
    }

    /// Creates an OrderReserved event.
    pub fn order_reserved(reservation_id: Option<String>) -> Self {
        OrderEvent::OrderReserved(OrderReservedData {
            reserved_at: Utc::now(),
            reservation_id,
        })
    }

    /// Creates an OrderProcessing event.
    pub fn order_processing(payment_id: Option<String>) -> Self {
        OrderEvent::OrderProcessing(OrderProcessingData {
            started_at: Utc::now(),
            payment_id,
        })
    }

    /// Creates an OrderCompleted event.
    pub fn order_completed(tracking_number: Option<String>) -> Self {
        OrderEvent::OrderCompleted(OrderCompletedData {
            completed_at: Utc::now(),
            tracking_number,
        })
    }

    /// Creates an OrderCancelled event.
    pub fn order_cancelled(reason: impl Into<String>, cancelled_by: Option<String>) -> Self {
        OrderEvent::OrderCancelled(OrderCancelledData {
            cancelled_at: Utc::now(),
            reason: reason.into(),
            cancelled_by,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        let event = OrderEvent::order_created(order_id, customer_id);
        assert_eq!(event.event_type(), "OrderCreated");

        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
        let event = OrderEvent::item_added(&item);
        assert_eq!(event.event_type(), "ItemAdded");

        let event = OrderEvent::item_removed(ProductId::new("SKU-001"));
        assert_eq!(event.event_type(), "ItemRemoved");

        let event = OrderEvent::item_quantity_updated(ProductId::new("SKU-001"), 1, 3);
        assert_eq!(event.event_type(), "ItemQuantityUpdated");

        let event = OrderEvent::order_submitted(Money::from_cents(2000), 2);
        assert_eq!(event.event_type(), "OrderSubmitted");

        let event = OrderEvent::order_reserved(Some("RES-123".to_string()));
        assert_eq!(event.event_type(), "OrderReserved");

        let event = OrderEvent::order_processing(Some("PAY-123".to_string()));
        assert_eq!(event.event_type(), "OrderProcessing");

        let event = OrderEvent::order_completed(Some("TRACK-123".to_string()));
        assert_eq!(event.event_type(), "OrderCompleted");

        let event = OrderEvent::order_cancelled("Customer request", None);
        assert_eq!(event.event_type(), "OrderCancelled");
    }

    #[test]
    fn test_event_serialization() {
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();
        let event = OrderEvent::order_created(order_id, customer_id);

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("OrderCreated"));

        let deserialized: OrderEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type(), "OrderCreated");

        if let OrderEvent::OrderCreated(data) = deserialized {
            assert_eq!(data.order_id, order_id);
            assert_eq!(data.customer_id, customer_id);
        } else {
            panic!("Expected OrderCreated event");
        }
    }

    #[test]
    fn test_item_added_serialization() {
        let item = OrderItem::new("SKU-001", "Widget", 3, Money::from_cents(1500));
        let event = OrderEvent::item_added(&item);

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: OrderEvent = serde_json::from_str(&json).unwrap();

        if let OrderEvent::ItemAdded(data) = deserialized {
            assert_eq!(data.product_id.as_str(), "SKU-001");
            assert_eq!(data.product_name, "Widget");
            assert_eq!(data.quantity, 3);
            assert_eq!(data.unit_price.cents(), 1500);
        } else {
            panic!("Expected ItemAdded event");
        }
    }

    #[test]
    fn test_order_cancelled_serialization() {
        let event = OrderEvent::order_cancelled("Out of stock", Some("system".to_string()));

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: OrderEvent = serde_json::from_str(&json).unwrap();

        if let OrderEvent::OrderCancelled(data) = deserialized {
            assert_eq!(data.reason, "Out of stock");
            assert_eq!(data.cancelled_by, Some("system".to_string()));
        } else {
            panic!("Expected OrderCancelled event");
        }
    }
}
