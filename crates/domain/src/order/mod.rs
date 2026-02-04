//! Order aggregate and related types.

mod aggregate;
mod commands;
mod events;
mod service;
mod state;
mod value_objects;

pub use aggregate::Order;
pub use commands::*;
pub use events::{
    ItemAddedData, ItemQuantityUpdatedData, ItemRemovedData, OrderCancelledData,
    OrderCompletedData, OrderCreatedData, OrderEvent, OrderProcessingData, OrderReservedData,
    OrderSubmittedData,
};
pub use service::OrderService;
pub use state::OrderState;
pub use value_objects::{CustomerId, Money, OrderItem, ProductId};

use thiserror::Error;

/// Errors that can occur during order operations.
#[derive(Debug, Error)]
pub enum OrderError {
    /// Customer ID is required.
    #[error("Customer ID is required")]
    CustomerIdRequired,

    /// Order is not in the expected state.
    #[error("Invalid state transition: cannot {action} from {current_state} state")]
    InvalidStateTransition {
        current_state: OrderState,
        action: &'static str,
    },

    /// Item not found in order.
    #[error("Item not found: {product_id}")]
    ItemNotFound { product_id: String },

    /// Invalid quantity.
    #[error("Invalid quantity: {quantity} (must be greater than 0)")]
    InvalidQuantity { quantity: u32 },

    /// Invalid price.
    #[error("Invalid price: {price} (must be greater than 0)")]
    InvalidPrice { price: i64 },

    /// Order has no items.
    #[error("Order has no items")]
    NoItems,

    /// Order is already created.
    #[error("Order already created")]
    AlreadyCreated,
}
