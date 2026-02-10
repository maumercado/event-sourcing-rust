//! Saga error types.

use common::AggregateId;
use domain::DomainError;
use event_store::EventStoreError;
use thiserror::Error;

use crate::state::SagaState;

/// Errors that can occur during saga operations.
#[derive(Debug, Error)]
pub enum SagaError {
    /// Saga is in an invalid state for the requested operation.
    #[error("Invalid saga state: expected {expected}, actual {actual}")]
    InvalidState { expected: String, actual: SagaState },

    /// A saga step failed.
    #[error("Saga step '{step}' failed: {reason}")]
    StepFailed { step: String, reason: String },

    /// A compensation step failed.
    #[error("Compensation step '{step}' failed: {reason}")]
    CompensationFailed { step: String, reason: String },

    /// Inventory service error.
    #[error("Inventory service error: {0}")]
    InventoryService(String),

    /// Payment service error.
    #[error("Payment service error: {0}")]
    PaymentService(String),

    /// Shipping service error.
    #[error("Shipping service error: {0}")]
    ShippingService(String),

    /// Domain error.
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    /// Event store error.
    #[error("Event store error: {0}")]
    EventStore(#[from] EventStoreError),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Saga has already been started.
    #[error("Saga has already been started")]
    AlreadyStarted,

    /// Order not found.
    #[error("Order not found: {0}")]
    OrderNotFound(AggregateId),

    /// Order is not in the expected state for saga execution.
    #[error("Order not ready: {0}")]
    OrderNotReady(String),
}

/// Convenience type alias for saga results.
pub type Result<T> = std::result::Result<T, SagaError>;
