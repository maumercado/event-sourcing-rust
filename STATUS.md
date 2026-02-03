# Project Status

## Current Phase: Phase 1 - Event Store Foundation

**Status:** Complete
**Tag:** v0.1.0-phase1

### Completed Features

- [x] Workspace structure with `common` and `event-store` crates
- [x] Core types: `AggregateId`, `EventId`, `Version`, `EventEnvelope`
- [x] `EventStore` trait with async operations
- [x] `PostgresEventStore` implementation with SQLx
- [x] `InMemoryEventStore` for testing
- [x] Optimistic concurrency control via version checking
- [x] Event queries by aggregate ID, type, version range
- [x] Snapshot support for aggregate state caching
- [x] Comprehensive test suite (unit + integration)
- [x] Docker Compose for local PostgreSQL

### Test Coverage

- **Unit tests:** 24 tests covering all core types and in-memory store
- **Integration tests:** 17 tests covering PostgreSQL operations

Run tests:
```bash
# Unit tests
cargo test --lib

# Integration tests (requires Docker)
cargo test -p event-store --test postgres_integration -- --test-threads=1

# All unit tests (fast)
cargo test --lib
```

## Upcoming Phases

### Phase 2: Command Handlers & Aggregates
- Order aggregate with state machine
- Command validation and handling
- Domain event definitions

### Phase 3: Read Models & Projections
- Current orders projection
- Order history projection
- Customer orders view
- Inventory view

### Phase 4: Saga Pattern
- Order fulfillment saga
- Inventory reservation step
- Payment processing step
- Shipping coordination
- Compensating transactions

### Phase 5: Observability & Operations
- Structured logging
- Metrics collection
- Distributed tracing
- Health checks

### Phase 6: Production Ready
- Performance optimization
- Error recovery
- Documentation
- Deployment configuration

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| Event write latency (P95) | <10ms | Not measured |
| Commands/sec | 1,000 | Not measured |
| Queries/sec | 10,000 | Not measured |
| Read model lag | <100ms | Not measured |
| Event replay | 10,000/sec | Not measured |

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Storage | PostgreSQL | Production-realistic, JSONB support |
| Async | Tokio | Standard for Rust web services |
| DB Driver | SQLx | Compile-time checked queries |
| Errors | thiserror | Typed errors for library crate |
| Serialization | serde_json | Human-readable, JSONB compatible |
