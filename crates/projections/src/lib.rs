//! Read models and projections for the CQRS query side.
//!
//! This crate provides the query side of the CQRS pattern:
//! - [`Projection`] trait for processing events into read models
//! - [`ReadModel`] trait for query access to denormalized data
//! - [`ProjectionProcessor`] for feeding events from the store to projections
//! - Four read model views: current orders, order history, customer orders, inventory

pub mod error;
pub mod processor;
pub mod projection;
pub mod read_model;
pub mod views;

pub use error::{ProjectionError, Result};
pub use processor::ProjectionProcessor;
pub use projection::{Projection, ProjectionPosition};
pub use read_model::ReadModel;
pub use views::{CurrentOrdersView, CustomerOrdersView, InventoryView, OrderHistoryView};
