//! HTTP API server with observability for the event-sourcing system.
//!
//! Provides REST endpoints for order management and saga execution,
//! with structured logging (tracing) and Prometheus metrics.

pub mod config;
pub mod error;
pub mod routes;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use event_store::EventStore;
use metrics_exporter_prometheus::PrometheusHandle;
use projections::{CurrentOrdersView, ProjectionProcessor};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use routes::orders::AppState;

/// Creates the Axum application router with all routes and shared state.
pub fn create_app<S: EventStore + Clone + 'static>(
    state: Arc<AppState<S>>,
    metrics_handle: PrometheusHandle,
    projection_processor: Arc<ProjectionProcessor<S>>,
) -> Router {
    let _ = &projection_processor;

    let metrics_router = Router::new()
        .route("/metrics", get(routes::metrics::get))
        .with_state(metrics_handle);

    Router::new()
        .route("/health", get(routes::health::check))
        .route("/orders", post(routes::orders::create::<S>))
        .route("/orders", get(routes::orders::list::<S>))
        .route("/orders/{id}", get(routes::orders::get::<S>))
        .route("/orders/{id}/submit", post(routes::orders::submit::<S>))
        .route("/orders/{id}/fulfill", post(routes::orders::fulfill::<S>))
        .route("/orders/{id}/saga", get(routes::orders::saga_status::<S>))
        .route("/orders/{id}/events", get(routes::orders::events::<S>))
        .with_state(state)
        .merge(metrics_router)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

/// Creates the default application state with stores and mock services.
pub fn create_default_state<S: EventStore + Clone + 'static>(
    event_store: S,
) -> (
    Arc<AppState<S>>,
    Arc<ProjectionProcessor<S>>,
    Arc<CurrentOrdersView>,
) {
    use domain::OrderService;
    use projections::Projection;
    use saga::{
        InMemoryInventoryService, InMemoryPaymentService, InMemoryShippingService, SagaCoordinator,
    };

    let order_service = OrderService::new(event_store.clone());
    let inventory = InMemoryInventoryService::new();
    let payment = InMemoryPaymentService::new();
    let shipping = InMemoryShippingService::new();
    let saga_coordinator = SagaCoordinator::new(event_store.clone(), inventory, payment, shipping);

    let current_orders = Arc::new(CurrentOrdersView::new());

    let mut processor = ProjectionProcessor::new(event_store.clone());
    processor.register(Box::new(current_orders.as_ref().clone()) as Box<dyn Projection>);
    let processor = Arc::new(processor);

    let state = Arc::new(AppState {
        order_service,
        saga_coordinator,
        current_orders: current_orders.clone(),
        event_store,
        projection_processor: processor.clone(),
    });

    (state, processor, current_orders)
}
