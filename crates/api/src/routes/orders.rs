//! Order CRUD and saga trigger endpoints.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use common::AggregateId;
use domain::{AddItem, CreateOrder, CustomerId, Money, OrderItem, OrderService, SubmitOrder};
use event_store::EventStore;
use projections::{CurrentOrdersView, ProjectionProcessor};
use saga::{
    InMemoryInventoryService, InMemoryPaymentService, InMemoryShippingService, SagaCoordinator,
};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;

/// Shared application state accessible from all handlers.
pub struct AppState<S: EventStore> {
    pub order_service: OrderService<S>,
    pub saga_coordinator: SagaCoordinator<
        S,
        InMemoryInventoryService,
        InMemoryPaymentService,
        InMemoryShippingService,
    >,
    pub current_orders: Arc<CurrentOrdersView>,
    pub event_store: S,
    pub projection_processor: Arc<ProjectionProcessor<S>>,
}

// -- Request types --

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub customer_id: Option<String>,
    pub items: Vec<OrderItemRequest>,
}

#[derive(Deserialize)]
pub struct OrderItemRequest {
    pub product_id: String,
    pub product_name: String,
    pub quantity: u32,
    pub unit_price_cents: i64,
}

// -- Response types --

#[derive(Serialize)]
pub struct OrderResponse {
    pub id: String,
    pub customer_id: String,
    pub state: String,
    pub items: Vec<OrderItemResponse>,
    pub total_cents: i64,
}

#[derive(Serialize)]
pub struct OrderItemResponse {
    pub product_id: String,
    pub product_name: String,
    pub quantity: u32,
    pub unit_price_cents: i64,
}

#[derive(Serialize)]
pub struct OrderCreatedResponse {
    pub order_id: String,
    pub state: String,
}

#[derive(Serialize)]
pub struct SagaStatusResponse {
    pub saga_id: String,
    pub order_id: String,
    pub state: String,
    pub completed_steps: Vec<String>,
    pub reservation_id: Option<String>,
    pub payment_id: Option<String>,
    pub tracking_number: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Serialize)]
pub struct FulfillResponse {
    pub saga_id: String,
    pub saga_state: String,
}

// -- Handlers --

/// POST /orders — create a new order with optional items.
#[tracing::instrument(skip(state, req))]
pub async fn create<S: EventStore + Clone + 'static>(
    State(state): State<Arc<AppState<S>>>,
    Json(req): Json<CreateOrderRequest>,
) -> Result<(axum::http::StatusCode, Json<OrderCreatedResponse>), ApiError> {
    let customer_id = if let Some(ref id_str) = req.customer_id {
        let uuid = uuid::Uuid::parse_str(id_str)
            .map_err(|e| ApiError::BadRequest(format!("Invalid customer_id: {e}")))?;
        CustomerId::from_uuid(uuid)
    } else {
        CustomerId::new()
    };

    let cmd = CreateOrder::for_customer(customer_id);
    let order_id = cmd.order_id;
    state.order_service.create_order(cmd).await?;

    for item_req in &req.items {
        let item = OrderItem::new(
            item_req.product_id.as_str(),
            item_req.product_name.as_str(),
            item_req.quantity,
            Money::from_cents(item_req.unit_price_cents),
        );
        state
            .order_service
            .add_item(AddItem::new(order_id, item))
            .await?;
    }

    let response = OrderCreatedResponse {
        order_id: order_id.to_string(),
        state: "Draft".to_string(),
    };

    Ok((axum::http::StatusCode::CREATED, Json(response)))
}

/// GET /orders/:id — load an order aggregate by ID.
#[tracing::instrument(skip(state))]
pub async fn get<S: EventStore + Clone + 'static>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<String>,
) -> Result<Json<OrderResponse>, ApiError> {
    let aggregate_id = parse_aggregate_id(&id)?;
    let order = state
        .order_service
        .get_order(aggregate_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Order {id} not found")))?;

    let items: Vec<OrderItemResponse> = order
        .items()
        .map(|item| OrderItemResponse {
            product_id: item.product_id.to_string(),
            product_name: item.product_name.clone(),
            quantity: item.quantity,
            unit_price_cents: item.unit_price.cents(),
        })
        .collect();

    Ok(Json(OrderResponse {
        id: aggregate_id.to_string(),
        customer_id: order
            .customer_id()
            .map(|c| c.to_string())
            .unwrap_or_default(),
        state: order.state().to_string(),
        items,
        total_cents: order.total_amount().cents(),
    }))
}

/// GET /orders — list current (active) orders from projection.
#[tracing::instrument(skip(state))]
pub async fn list<S: EventStore + Clone + 'static>(
    State(state): State<Arc<AppState<S>>>,
) -> Result<Json<Vec<OrderResponse>>, ApiError> {
    // Run catch-up to ensure the read model includes latest events
    state
        .projection_processor
        .run_catch_up()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let orders = state.current_orders.get_all_orders().await;

    let responses: Vec<OrderResponse> = orders
        .into_iter()
        .map(|o| {
            let items: Vec<OrderItemResponse> = o
                .items
                .values()
                .map(|item| OrderItemResponse {
                    product_id: item.product_id.to_string(),
                    product_name: item.product_name.clone(),
                    quantity: item.quantity,
                    unit_price_cents: item.unit_price.cents(),
                })
                .collect();
            OrderResponse {
                id: o.order_id.to_string(),
                customer_id: o.customer_id.to_string(),
                state: o.state.to_string(),
                items,
                total_cents: o.total_amount.cents(),
            }
        })
        .collect();

    Ok(Json(responses))
}

