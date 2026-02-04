//! Domain layer for the event-sourcing system.
//!
//! This crate provides the core domain abstractions including:
//! - Aggregate trait for event-sourced entities
//! - DomainEvent trait for domain events
//! - Command trait and CommandHandler for command processing
//! - Order aggregate implementation with state machine

pub mod aggregate;
pub mod command;
pub mod error;
pub mod order;

pub use aggregate::{Aggregate, DomainEvent};
pub use command::{Command, CommandHandler, CommandResult};
pub use error::DomainError;
pub use order::{
    AddItem, CancelOrder, CompleteOrder, CreateOrder, CustomerId, MarkReserved, Money, Order,
    OrderError, OrderEvent, OrderItem, OrderService, OrderState, ProductId, RemoveItem,
    StartProcessing, SubmitOrder, UpdateItemQuantity,
};
