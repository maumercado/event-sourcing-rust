//! Order commands.

use common::AggregateId;

use crate::command::Command;

use super::{CustomerId, Money, Order, OrderItem, ProductId};

/// Command to create a new order.
#[derive(Debug, Clone)]
pub struct CreateOrder {
    /// The order ID to create.
    pub order_id: AggregateId,

    /// The customer placing the order.
    pub customer_id: CustomerId,
}

impl CreateOrder {
    /// Creates a new CreateOrder command.
    pub fn new(order_id: AggregateId, customer_id: CustomerId) -> Self {
        Self {
            order_id,
            customer_id,
        }
    }

    /// Creates a new CreateOrder command with a generated order ID.
    pub fn for_customer(customer_id: CustomerId) -> Self {
        Self {
            order_id: AggregateId::new(),
            customer_id,
        }
    }
}

impl Command for CreateOrder {
    type Aggregate = Order;

    fn aggregate_id(&self) -> AggregateId {
        self.order_id
    }
}

/// Command to add an item to an order.
#[derive(Debug, Clone)]
pub struct AddItem {
    /// The order to add the item to.
    pub order_id: AggregateId,

    /// The item to add.
    pub item: OrderItem,
}

impl AddItem {
    /// Creates a new AddItem command.
    pub fn new(order_id: AggregateId, item: OrderItem) -> Self {
        Self { order_id, item }
    }

    /// Creates a new AddItem command from individual fields.
    pub fn with_details(
        order_id: AggregateId,
        product_id: impl Into<ProductId>,
        product_name: impl Into<String>,
        quantity: u32,
        unit_price: Money,
    ) -> Self {
        Self {
            order_id,
            item: OrderItem::new(product_id, product_name, quantity, unit_price),
        }
    }
}

impl Command for AddItem {
    type Aggregate = Order;

    fn aggregate_id(&self) -> AggregateId {
        self.order_id
    }
}

/// Command to remove an item from an order.
#[derive(Debug, Clone)]
pub struct RemoveItem {
    /// The order to remove the item from.
    pub order_id: AggregateId,

    /// The product to remove.
    pub product_id: ProductId,
}

impl RemoveItem {
    /// Creates a new RemoveItem command.
    pub fn new(order_id: AggregateId, product_id: impl Into<ProductId>) -> Self {
        Self {
            order_id,
            product_id: product_id.into(),
        }
    }
}

impl Command for RemoveItem {
    type Aggregate = Order;

    fn aggregate_id(&self) -> AggregateId {
        self.order_id
    }
}

/// Command to update the quantity of an item.
#[derive(Debug, Clone)]
pub struct UpdateItemQuantity {
    /// The order containing the item.
    pub order_id: AggregateId,

    /// The product to update.
    pub product_id: ProductId,

    /// The new quantity.
    pub new_quantity: u32,
}

impl UpdateItemQuantity {
    /// Creates a new UpdateItemQuantity command.
    pub fn new(order_id: AggregateId, product_id: impl Into<ProductId>, new_quantity: u32) -> Self {
        Self {
            order_id,
            product_id: product_id.into(),
            new_quantity,
        }
    }
}

impl Command for UpdateItemQuantity {
    type Aggregate = Order;

    fn aggregate_id(&self) -> AggregateId {
        self.order_id
    }
}

/// Command to submit an order for processing.
#[derive(Debug, Clone)]
pub struct SubmitOrder {
    /// The order to submit.
    pub order_id: AggregateId,
}

impl SubmitOrder {
    /// Creates a new SubmitOrder command.
    pub fn new(order_id: AggregateId) -> Self {
        Self { order_id }
    }
}

impl Command for SubmitOrder {
    type Aggregate = Order;

    fn aggregate_id(&self) -> AggregateId {
        self.order_id
    }
}

/// Command to cancel an order.
#[derive(Debug, Clone)]
pub struct CancelOrder {
    /// The order to cancel.
    pub order_id: AggregateId,

    /// Reason for cancellation.
    pub reason: String,

    /// Who is cancelling the order.
    pub cancelled_by: Option<String>,
}

