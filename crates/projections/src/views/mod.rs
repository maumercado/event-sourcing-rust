//! Read model views for the CQRS query side.

pub mod current_orders;
pub mod customer_orders;
pub mod inventory;
pub mod order_history;

pub use current_orders::CurrentOrdersView;
pub use customer_orders::CustomerOrdersView;
pub use inventory::InventoryView;
pub use order_history::OrderHistoryView;
