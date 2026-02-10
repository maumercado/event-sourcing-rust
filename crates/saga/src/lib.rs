//! Saga pattern implementation for order fulfillment.
//!
//! This crate provides the Saga Pattern for orchestrating multi-step
//! distributed transactions with compensating actions on failure.
//!
//! The order fulfillment saga follows these steps:
//! 1. Reserve inventory
//! 2. Process payment
//! 3. Create shipment
//!
//! If any step fails, previously completed steps are compensated in reverse order.

pub mod aggregate;
pub mod coordinator;
pub mod error;
pub mod events;
pub mod order_fulfillment;
pub mod services;
pub mod state;

pub use aggregate::SagaInstance;
pub use coordinator::SagaCoordinator;
pub use error::SagaError;
pub use events::SagaEvent;
pub use services::{
    InMemoryInventoryService, InMemoryPaymentService, InMemoryShippingService, InventoryService,
    PaymentResult, PaymentService, ReservationItem, ReservationResult, ShipmentResult,
    ShippingService,
};
pub use state::SagaState;