impl CancelOrder {
    /// Creates a new CancelOrder command.
    pub fn new(
        order_id: AggregateId,
        reason: impl Into<String>,
        cancelled_by: Option<String>,
    ) -> Self {
        Self {
            order_id,
            reason: reason.into(),
            cancelled_by,
        }
    }
}

impl Command for CancelOrder {
    type Aggregate = Order;

    fn aggregate_id(&self) -> AggregateId {
        self.order_id
    }
}

/// Command to mark inventory as reserved.
#[derive(Debug, Clone)]
pub struct MarkReserved {
    /// The order to mark as reserved.
    pub order_id: AggregateId,

    /// Reservation reference ID.
    pub reservation_id: Option<String>,
}

impl MarkReserved {
    /// Creates a new MarkReserved command.
    pub fn new(order_id: AggregateId, reservation_id: Option<String>) -> Self {
        Self {
            order_id,
            reservation_id,
        }
    }
}

impl Command for MarkReserved {
    type Aggregate = Order;

    fn aggregate_id(&self) -> AggregateId {
        self.order_id
    }
}

/// Command to start processing an order.
#[derive(Debug, Clone)]
pub struct StartProcessing {
    /// The order to start processing.
    pub order_id: AggregateId,

    /// Payment reference ID.
    pub payment_id: Option<String>,
}

impl StartProcessing {
    /// Creates a new StartProcessing command.
    pub fn new(order_id: AggregateId, payment_id: Option<String>) -> Self {
        Self {
            order_id,
            payment_id,
        }
    }
}

impl Command for StartProcessing {
    type Aggregate = Order;

    fn aggregate_id(&self) -> AggregateId {
        self.order_id
    }
}

/// Command to complete an order.
#[derive(Debug, Clone)]
pub struct CompleteOrder {
    /// The order to complete.
    pub order_id: AggregateId,

    /// Shipment tracking number.
    pub tracking_number: Option<String>,
}

impl CompleteOrder {
    /// Creates a new CompleteOrder command.
    pub fn new(order_id: AggregateId, tracking_number: Option<String>) -> Self {
        Self {
            order_id,
            tracking_number,
        }
    }
}

impl Command for CompleteOrder {
    type Aggregate = Order;

    fn aggregate_id(&self) -> AggregateId {
        self.order_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_order_command() {
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();

        let cmd = CreateOrder::new(order_id, customer_id);
        assert_eq!(cmd.aggregate_id(), order_id);
        assert_eq!(cmd.customer_id, customer_id);
    }

    #[test]
    fn test_create_order_for_customer() {
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);

        // Order ID should be generated
        assert_ne!(cmd.order_id, AggregateId::new());
        assert_eq!(cmd.customer_id, customer_id);
    }

    #[test]
    fn test_add_item_command() {
        let order_id = AggregateId::new();
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));

        let cmd = AddItem::new(order_id, item);
        assert_eq!(cmd.aggregate_id(), order_id);
        assert_eq!(cmd.item.product_id.as_str(), "SKU-001");
    }

    #[test]
    fn test_add_item_with_details() {
        let order_id = AggregateId::new();

        let cmd = AddItem::with_details(order_id, "SKU-002", "Gadget", 3, Money::from_cents(500));
        assert_eq!(cmd.aggregate_id(), order_id);
        assert_eq!(cmd.item.product_id.as_str(), "SKU-002");
        assert_eq!(cmd.item.quantity, 3);
    }

    #[test]
    fn test_remove_item_command() {
        let order_id = AggregateId::new();

        let cmd = RemoveItem::new(order_id, "SKU-001");
        assert_eq!(cmd.aggregate_id(), order_id);
        assert_eq!(cmd.product_id.as_str(), "SKU-001");
    }

    #[test]
    fn test_cancel_order_command() {
        let order_id = AggregateId::new();

        let cmd = CancelOrder::new(
            order_id,
            "Customer request",
            Some("user@example.com".to_string()),
        );
        assert_eq!(cmd.aggregate_id(), order_id);
        assert_eq!(cmd.reason, "Customer request");
        assert_eq!(cmd.cancelled_by, Some("user@example.com".to_string()));
    }
}
