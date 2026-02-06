# CQRS (Command Query Responsibility Segregation)

## What is CQRS?

CQRS is an architectural pattern that separates read operations (queries) from write operations (commands) into different models.

```
Traditional CRUD:
┌─────────────┐
│   Client    │
└──────┬──────┘
       │ Create/Read/Update/Delete
       ▼
┌─────────────┐
│  Database   │
└─────────────┘

CQRS:
┌─────────────┐
│   Client    │
└──────┬──────┘
       │
   ┌───┴───┐
   │       │
   ▼       ▼
┌─────┐ ┌─────┐
│Write│ │Read │
│Model│ │Model│
└──┬──┘ └──┬──┘
   │       │
   ▼       ▼
┌─────┐ ┌─────┐
│Write│ │Read │
│ DB  │ │ DB  │
└─────┘ └─────┘
```

## Why CQRS?

### Benefits

1. **Optimized Models**: Read model optimized for queries, write model optimized for business logic.

2. **Scalability**: Scale reads and writes independently. Most systems are read-heavy.

3. **Simplified Queries**: Read models can be denormalized for fast, simple queries.

4. **Flexibility**: Different storage technologies for different needs (e.g., SQL for writes, Elasticsearch for searches).

5. **Natural Fit with Event Sourcing**: Events from the write side update the read models.

### Trade-offs

1. **Eventual Consistency**: Read models may be slightly behind the write model.

2. **Complexity**: Two models to maintain instead of one.

3. **Synchronization**: Need a mechanism to keep read models updated.

## Core Concepts

### Commands

Commands are **intentions to change state**. They are named in imperative form:

- `CreateOrder` (not `OrderCreated`)
- `AddItem` (not `ItemAdded`)
- `CancelOrder` (not `OrderCancelled`)

Commands can be **rejected** if business rules aren't satisfied.

```rust
// In this project: crates/domain/src/order/commands.rs
pub struct CreateOrder {
    pub order_id: AggregateId,
    pub customer_id: CustomerId,
}

pub struct AddItem {
    pub order_id: AggregateId,
    pub item: OrderItem,
}

pub struct CancelOrder {
    pub order_id: AggregateId,
    pub reason: String,
}
```

### Command Handlers

Command handlers process commands through a consistent flow:

1. **Load** the aggregate from the event store
2. **Validate** the command against current state
3. **Execute** to produce new events
4. **Persist** events to the event store

```rust
// In this project: crates/domain/src/command.rs
impl<S, A> CommandHandler<S, A> {
    pub async fn execute<F>(
        &self,
        aggregate_id: AggregateId,
        command_fn: F,
    ) -> Result<CommandResult<A>, DomainError>
    where
        F: FnOnce(&A) -> Result<Vec<A::Event>, A::Error>,
    {
        // 1. Load aggregate
        let mut aggregate = self.load(aggregate_id).await?;

        // 2 & 3. Validate and execute (in command_fn)
        let events = command_fn(&aggregate)?;

        // 4. Persist with optimistic concurrency
        let new_version = self.store.append(envelopes, options).await?;

        // Apply events to return updated aggregate
        for event in &events {
            aggregate.apply(event.clone());
        }

        Ok(CommandResult { aggregate, events, new_version })
    }
}
```

### Queries and Read Models

Queries retrieve data optimized for specific use cases. Read models (projections) are denormalized views built from events.

```
Events:                          Read Models:
┌──────────────────┐            ┌─────────────────────┐
│ OrderCreated     │───────────▶│ CurrentOrdersView   │
│ ItemAdded        │            │ - order_id          │
│ ItemAdded        │            │ - customer_name     │
│ OrderSubmitted   │            │ - item_count        │
│ PaymentReceived  │            │ - total             │
│ OrderShipped     │            │ - status            │
└──────────────────┘            └─────────────────────┘
                                         │
                                ┌────────┴────────┐
                                ▼                 ▼
                    ┌─────────────────┐ ┌─────────────────┐
                    │ CustomerOrders  │ │ OrderHistory    │
                    │ - customer_id   │ │ - date          │
                    │ - order_count   │ │ - order_id      │
                    │ - total_spent   │ │ - events[]      │
                    └─────────────────┘ └─────────────────┘
```

Read models are **eventually consistent** with the write model. They subscribe to events and update themselves.

## CQRS + Event Sourcing

When combined with Event Sourcing, CQRS becomes powerful:

```
┌─────────────────────────────────────────────────────────────┐
│                      Command Side                            │
│  ┌─────────┐    ┌─────────────┐    ┌─────────────────────┐  │
│  │ Command │───▶│   Handler   │───▶│    Event Store      │  │
│  └─────────┘    └─────────────┘    │  (append-only log)  │  │
│                        │           └──────────┬──────────┘  │
│                        ▼                      │             │
│                 ┌─────────────┐               │             │
│                 │  Aggregate  │               │             │
│                 │ (validates) │               │             │
│                 └─────────────┘               │             │
└──────────────────────────────────────────────│─────────────┘
                                               │
                                               │ Events
                                               ▼
┌─────────────────────────────────────────────────────────────┐
│                       Query Side                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                   Projections                        │    │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────────┐   │    │
│  │  │ OrderList │  │ Customer  │  │   Inventory   │   │    │
│  │  │   View    │  │  Orders   │  │     View      │   │    │
│  │  └───────────┘  └───────────┘  └───────────────┘   │    │
│  └─────────────────────────────────────────────────────┘    │
│                            │                                 │
│                            ▼                                 │
│  ┌─────────┐    ┌─────────────────────────────────────┐     │
│  │  Query  │◀───│        Read Database(s)             │     │
│  └─────────┘    └─────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

### Benefits of the Combination

1. **Single Source of Truth**: Events are the source of truth
2. **Multiple Views**: Build any read model from the same events
3. **Replay**: Rebuild read models by replaying events
4. **Audit**: Complete history in the event store

## Implementation in This Project

### Current Status

| Component | Status | Location |
|-----------|--------|----------|
| Commands | ✅ Implemented | `crates/domain/src/order/commands.rs` |
| Command Handler | ✅ Implemented | `crates/domain/src/command.rs` |
| Aggregates | ✅ Implemented | `crates/domain/src/order/aggregate.rs` |
| Read Models | 🔜 Phase 3 | Coming soon |
| Projections | 🔜 Phase 3 | Coming soon |

### Command Flow Example

```rust
// 1. Create command
let cmd = CreateOrder::for_customer(customer_id);

// 2. Execute through service (which uses CommandHandler)
let result = service.create_order(cmd).await?;

// 3. Result contains updated aggregate and produced events
println!("Order created: {:?}", result.aggregate.id());
println!("Events: {:?}", result.events);
```

### Planned Read Models (Phase 3)

- **CurrentOrdersView**: Active orders with status
- **OrderHistoryView**: Complete order history
- **CustomerOrdersView**: Orders by customer
- **InventoryView**: Product availability

## Further Reading

- [Event Sourcing](./event-sourcing.md) - The foundation for CQRS
- [Saga Pattern](./saga-pattern.md) - Coordinating across aggregates
- [Architecture](./architecture.md) - How it all fits together
