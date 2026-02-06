//! Projection error types.

use thiserror::Error;

/// Errors that can occur during projection processing.
#[derive(Debug, Error)]
pub enum ProjectionError {
    /// An error occurred in the event store.
    #[error("Event store error: {0}")]
    EventStore(#[from] event_store::EventStoreError),

    /// Failed to deserialize an event payload.
    #[error("Event deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),

    /// A projection-specific error.
    #[error("Projection error: {0}")]
    Projection(String),
}

/// Result type for projection operations.
pub type Result<T> = std::result::Result<T, ProjectionError>;
