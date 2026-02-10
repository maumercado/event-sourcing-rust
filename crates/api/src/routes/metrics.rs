//! Prometheus metrics endpoint.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use metrics_exporter_prometheus::PrometheusHandle;

/// GET /metrics — returns Prometheus-formatted metrics.
pub async fn get(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        handle.render(),
    )
}
