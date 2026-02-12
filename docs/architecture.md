# Architecture

## Overview

This system implements an event-sourced order management platform using three complementary patterns:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Order Management System                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────────────┐ │
│  │   Event Store   │◀───│  Domain Layer   │───▶│    Saga Coordinator     │ │
│  │  (append-only)  │    │  (aggregates)   │    │  (distributed txns)     │ │
│  └────────┬────────┘    └────────┬────────┘    └───────────┬─────────────┘ │
│           │                      │                         │               │
│           │                      │                         │               │
│           ▼                      ▼                         ▼               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         Event Bus                                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│           │                      │                         │               │
│           ▼                      ▼                         ▼               │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────────┐    │
│  │  Read Models    │    │   Audit Log     │    │    Notifications    │    │
│  │  (projections)  │    │   (analytics)   │    │      (alerts)       │    │
│  └─────────────────┘    └─────────────────┘    └─────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## How the Patterns Work Together

### Data Flow

```
User Request
     │
     ▼
┌─────────────┐
│   Command   │ ─────────────────────────────────────────┐
└──────┬──────┘                                          │
       │                                                 │
       ▼                                                 ▼
┌─────────────┐                                 ┌─────────────┐
│  Aggregate  │ ◀────── Load Events ─────────── │ Event Store │
└──────┬──────┘                                 └──────┬──────┘
       │                                               │
       │ Validate                                      │
       │ & Execute                                     │
       ▼                                               │
┌─────────────┐                                        │
│   Events    │ ─────── Append ───────────────────────▶│
└──────┬──────┘                                        │
       │                                               │
       │ Publish                                       │
       ▼                                               ▼
┌─────────────┐                               ┌─────────────┐
│    Saga     │ ◀───── Subscribe ───────────  │ Projections │
│ Coordinator │                               │(Read Models)│
└─────────────┘                               └──────┬──────┘
                                                     │
                                                     ▼
                                               ┌─────────────┐
                                               │   Queries   │
                                               └─────────────┘
```

### Pattern Responsibilities

| Pattern | Responsibility | Key Benefit |
|---------|---------------|-------------|
| Event Sourcing | Store what happened | Audit trail, time travel |
| CQRS | Separate read/write models | Optimized queries, scalability |
| Saga | Coordinate distributed operations | Reliable cross-service transactions |

## Crate Structure

```
crates/
├── common/                    # Shared types
│   └── src/
│       └── lib.rs            # AggregateId, etc.
│
├── event-store/              # Event persistence
│   └── src/
│       ├── event.rs          # EventEnvelope, Version
│       ├── store.rs          # EventStore trait
│       ├── postgres.rs       # PostgreSQL implementation
│       ├── memory.rs         # In-memory (testing)
│       ├── snapshot.rs       # Aggregate snapshots
│       ├── query.rs          # Event queries
│       └── error.rs          # Store errors
│
├── domain/                   # Business logic
│   └── src/
│       ├── aggregate.rs      # Aggregate trait
│       ├── command.rs        # CommandHandler
│       ├── error.rs          # Domain errors
│       └── order/            # Order aggregate
│           ├── aggregate.rs  # Order struct
│           ├── state.rs      # State machine
│           ├── events.rs     # Domain events
│           ├── commands.rs   # Command structs
│           ├── service.rs    # High-level API
│           └── value_objects.rs
│
├── projections/              # Read models (Phase 3)
│   └── src/
│       ├── lib.rs
│       ├── error.rs          # ProjectionError
│       ├── projection.rs     # Projection trait
│       ├── read_model.rs     # ReadModel trait
│       ├── processor.rs      # ProjectionProcessor
│       └── views/
│           ├── current_orders.rs   # Active orders
│           ├── order_history.rs    # Completed/cancelled
│           ├── customer_orders.rs  # Per-customer stats
│           └── inventory.rs        # Product demand
│
├── saga/                     # Saga coordination (Phase 4)
│   └── src/
│       ├── lib.rs
│       ├── error.rs          # SagaError
│       ├── state.rs          # SagaState enum
│       ├── events.rs         # SagaEvent enum
│       ├── aggregate.rs      # SagaInstance (implements Aggregate)
│       ├── coordinator.rs    # SagaCoordinator orchestrator
│       ├── order_fulfillment.rs  # Step name constants
│       └── services/
│           ├── inventory.rs  # InventoryService trait + mock
│           ├── payment.rs    # PaymentService trait + mock
│           └── shipping.rs   # ShippingService trait + mock
│
└── api/                      # HTTP API server (Phase 5)
    └── src/
        ├── lib.rs            # AppState, create_app(), router
        ├── main.rs           # Binary entry point
        ├── error.rs          # ApiError → HTTP response mapping
        └── routes/
            ├── health.rs     # GET /health
            ├── metrics.rs    # GET /metrics (Prometheus)
            └── orders.rs     # Order CRUD + saga trigger
```

