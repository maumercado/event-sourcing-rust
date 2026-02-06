# Event Sourcing in Rust

[![CI](https://github.com/maumercado/event-sourcing-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/maumercado/event-sourcing-rust/actions/workflows/ci.yml)

An event-sourced order management system demonstrating Event Sourcing, CQRS, and the Saga pattern in Rust.

## Features

- **Event Store**: Append-only event storage with PostgreSQL backend
- **Domain Layer**: Order aggregate with state machine and command handling
- **CQRS**: Command and Query Responsibility Segregation with read model projections
- **Projections**: Four read model views (Current Orders, Order History, Customer Stats, Inventory Demand)
- **Saga Pattern**: Multi-step distributed transactions with compensation
- **Optimistic Concurrency**: Version-based conflict detection
- **Snapshots**: Aggregate state caching for performance

## Quick Start

### Prerequisites

- Rust 1.85+ (2024 edition)
- Docker (for integration tests via testcontainers)
- PostgreSQL client (optional, for manual queries)

### Setup

1. Clone the repository:
```bash
git clone https://github.com/maumercado/event-sourcing-rust.git
cd event-sourcing-rust
```

2. Build and test:
```bash
cargo build
cargo test
```

> **Note**: Integration tests automatically start PostgreSQL via testcontainers. Docker must be running.

### Running Tests

```bash
# Unit tests only (fast)
cargo test --lib

# Domain integration tests
cargo test -p domain --test order_integration

# Postgres integration tests (requires Docker via testcontainers)
cargo test -p event-store --test postgres_integration

# Projection tests
cargo test -p projections

# All tests (169 total)
cargo test
```

> **Note**: Integration tests use [testcontainers](https://github.com/testcontainers/testcontainers-rs) to automatically spin up PostgreSQL in Docker. No manual setup required.

## Project Structure

```
event-sourcing-rust/
├── .github/
│   └── workflows/
│       └── ci.yml        # GitHub Actions CI pipeline
├── crates/
│   ├── common/           # Shared types (AggregateId)
│   ├── event-store/      # Event store implementation
│   │   ├── src/
│   │   │   ├── event.rs      # EventEnvelope, EventId, Version
│   │   │   ├── store.rs      # EventStore trait
│   │   │   ├── postgres.rs   # PostgreSQL implementation
│   │   │   ├── memory.rs     # In-memory implementation (testing)
│   │   │   ├── snapshot.rs   # Snapshot support
│   │   │   ├── query.rs      # EventQuery builder
│   │   │   └── error.rs      # Error types
│   │   └── tests/
│   │       └── postgres_integration.rs
│   ├── domain/           # Domain layer (Phase 2)
│   │   ├── src/
│   │   │   ├── aggregate.rs  # Aggregate, DomainEvent traits
│   │   │   ├── command.rs    # Command, CommandHandler
│   │   │   ├── error.rs      # DomainError
│   │   │   └── order/        # Order aggregate
│   │   │       ├── aggregate.rs    # Order struct
│   │   │       ├── state.rs        # OrderState enum
│   │   │       ├── events.rs       # OrderEvent variants
│   │   │       ├── commands.rs     # Command structs
│   │   │       ├── service.rs      # OrderService
│   │   │       └── value_objects.rs
│   │   └── tests/
│   │       └── order_integration.rs
│   └── projections/      # CQRS read side (Phase 3)
│       ├── src/
│       │   ├── error.rs          # ProjectionError
│       │   ├── projection.rs     # Projection trait
│       │   ├── read_model.rs     # ReadModel trait
│       │   ├── processor.rs      # ProjectionProcessor
│       │   └── views/
│       │       ├── current_orders.rs   # Active orders
│       │       ├── order_history.rs    # Completed/cancelled
│       │       ├── customer_orders.rs  # Per-customer stats
│       │       └── inventory.rs        # Product demand
│       └── tests/
│           └── integration.rs
├── migrations/           # SQL migrations
└── docker-compose.yml    # Local PostgreSQL
```

## Architecture

### Event Store

The event store provides:

- **Append-only storage**: Events are immutable once written
- **Optimistic concurrency**: Version-based conflict detection prevents lost updates
- **Flexible queries**: Query by aggregate ID, event type, version range, or timestamp
- **Event streaming**: Stream all events for projections
- **Snapshots**: Cache aggregate state to avoid replaying all events

### Domain Layer (Phase 2)

The domain layer provides:

- **Aggregate trait**: Base trait for event-sourced aggregates
- **CommandHandler**: Generic handler implementing load → execute → persist pattern
- **Order Aggregate**: Complete order management with state machine

#### Order State Machine

```
Draft ──────┬──► Reserved ──► Processing ──► Completed
            │        │            │
            └────────┴────────────┴──► Cancelled
```

- **Draft**: Items can be added/removed
- **Reserved**: Inventory reserved, awaiting payment
- **Processing**: Payment confirmed, being fulfilled
- **Completed**: Shipped (terminal state)
- **Cancelled**: Cancelled at any point (terminal state)

#### Order Events

- `OrderCreated` - Order initialized for a customer
- `ItemAdded` - Product added to order
- `ItemRemoved` - Product removed from order
- `ItemQuantityUpdated` - Quantity changed
- `OrderSubmitted` - Order submitted for processing
- `OrderReserved` - Inventory reserved
- `OrderProcessing` - Payment confirmed
- `OrderCompleted` - Order shipped
- `OrderCancelled` - Order cancelled with reason

### Projections (Phase 3)

The CQRS query side provides denormalized read models updated from events:

- **CurrentOrdersView**: Active (non-terminal) orders with items and totals. Orders removed on completion/cancellation.
- **OrderHistoryView**: Completed and cancelled orders with final metadata (tracking number, cancellation reason).
- **CustomerOrdersView**: Per-customer statistics — order counts, spending, active/completed/cancelled breakdowns.
- **InventoryView**: Product demand across orders — quantities ordered, reserved, completed, and revenue.

The `ProjectionProcessor` feeds events from the event store to all registered projections, supporting catch-up replay, single-event delivery, and full rebuilds.

### Core Types

```rust
// Event envelope with metadata
pub struct EventEnvelope {
    pub event_id: EventId,
    pub event_type: String,
    pub aggregate_id: AggregateId,
    pub aggregate_type: String,
    pub version: Version,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,
    pub metadata: HashMap<String, serde_json::Value>,
}

// EventStore trait
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, events: Vec<EventEnvelope>, options: AppendOptions) -> Result<Version>;
    async fn get_events_for_aggregate(&self, id: AggregateId) -> Result<Vec<EventEnvelope>>;
    async fn get_events_by_type(&self, event_type: &str) -> Result<Vec<EventEnvelope>>;
    // ... more methods
}

// Aggregate trait for event-sourced entities
pub trait Aggregate: Default + Send + Sync + Sized {
    type Event: DomainEvent;
    type Error: std::error::Error + Send + Sync;

    fn aggregate_type() -> &'static str;
    fn id(&self) -> Option<AggregateId>;
    fn version(&self) -> Version;
    fn apply(&mut self, event: Self::Event);  // Pure, deterministic
}
```

### Usage Example

```rust
use domain::{
    CreateOrder, AddItem, OrderService, OrderItem, CustomerId, Money,
};
use event_store::InMemoryEventStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an order service with in-memory store
    let store = InMemoryEventStore::new();
    let service = OrderService::new(store);

    // Create an order
    let customer_id = CustomerId::new();
    let cmd = CreateOrder::for_customer(customer_id);
    let order_id = cmd.order_id;

    service.create_order(cmd).await?;

    // Add items
    service.add_item(AddItem::new(
        order_id,
        OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000)),
    )).await?;

    // Get order
    let order = service.get_order(order_id).await?.unwrap();
    println!("Order total: {}", order.total_amount());  // $20.00

    Ok(())
}
```

## Development

### Code Quality

```bash
# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings

# Type check
cargo check
```

### Conventions

- **Commits**: Follow [Conventional Commits](https://www.conventionalcommits.org/)
- **Coverage**: Target >80%
- **No warnings**: `cargo clippy -- -D warnings` must pass

## Performance Targets

- Event write latency: <10ms (P95)
- Commands: 1,000/sec
- Queries: 10,000/sec on read models
- Read model lag: <100ms
- Event replay: 10,000 events/sec

## Documentation

Detailed documentation on the architectural patterns used in this project:

| Document | Description |
|----------|-------------|
| [Event Sourcing](./docs/event-sourcing.md) | Store state as a sequence of immutable events |
| [CQRS](./docs/cqrs.md) | Separate read and write models for scalability |
| [Saga Pattern](./docs/saga-pattern.md) | Manage distributed transactions with compensation |
| [Architecture](./docs/architecture.md) | How all patterns work together |

See the [docs folder](./docs/) for the complete documentation.

## License

MIT License - see [LICENSE](LICENSE) for details.
