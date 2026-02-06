# Event Sourcing

## What is Event Sourcing?

Event Sourcing is an architectural pattern where the state of an application is determined by a sequence of events. Instead of storing the current state directly, you store all the events that led to that state.

```
Traditional: Store current state
┌─────────────────────────────┐
│ Order #123                  │
│ Status: Completed           │
│ Items: [{sku: "A", qty: 2}] │
│ Total: $50.00               │
└─────────────────────────────┘

Event Sourcing: Store events
┌─────────────────────────────┐
│ 1. OrderCreated {id: 123}   │
│ 2. ItemAdded {sku: "A", 2}  │
│ 3. OrderSubmitted           │
│ 4. OrderCompleted           │
└─────────────────────────────┘
```

## Why Event Sourcing?

### Benefits

1. **Complete Audit Trail**: Every change is recorded. You know exactly what happened and when.

2. **Time Travel**: Reconstruct the state at any point in time by replaying events up to that moment.

3. **Debugging**: When something goes wrong, you can see the exact sequence of events that led to the problem.

4. **Event-Driven Integration**: Events can be published to other systems, enabling loose coupling.

5. **Business Insights**: Analyze historical data to understand user behavior and business patterns.

### Trade-offs

1. **Complexity**: More moving parts than simple CRUD.

2. **Event Schema Evolution**: Changing event structure requires careful migration strategies.

3. **Eventual Consistency**: Read models may lag behind the write model.

4. **Storage Growth**: Events accumulate over time (mitigated by snapshots).

## Core Concepts

### Events

Events are **immutable facts** that happened in the past. They are named in past tense:

- `OrderCreated` (not `CreateOrder`)
- `ItemAdded` (not `AddItem`)
- `PaymentReceived` (not `ReceivePayment`)

```rust
// In this project: crates/domain/src/order/events.rs
pub enum OrderEvent {
    OrderCreated(OrderCreatedData),
    ItemAdded(ItemAddedData),
    ItemRemoved(ItemRemovedData),
    OrderSubmitted(OrderSubmittedData),
    OrderCompleted(OrderCompletedData),
    OrderCancelled(OrderCancelledData),
    // ...
}
```

### Event Store

The Event Store is an append-only database of events. Key operations:

- **Append**: Add new events (never update or delete)
- **Read**: Get events for an aggregate or by query
- **Stream**: Subscribe to new events

```rust
// In this project: crates/event-store/src/store.rs
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, events: Vec<EventEnvelope>, options: AppendOptions) -> Result<Version>;
    async fn get_events_for_aggregate(&self, id: AggregateId) -> Result<Vec<EventEnvelope>>;
    // ...
}
```

### Aggregates

An Aggregate is a cluster of domain objects that form a consistency boundary. In event sourcing:

1. **Load**: Replay events to reconstruct current state
2. **Execute**: Validate command and produce new events
3. **Apply**: Update internal state from events

```rust
// In this project: crates/domain/src/aggregate.rs
pub trait Aggregate: Default + Send + Sync + Sized {
    type Event: DomainEvent;
    type Error: std::error::Error;

    fn aggregate_type() -> &'static str;
    fn apply(&mut self, event: Self::Event);  // Pure, deterministic
}
```

The `apply` method must be:
- **Pure**: No side effects
- **Deterministic**: Same event always produces same state change

### Event Envelope

Events are wrapped in an envelope containing metadata:

```rust
// In this project: crates/event-store/src/event.rs
pub struct EventEnvelope {
    pub event_id: EventId,           // Unique identifier
    pub event_type: String,          // "OrderCreated", "ItemAdded", etc.
    pub aggregate_id: AggregateId,   // Which aggregate this belongs to
    pub aggregate_type: String,      // "Order", "Customer", etc.
    pub version: Version,            // Sequence number for optimistic concurrency
    pub timestamp: DateTime<Utc>,    // When the event occurred
    pub payload: serde_json::Value,  // The actual event data
    pub metadata: HashMap<String, Value>, // Correlation IDs, user info, etc.
}
```

### Optimistic Concurrency

To prevent lost updates when multiple processes modify the same aggregate:

1. Load aggregate and note current version (e.g., version 5)
2. Execute command and produce events
3. Append events with expected version check
4. If version mismatch → conflict error, retry with fresh state

```rust
// In this project: crates/event-store/src/store.rs
pub struct AppendOptions {
    pub expected_version: Option<Version>,
}

// Usage
store.append(events, AppendOptions::expect_version(Version::new(5))).await?;
```

### Snapshots

For aggregates with many events, replaying all events becomes slow. Snapshots cache the aggregate state at a point in time:

```
Without snapshots: Replay 10,000 events
With snapshots: Load snapshot at event 9,900 + replay 100 events
```

```rust
// In this project: crates/event-store/src/snapshot.rs
pub struct Snapshot {
    pub aggregate_id: AggregateId,
    pub aggregate_type: String,
    pub version: Version,      // Snapshot taken at this version
    pub state: serde_json::Value,  // Serialized aggregate state
}
```

## Implementation in This Project

### Event Store Layer (`crates/event-store/`)

- `EventStore` trait with PostgreSQL and in-memory implementations
- `EventEnvelope` for event metadata
- `Snapshot` support for performance optimization
- `EventQuery` for flexible event retrieval

### Domain Layer (`crates/domain/`)

- `Aggregate` trait for event-sourced entities
- `DomainEvent` trait for domain events
- `CommandHandler` for the load → execute → persist flow
- Order aggregate as a complete example

### Order Aggregate Example

```rust
// Create order
let events = order.create(order_id, customer_id)?;
// Returns: [OrderCreated { order_id, customer_id, created_at }]

// Add item
let events = order.add_item(item)?;
// Returns: [ItemAdded { product_id, quantity, price }]

// Apply events to update state
for event in events {
    order.apply(event);
}
```

## Further Reading

- [CQRS](./cqrs.md) - Separating reads from writes
- [Saga Pattern](./saga-pattern.md) - Managing distributed transactions
- [Architecture](./architecture.md) - How it all fits together