## Command Side (Write Path)

### 1. Command Received

A command represents an intention to change state:

```rust
// User wants to add an item to their order
let command = AddItem::new(
    order_id,
    OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000)),
);
```

### 2. Aggregate Loaded

The CommandHandler loads the aggregate by replaying its events:

```rust
// In CommandHandler::load()
let events = store.get_events_for_aggregate(aggregate_id).await?;
let mut aggregate = Order::default();
for event in events {
    aggregate.apply(event);
}
```

### 3. Command Validated & Executed

The aggregate validates the command against its current state:

```rust
// In Order::add_item()
fn add_item(&self, item: OrderItem) -> Result<Vec<OrderEvent>, OrderError> {
    // Validate current state allows this operation
    if !self.state.can_modify() {
        return Err(OrderError::InvalidStateTransition { ... });
    }

    // Validate command parameters
    if item.quantity == 0 {
        return Err(OrderError::InvalidQuantity);
    }

    // Return events (not mutations!)
    Ok(vec![OrderEvent::ItemAdded(ItemAddedData { ... })])
}
```

### 4. Events Persisted

Events are appended with optimistic concurrency control:

```rust
// In CommandHandler::execute()
let options = AppendOptions::expect_version(current_version);
store.append(events, options).await?;
```

### 5. Events Published

Events flow to projections and sagas:

```rust
// Projections update read models
projection.handle(OrderEvent::ItemAdded { ... });

// Sagas coordinate cross-service operations
saga.on_order_submitted(order_id);
```

## Query Side (Read Path)

### Projections

Projections build read-optimized views from events:

```rust
// Example: OrderSummaryProjection
impl Projection for OrderSummaryProjection {
    fn handle(&mut self, event: OrderEvent) {
        match event {
            OrderEvent::ItemAdded(data) => {
                self.orders.get_mut(&data.order_id)
                    .item_count += data.quantity;
            }
            // ... handle other events
        }
    }
}
```

### Query Handlers

Queries read directly from projections (fast, no event replay):

```rust
async fn get_customer_orders(customer_id: CustomerId) -> Vec<OrderSummary> {
    projection.orders_by_customer(customer_id)
}
```

## Distributed Transactions (Saga Path)

When an order is submitted, the saga coordinates multiple services:

```
OrderSubmitted Event
        │
        ▼
┌─────────────────────────────────────────────────────────┐
│              OrderFulfillmentSaga                        │
│                                                          │
│  1. Reserve Inventory                                   │
│     ├── Success ──▶ Continue                            │
│     └── Failure ──▶ Cancel Order                        │
│                                                          │
│  2. Process Payment                                      │
│     ├── Success ──▶ Continue                            │
│     └── Failure ──▶ Release Inventory → Cancel Order    │
│                                                          │
│  3. Create Shipment                                      │
│     ├── Success ──▶ Complete Order                      │
│     └── Failure ──▶ Refund → Release → Cancel           │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

## Consistency Model

### Strong Consistency (Write Side)

- Single aggregate is always consistent
- Optimistic concurrency prevents lost updates
- Commands either fully succeed or fully fail

### Eventual Consistency (Read Side)

- Projections may lag behind events
- Queries might return slightly stale data
- Acceptable for most read operations

### Saga Consistency

- Distributed operations are eventually consistent
- Compensations ensure system-wide consistency
- Each step is locally atomic

## Error Handling

### Command Errors

```rust
pub enum DomainError {
    // Aggregate state doesn't allow operation
    InvalidStateTransition { from: OrderState, operation: String },

    // Business rule violation
    InvalidQuantity,
    OrderEmpty,

