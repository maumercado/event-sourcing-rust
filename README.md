# Event Sourcing in Rust

An event-sourced order management system demonstrating Event Sourcing, CQRS, and the Saga pattern in Rust. This is a portfolio project targeting backend engineering roles.

## Features

- **Event Store**: Append-only event storage with PostgreSQL backend
- **CQRS**: Command and Query Responsibility Segregation
- **Saga Pattern**: Multi-step distributed transactions with compensation
- **Optimistic Concurrency**: Version-based conflict detection
- **Snapshots**: Aggregate state caching for performance

## Quick Start

### Prerequisites

- Rust 1.75+ (2024 edition)
- Docker and Docker Compose
- PostgreSQL client (optional, for manual queries)

### Setup

1. Clone the repository:
```bash
git clone https://github.com/yourusername/event-sourcing-rust.git
cd event-sourcing-rust
```

2. Start PostgreSQL:
```bash
docker-compose up -d
```

3. Build and test:
```bash
cargo build
cargo test
```

### Running Tests

```bash
# Unit tests only (fast)
cargo test --lib

# Integration tests (requires Docker)
cargo test --test postgres_integration

# All tests
cargo test
```

## Project Structure

```
event-sourcing-rust/
├── crates/
│   ├── common/           # Shared types (AggregateId)
│   └── event-store/      # Event store implementation
│       ├── src/
│       │   ├── event.rs      # EventEnvelope, EventId, Version
│       │   ├── store.rs      # EventStore trait
│       │   ├── postgres.rs   # PostgreSQL implementation
│       │   ├── memory.rs     # In-memory implementation (testing)
│       │   ├── snapshot.rs   # Snapshot support
│       │   ├── query.rs      # EventQuery builder
│       │   └── error.rs      # Error types
│       └── tests/
│           └── postgres_integration.rs
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
```

### Usage Example

```rust
use event_store::{
    AggregateId, AppendOptions, EventEnvelope, EventStore,
    PostgresEventStore, Version,
};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect("postgres://localhost/eventstore").await?;
    let store = PostgresEventStore::new(pool);

    let aggregate_id = AggregateId::new();

    // Create an event
    let event = EventEnvelope::builder()
        .aggregate_id(aggregate_id)
        .aggregate_type("Order")
        .event_type("OrderCreated")
        .version(Version::first())
        .payload_raw(serde_json::json!({
            "customer_id": "cust-123",
            "items": [{"sku": "ITEM-1", "quantity": 2}]
        }))
        .build();

    // Append with optimistic concurrency check
    store.append(vec![event], AppendOptions::expect_new()).await?;

    // Query events
    let events = store.get_events_for_aggregate(aggregate_id).await?;
    println!("Found {} events", events.len());

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

## Implementation Phases

| Phase | Tag | Focus | Status |
|-------|-----|-------|--------|
| 1 | v0.1.0-phase1 | Event Store Foundation | Complete |
| 2 | v0.2.0-phase2 | Command Handlers & Aggregates | Planned |
| 3 | v0.3.0-phase3 | Read Models & Projections | Planned |
| 4 | v0.4.0-phase4 | Saga Pattern | Planned |
| 5 | v0.5.0-phase5 | Observability & Operations | Planned |
| 6 | v1.0.0 | Production Ready | Planned |

## Performance Targets

- Event write latency: <10ms (P95)
- Commands: 1,000/sec
- Queries: 10,000/sec on read models
- Read model lag: <100ms
- Event replay: 10,000 events/sec

## License

MIT License - see [LICENSE](LICENSE) for details.
