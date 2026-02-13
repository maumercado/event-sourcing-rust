//! Integration tests for the API server.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use event_store::InMemoryEventStore;
use metrics_exporter_prometheus::PrometheusHandle;
use tower::ServiceExt;

use std::sync::OnceLock;

static METRICS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

fn get_metrics_handle() -> PrometheusHandle {
    METRICS_HANDLE
        .get_or_init(|| {
            let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
            builder
                .install_recorder()
                .expect("failed to install Prometheus recorder")
        })
        .clone()
}

fn setup() -> axum::Router {
    let store = InMemoryEventStore::new();
    let (state, processor, _) = api::create_default_state(store);
    let metrics_handle = get_metrics_handle();
    api::create_app(state, metrics_handle, processor)
}

fn setup_with_state() -> (
    axum::Router,
    Arc<api::routes::orders::AppState<InMemoryEventStore>>,
    Arc<projections::ProjectionProcessor<InMemoryEventStore>>,
) {
    let store = InMemoryEventStore::new();
    let (state, processor, _) = api::create_default_state(store);
    let metrics_handle = get_metrics_handle();
    let app = api::create_app(state.clone(), metrics_handle, processor.clone());
    (app, state, processor)
}

#[tokio::test]
async fn test_health_check() {
    let app = setup();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_create_order() {
    let app = setup();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "items": [{
                            "product_id": "SKU-001",
                            "product_name": "Widget",
                            "quantity": 2,
                            "unit_price_cents": 1000
                        }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["state"], "Draft");
    assert!(json["order_id"].as_str().is_some());
}

#[tokio::test]
async fn test_create_and_get_order() {
    let (app, _, _) = setup_with_state();

    // Create order
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "items": [{
                            "product_id": "SKU-001",
                            "product_name": "Widget",
                            "quantity": 2,
                            "unit_price_cents": 1000
                        }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let order_id = created["order_id"].as_str().unwrap();

    // Get order
    let get_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/orders/{order_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let order: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(order["id"], order_id);
    assert_eq!(order["state"], "Draft");
    assert_eq!(order["total_cents"], 2000);
    assert_eq!(order["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_get_nonexistent_order() {
    let app = setup();
    let fake_id = uuid::Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/orders/{fake_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_orders_from_projection() {
    let (app, _, processor) = setup_with_state();

    // Create an order
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "items": [{
                            "product_id": "SKU-001",
                            "product_name": "Widget",
                            "quantity": 1,
                            "unit_price_cents": 500
                        }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    // Run projection catch-up to populate read model
    processor.run_catch_up().await.unwrap();

    // List orders
    let list_response = app
        .oneshot(
            Request::builder()
                .uri("/orders")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let orders: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0]["total_cents"], 500);
}

#[tokio::test]
async fn test_submit_order() {
    let (app, _, _) = setup_with_state();

    // Create order with items
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "items": [{
                            "product_id": "SKU-001",
                            "product_name": "Widget",
                            "quantity": 1,
                            "unit_price_cents": 1000
                        }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let order_id = created["order_id"].as_str().unwrap();

    // Submit
    let submit_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/orders/{order_id}/submit"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(submit_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(submit_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let order: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(order["id"], order_id);
}

#[tokio::test]
async fn test_fulfill_order() {
    let (app, _, _) = setup_with_state();

    // Create order with items
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "items": [{
                            "product_id": "SKU-001",
                            "product_name": "Widget",
                            "quantity": 2,
                            "unit_price_cents": 1000
                        }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let order_id = created["order_id"].as_str().unwrap();

    // Fulfill (triggers saga)
    let fulfill_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/orders/{order_id}/fulfill"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(fulfill_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(fulfill_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["saga_state"], "Completed");
    let saga_id = result["saga_id"].as_str().unwrap();

    // Check saga status
    let saga_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/orders/{saga_id}/saga"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(saga_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(saga_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let saga: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(saga["state"], "Completed");
    assert_eq!(saga["completed_steps"].as_array().unwrap().len(), 3);
    assert!(saga["reservation_id"].as_str().is_some());
    assert!(saga["payment_id"].as_str().is_some());
    assert!(saga["tracking_number"].as_str().is_some());
}

#[tokio::test]
async fn test_invalid_order_id_format() {
    let app = setup();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/orders/not-a-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_order_with_customer_id() {
    let app = setup();
    let customer_id = uuid::Uuid::new_v4().to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "customer_id": customer_id,
                        "items": [{
                            "product_id": "SKU-001",
                            "product_name": "Widget",
                            "quantity": 1,
                            "unit_price_cents": 100
                        }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_get_order_events() {
    let (app, _, _) = setup_with_state();

    // Create order with items
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "items": [{
                            "product_id": "SKU-001",
                            "product_name": "Widget",
                            "quantity": 2,
                            "unit_price_cents": 1000
                        }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let order_id = created["order_id"].as_str().unwrap();

    // Get events
    let events_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/orders/{order_id}/events"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(events_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(events_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let events: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    // Should have OrderCreated + ItemAdded events
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event_type"], "OrderCreated");
    assert_eq!(events[0]["version"], 1);
    assert_eq!(events[1]["event_type"], "ItemAdded");
    assert_eq!(events[1]["version"], 2);
    assert!(events[0]["event_id"].as_str().is_some());
    assert!(events[0]["timestamp"].as_str().is_some());
    assert!(events[0]["payload"].is_object());
}

#[tokio::test]
async fn test_create_order_with_invalid_customer_id() {
    let app = setup();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/orders")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "customer_id": "not-a-uuid",
                        "items": [{
                            "product_id": "SKU-001",
                            "product_name": "Widget",
                            "quantity": 1,
                            "unit_price_cents": 100
                        }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
