//! API error types with HTTP response mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use domain::{DomainError, OrderError};
use event_store::EventStoreError;
use saga::SagaError;

/// API-level error type that maps to HTTP responses.
#[derive(Debug)]
pub enum ApiError {
    /// Resource not found.
    NotFound(String),
    /// Bad request from the client.
    BadRequest(String),
    /// Domain logic error.
    Domain(DomainError),
    /// Saga execution error.
    Saga(SagaError),
    /// Internal server error.
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Domain(err) => domain_error_to_response(err),
            ApiError::Saga(err) => saga_error_to_response(err),
            ApiError::Internal(msg) => {
                tracing::error!(error = %msg, "internal server error");
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };

        let body = serde_json::json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}

fn domain_error_to_response(err: DomainError) -> (StatusCode, String) {
    match &err {
        DomainError::Order(order_err) => match order_err {
            OrderError::InvalidStateTransition { .. } => (StatusCode::CONFLICT, err.to_string()),
            OrderError::ItemNotFound { .. } => (StatusCode::NOT_FOUND, err.to_string()),
            OrderError::InvalidQuantity { .. }
            | OrderError::InvalidPrice { .. }
            | OrderError::NoItems
            | OrderError::CustomerIdRequired
            | OrderError::AlreadyCreated => (StatusCode::BAD_REQUEST, err.to_string()),
        },
        DomainError::AggregateNotFound { .. } => (StatusCode::NOT_FOUND, err.to_string()),
        DomainError::EventStore(EventStoreError::ConcurrencyConflict { .. }) => {
            (StatusCode::CONFLICT, err.to_string())
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

fn saga_error_to_response(err: SagaError) -> (StatusCode, String) {
    match &err {
        SagaError::OrderNotFound(_) => (StatusCode::NOT_FOUND, err.to_string()),
        SagaError::OrderNotReady(_) => (StatusCode::BAD_REQUEST, err.to_string()),
        SagaError::InvalidState { .. } => (StatusCode::CONFLICT, err.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        ApiError::Domain(err)
    }
}

impl From<SagaError> for ApiError {
    fn from(err: SagaError) -> Self {
        ApiError::Saga(err)
    }
}
