//! Health check endpoint.

use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

/// GET /health — returns system health status.
pub async fn check() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