    // Concurrency conflict
    ConcurrencyConflict { expected: Version, actual: Version },
}
```

### Resolution Strategies

| Error Type | Strategy |
|------------|----------|
| Validation | Return error to client |
| Concurrency | Retry with fresh state |
| Infrastructure | Retry with backoff |
| Saga Step Failure | Compensate and fail saga |

## Implementation Phases

### Phase 1: Event Store (Complete)
- [x] EventStore trait with PostgreSQL and in-memory implementations
- [x] Event envelope with metadata
- [x] Optimistic concurrency control
- [x] Snapshot support

### Phase 2: Domain Layer (Complete)
- [x] Aggregate and DomainEvent traits
- [x] CommandHandler with load → execute → persist
- [x] Order aggregate with state machine
- [x] Value objects (Money, CustomerId, etc.)

### Phase 3: Read Models (Complete)
- [x] Projection trait and ProjectionProcessor
- [x] CurrentOrdersView (active orders)
- [x] OrderHistoryView (completed/cancelled with staging pattern)
- [x] CustomerOrdersView (per-customer stats and spending)
- [x] InventoryView (product demand tracking)

### Phase 4: Saga Pattern (Complete)
- [x] SagaCoordinator with orchestration pattern
- [x] OrderFulfillmentSaga (inventory → payment → shipping)
- [x] Compensating transactions in reverse order
- [x] Event-sourced SagaInstance aggregate for recovery

### Phase 5: Observability & API Server (Complete)
- [x] Structured logging with `tracing` and `#[instrument]`
- [x] Prometheus metrics (events_appended, commands_executed/failed, saga metrics)
- [x] Axum HTTP server with REST API
- [x] Health check and metrics endpoints
- [x] Order CRUD + saga trigger endpoints
- [x] API integration tests

### Phase 6: Production Ready (In Progress)
- [x] Configuration management
- [x] Graceful shutdown
- [x] Postgres integration (runtime-swappable event store)
- [ ] Performance optimization
- [ ] Documentation completion

## Runtime Event Store Selection

The API server selects its event store implementation at startup based on environment configuration:

- **`DATABASE_URL` set** — connects to PostgreSQL via `PostgresEventStore::connect()`, which creates a connection pool and runs migrations automatically.
- **`DATABASE_URL` not set** — falls back to `InMemoryEventStore` (useful for development and testing).

The entire API layer (`AppState`, route handlers, `create_app`, `create_default_state`) is generic over `S: EventStore + Clone`. This means both stores satisfy the same interface with zero runtime overhead from dynamic dispatch — the concrete type is monomorphized at compile time, and type erasure happens at the `Router` level.

```
DATABASE_URL=postgres://... → PostgresEventStore (persistent, production)
(not set)                   → InMemoryEventStore  (ephemeral, development)
```

Related configuration:
- `DB_MAX_CONNECTIONS` — max pool connections (default: 10)

## Snapshot Readiness Assessment

The snapshot infrastructure is **fully implemented** but not yet wired to the Order aggregate:

### Already in place
- `Snapshot` struct in `event-store/src/snapshot.rs`
- `SnapshotCapable` trait in `domain/src/aggregate.rs`
- `CommandHandler::execute_with_snapshot()` — loads from snapshot + remaining events
- `EventStore::save_snapshot()` / `get_snapshot()` — persist and retrieve snapshots
- `EventStoreExt::load_aggregate()` — returns snapshot + events-after-snapshot
- Full PostgreSQL implementation with `snapshots` table

### Remaining work (small)
1. `impl SnapshotCapable for Order {}` — one line; `Order` already derives `Serialize + Deserialize`
2. Switch `OrderService` methods from `execute()` to `execute_with_snapshot()`
3. Add integration tests verifying snapshot creation and restore

### Priority assessment
**Not urgent.** Orders typically have a small number of events (create, add items, submit, fulfill/cancel). Snapshots become valuable at hundreds or thousands of events per aggregate, which is unlikely for this domain. The infrastructure is ready for when it's needed.

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Event Store | PostgreSQL | ACID guarantees, familiar tooling |
| Serialization | JSON | Human-readable, flexible schema |
| Concurrency | Optimistic locking | Simple, scales well for most cases |
| Saga Style | Orchestration | Clearer flow, easier debugging |
| Read Models | Separate tables | Optimized queries, independent scaling |
| Money | Cents (i64) | Avoid floating-point precision issues |
| IDs | Newtype wrappers | Type safety, prevent ID confusion |
| HTTP Framework | Axum | Tower-compatible, async, ergonomic extractors |
| Logging | tracing + #[instrument] | Span-based, structured, non-invasive |
| Metrics | metrics + Prometheus exporter | Lightweight, Prometheus-native |
| Configuration | Env vars + Config struct | 12-factor app, discoverable defaults |
| Graceful Shutdown | tokio::signal + with_graceful_shutdown | Drains in-flight requests on SIGINT/SIGTERM |
| Event Store Selection | Generic AppState + env-based branching | Postgres for production, in-memory for dev/test, zero dynamic dispatch |

## Further Reading

- [Event Sourcing](./event-sourcing.md) - Deep dive into event sourcing
- [CQRS](./cqrs.md) - Command Query Responsibility Segregation
- [Saga Pattern](./saga-pattern.md) - Distributed transaction coordination