/// POST /orders/:id/submit — submit an order for fulfillment.
#[tracing::instrument(skip(state))]
pub async fn submit<S: EventStore + Clone + 'static>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<String>,
) -> Result<Json<OrderResponse>, ApiError> {
    let aggregate_id = parse_aggregate_id(&id)?;

    state
        .order_service
        .submit_order(SubmitOrder::new(aggregate_id))
        .await?;

    // Re-load and return
    let order = state
        .order_service
        .get_order(aggregate_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Order {id} not found")))?;

    let items: Vec<OrderItemResponse> = order
        .items()
        .map(|item| OrderItemResponse {
            product_id: item.product_id.to_string(),
            product_name: item.product_name.clone(),
            quantity: item.quantity,
            unit_price_cents: item.unit_price.cents(),
        })
        .collect();

    Ok(Json(OrderResponse {
        id: aggregate_id.to_string(),
        customer_id: order
            .customer_id()
            .map(|c| c.to_string())
            .unwrap_or_default(),
        state: order.state().to_string(),
        items,
        total_cents: order.total_amount().cents(),
    }))
}

/// POST /orders/:id/fulfill — trigger saga execution for the order.
#[tracing::instrument(skip(state))]
pub async fn fulfill<S: EventStore + Clone + 'static>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<String>,
) -> Result<Json<FulfillResponse>, ApiError> {
    let aggregate_id = parse_aggregate_id(&id)?;

    let saga_id = state.saga_coordinator.execute_saga(aggregate_id).await?;

    let saga = state
        .saga_coordinator
        .get_saga(saga_id)
        .await?
        .ok_or_else(|| ApiError::Internal("Saga not found after execution".to_string()))?;

    Ok(Json(FulfillResponse {
        saga_id: saga_id.to_string(),
        saga_state: format!("{:?}", saga.state()),
    }))
}

/// GET /orders/:id/saga — get saga state for an order.
#[tracing::instrument(skip(state))]
pub async fn saga_status<S: EventStore + Clone + 'static>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<String>,
) -> Result<Json<SagaStatusResponse>, ApiError> {
    let saga_id = parse_aggregate_id(&id)?;

    let saga = state
        .saga_coordinator
        .get_saga(saga_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Saga {id} not found")))?;

    Ok(Json(SagaStatusResponse {
        saga_id: saga_id.to_string(),
        order_id: saga.order_id().map(|id| id.to_string()).unwrap_or_default(),
        state: format!("{:?}", saga.state()),
        completed_steps: saga.completed_steps().to_vec(),
        reservation_id: saga.reservation_id().map(String::from),
        payment_id: saga.payment_id().map(String::from),
        tracking_number: saga.tracking_number().map(String::from),
        failure_reason: saga.failure_reason().map(String::from),
    }))
}

/// Response type for event envelope data.
#[derive(Serialize)]
pub struct EventEnvelopeResponse {
    pub event_id: String,
    pub event_type: String,
    pub aggregate_id: String,
    pub version: i64,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

/// GET /orders/:id/events — list all events for an order aggregate.
#[tracing::instrument(skip(state))]
pub async fn events<S: EventStore + Clone + 'static>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<EventEnvelopeResponse>>, ApiError> {
    let aggregate_id = parse_aggregate_id(&id)?;

    let envelopes = state
        .event_store
        .get_events_for_aggregate(aggregate_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let responses: Vec<EventEnvelopeResponse> = envelopes
        .into_iter()
        .map(|e| EventEnvelopeResponse {
            event_id: e.event_id.to_string(),
            event_type: e.event_type,
            aggregate_id: e.aggregate_id.to_string(),
            version: e.version.as_i64(),
            timestamp: e.timestamp.to_rfc3339(),
            payload: e.payload,
        })
        .collect();

    Ok(Json(responses))
}

fn parse_aggregate_id(id: &str) -> Result<AggregateId, ApiError> {
    let uuid = uuid::Uuid::parse_str(id)
        .map_err(|e| ApiError::BadRequest(format!("Invalid ID format: {e}")))?;
    Ok(AggregateId::from(uuid))
}
