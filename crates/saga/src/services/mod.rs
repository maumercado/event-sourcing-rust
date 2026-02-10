//! External service traits and in-memory implementations for saga steps.

pub mod inventory;
pub mod payment;
pub mod shipping;

pub use inventory::{
    InMemoryInventoryService, InventoryService, ReservationItem, ReservationResult,
};
pub use payment::{InMemoryPaymentService, PaymentResult, PaymentService};
pub use shipping::{InMemoryShippingService, ShipmentResult, ShippingService};
