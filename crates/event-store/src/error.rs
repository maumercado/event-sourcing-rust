use thiserror::Error;

use crate::{AggregateId, Version};

/// Errors that can occur when interacting with the event store.
#[derive(Debug, Error)]
pub enum EventStoreError {
    /// A concurrency conflict occurred when appending events.
    /// The expected version did not match the actual version.
    #[error(
        "Concurrency conflict for aggregate {aggregate_id}: expected version {expected}, found {actual}"
    )]
    ConcurrencyConflict {
        aggregate_id: AggregateId,
        expected: Version,
        actual: Version,
    },

    /// The aggregate was not found in the event store.
    #[error("Aggregate not found: {0}")]
    AggregateNotFound(AggregateId),

    /// A database error occurred.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// A database migration error occurred.
    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// A serialization/deserialization error occurred.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type for event store operations.
pub type Result<T> = std::result::Result<T, EventStoreError>;
