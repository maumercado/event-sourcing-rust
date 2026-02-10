//! Order service providing a simplified API for order operations.

use common::AggregateId;
use event_store::EventStore;

use crate::command::{CommandHandler, CommandResult};
use crate::error::DomainError;

use super::{
    AddItem, CancelOrder, CompleteOrder, CreateOrder, CustomerId, MarkReserved, Money, Order,
    OrderItem, ProductId, RemoveItem, StartProcessing, SubmitOrder, UpdateItemQuantity,
};

impl From<super::OrderError> for DomainError {
    fn from(e: super::OrderError) -> Self {
        DomainError::Order(e)
    }
}

/// Service for managing orders.
///
/// Provides a high-level API for order operations, wrapping the command handler
/// and providing convenient methods for common operations.
pub struct OrderService<S: EventStore> {
    handler: CommandHandler<S, Order>,
}

impl<S: EventStore> OrderService<S> {
    /// Creates a new order service with the given event store.
    pub fn new(store: S) -> Self {
        Self {
            handler: CommandHandler::new(store),
        }
    }

    /// Returns a reference to the underlying command handler.
    pub fn handler(&self) -> &CommandHandler<S, Order> {
        &self.handler
    }

    /// Creates a new order for a customer.
    #[tracing::instrument(skip(self))]
    pub async fn create_order(
        &self,
        cmd: CreateOrder,
    ) -> Result<CommandResult<Order>, DomainError> {
        let order_id = cmd.order_id;
        let customer_id = cmd.customer_id;

        self.handler
            .execute(order_id, |order| order.create(order_id, customer_id))
            .await
    }

    /// Adds an item to an order.
    #[tracing::instrument(skip(self))]
    pub async fn add_item(&self, cmd: AddItem) -> Result<CommandResult<Order>, DomainError> {
        let item = cmd.item.clone();

        self.handler
            .execute(cmd.order_id, |order| order.add_item(item))
            .await
    }

    /// Removes an item from an order.
    #[tracing::instrument(skip(self))]
    pub async fn remove_item(&self, cmd: RemoveItem) -> Result<CommandResult<Order>, DomainError> {
        let product_id = cmd.product_id.clone();

        self.handler
            .execute(cmd.order_id, |order| order.remove_item(product_id))
            .await
    }

    /// Updates the quantity of an item in an order.
    #[tracing::instrument(skip(self))]
    pub async fn update_item_quantity(
        &self,
        cmd: UpdateItemQuantity,
    ) -> Result<CommandResult<Order>, DomainError> {
        let product_id = cmd.product_id.clone();
        let new_quantity = cmd.new_quantity;

        self.handler
            .execute(cmd.order_id, |order| {
                order.update_item_quantity(product_id, new_quantity)
            })
            .await
    }

    /// Submits an order for processing.
    #[tracing::instrument(skip(self))]
    pub async fn submit_order(
        &self,
        cmd: SubmitOrder,
    ) -> Result<CommandResult<Order>, DomainError> {
        self.handler
            .execute(cmd.order_id, |order| order.submit())
            .await
    }

    /// Marks inventory as reserved for an order.
    #[tracing::instrument(skip(self))]
    pub async fn mark_reserved(
        &self,
        cmd: MarkReserved,
    ) -> Result<CommandResult<Order>, DomainError> {
        let reservation_id = cmd.reservation_id.clone();

        self.handler
            .execute(cmd.order_id, |order| order.mark_reserved(reservation_id))
            .await
    }

    /// Starts processing an order.
    #[tracing::instrument(skip(self))]
    pub async fn start_processing(
        &self,
        cmd: StartProcessing,
    ) -> Result<CommandResult<Order>, DomainError> {
        let payment_id = cmd.payment_id.clone();

        self.handler
            .execute(cmd.order_id, |order| order.start_processing(payment_id))
            .await
    }

    /// Completes an order.
    #[tracing::instrument(skip(self))]
    pub async fn complete_order(
        &self,
        cmd: CompleteOrder,
    ) -> Result<CommandResult<Order>, DomainError> {
        let tracking_number = cmd.tracking_number.clone();

        self.handler
            .execute(cmd.order_id, |order| order.complete(tracking_number))
            .await
    }

    /// Cancels an order.
    #[tracing::instrument(skip(self))]
    pub async fn cancel_order(
        &self,
        cmd: CancelOrder,
    ) -> Result<CommandResult<Order>, DomainError> {
        let reason = cmd.reason.clone();
        let cancelled_by = cmd.cancelled_by.clone();

        self.handler
            .execute(cmd.order_id, |order| order.cancel(reason, cancelled_by))
            .await
    }

    /// Loads an order by ID.
    ///
    /// Returns None if the order doesn't exist.
    #[tracing::instrument(skip(self))]
    pub async fn get_order(&self, order_id: AggregateId) -> Result<Option<Order>, DomainError> {
        self.handler.load_existing(order_id).await
    }

    // Convenience methods

    /// Creates an order and adds items in a single operation.
    ///
    /// This is a convenience method that creates the order and adds all items
    /// in sequence.
    pub async fn create_order_with_items(
        &self,
        customer_id: CustomerId,
        items: Vec<OrderItem>,
    ) -> Result<CommandResult<Order>, DomainError> {
        let order_id = AggregateId::new();

        // Create order
        self.create_order(CreateOrder::new(order_id, customer_id))
            .await?;

        // Add items
        let mut result = None;
        for item in items {
            result = Some(self.add_item(AddItem::new(order_id, item)).await?);
        }

        // Return the final state, or load if no items were added
        match result {
            Some(r) => Ok(r),
            None => {
                let order = self.handler.load(order_id).await?;
                Ok(CommandResult {
                    aggregate: order,
                    events: vec![],
                    new_version: event_store::Version::first(),
                })
            }
        }
    }

