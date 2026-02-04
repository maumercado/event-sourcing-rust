//! Domain error types.

use event_store::EventStoreError;
use thiserror::Error;

use crate::order::OrderError;

/// Errors that can occur during domain operations.
#[derive(Debug, Error)]
pub enum DomainError {
    /// An error occurred in the event store.
    #[error("Event store error: {0}")]
    EventStore(#[from] EventStoreError),

    /// An error occurred in the order aggregate.
    #[error("Order error: {0}")]
    Order(OrderError),

    /// Aggregate not found.
    #[error("Aggregate not found: {aggregate_type} with id {aggregate_id}")]
    AggregateNotFound {
        aggregate_type: &'static str,
        aggregate_id: String,
    },

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