    /// Adds an item using individual fields.
    pub async fn add_item_to_order(
        &self,
        order_id: AggregateId,
        product_id: impl Into<ProductId>,
        product_name: impl Into<String>,
        quantity: u32,
        unit_price: Money,
    ) -> Result<CommandResult<Order>, DomainError> {
        let item = OrderItem::new(product_id, product_name, quantity, unit_price);
        self.add_item(AddItem::new(order_id, item)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::Aggregate;
    use crate::order::OrderState;
    use event_store::InMemoryEventStore;

    #[tokio::test]
    async fn test_create_order() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store);

        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;

        let result = service.create_order(cmd).await.unwrap();

        assert_eq!(result.aggregate.id(), Some(order_id));
        assert_eq!(result.aggregate.customer_id(), Some(customer_id));
        assert_eq!(result.events.len(), 1);
    }

    #[tokio::test]
    async fn test_add_item() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store);

        // Create order
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;
        service.create_order(cmd).await.unwrap();

        // Add item
        let result = service
            .add_item_to_order(order_id, "SKU-001", "Widget", 2, Money::from_cents(1000))
            .await
            .unwrap();

        assert_eq!(result.aggregate.item_count(), 1);
        assert_eq!(result.aggregate.total_amount().cents(), 2000);
    }

    #[tokio::test]
    async fn test_full_order_lifecycle() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store);

        // Create order
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;
        service.create_order(cmd).await.unwrap();

        // Add items
        service
            .add_item_to_order(order_id, "SKU-001", "Widget", 2, Money::from_cents(1000))
            .await
            .unwrap();

        // Submit
        service
            .submit_order(SubmitOrder::new(order_id))
            .await
            .unwrap();

        // Reserve
        service
            .mark_reserved(MarkReserved::new(order_id, Some("RES-123".to_string())))
            .await
            .unwrap();

        // Process
        service
            .start_processing(StartProcessing::new(order_id, Some("PAY-123".to_string())))
            .await
            .unwrap();

        // Complete
        let result = service
            .complete_order(CompleteOrder::new(order_id, Some("TRACK-123".to_string())))
            .await
            .unwrap();

        assert_eq!(result.aggregate.state(), OrderState::Completed);
    }

    #[tokio::test]
    async fn test_cancel_order() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store);

        // Create order with items
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;
        service.create_order(cmd).await.unwrap();

        service
            .add_item_to_order(order_id, "SKU-001", "Widget", 1, Money::from_cents(1000))
            .await
            .unwrap();

        // Cancel
        let result = service
            .cancel_order(CancelOrder::new(order_id, "Customer changed mind", None))
            .await
            .unwrap();

        assert_eq!(result.aggregate.state(), OrderState::Cancelled);
    }

    #[tokio::test]
    async fn test_get_order() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store);

        // Non-existent order
        let result = service.get_order(AggregateId::new()).await.unwrap();
        assert!(result.is_none());

        // Create and get
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;
        service.create_order(cmd).await.unwrap();

        let result = service.get_order(order_id).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id(), Some(order_id));
    }

    #[tokio::test]
    async fn test_create_order_with_items() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store);

        let customer_id = CustomerId::new();
        let items = vec![
            OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000)),
            OrderItem::new("SKU-002", "Gadget", 1, Money::from_cents(500)),
        ];

        let result = service
            .create_order_with_items(customer_id, items)
            .await
            .unwrap();

        assert_eq!(result.aggregate.item_count(), 2);
        assert_eq!(result.aggregate.total_amount().cents(), 2500);
    }

    #[tokio::test]
    async fn test_update_item_quantity() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store);

        // Create order with item
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;
        service.create_order(cmd).await.unwrap();

        service
            .add_item_to_order(order_id, "SKU-001", "Widget", 2, Money::from_cents(1000))
            .await
            .unwrap();

        // Update quantity
        let result = service
            .update_item_quantity(UpdateItemQuantity::new(order_id, "SKU-001", 5))
            .await
            .unwrap();

        let item = result
            .aggregate
            .get_item(&ProductId::new("SKU-001"))
            .unwrap();
        assert_eq!(item.quantity, 5);
        assert_eq!(result.aggregate.total_amount().cents(), 5000);
    }

    #[tokio::test]
    async fn test_remove_item() {
        let store = InMemoryEventStore::new();
        let service = OrderService::new(store);

        // Create order with items
        let customer_id = CustomerId::new();
        let cmd = CreateOrder::for_customer(customer_id);
        let order_id = cmd.order_id;
        service.create_order(cmd).await.unwrap();

        service
            .add_item_to_order(order_id, "SKU-001", "Widget", 2, Money::from_cents(1000))
            .await
            .unwrap();

        // Remove item
        let result = service
            .remove_item(RemoveItem::new(order_id, "SKU-001"))
            .await
            .unwrap();

        assert_eq!(result.aggregate.item_count(), 0);
        assert_eq!(result.aggregate.total_amount().cents(), 0);
    }
}
