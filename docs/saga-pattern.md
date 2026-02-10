# Saga Pattern

## What is the Saga Pattern?

The Saga pattern manages distributed transactions across multiple services or aggregates. Instead of a single ACID transaction, a saga is a sequence of local transactions where each step publishes events that trigger the next step.

```
Traditional Transaction:
┌─────────────────────────────────────────────────┐
│ BEGIN TRANSACTION                               │
│   1. Reserve Inventory                          │
│   2. Charge Payment                             │
│   3. Schedule Shipping                          │
│ COMMIT (all or nothing)                         │
└─────────────────────────────────────────────────┘

Saga:
┌───────────┐    ┌───────────┐    ┌───────────┐
│ Reserve   │───▶│  Charge   │───▶│ Schedule  │
│ Inventory │    │  Payment  │    │ Shipping  │
└───────────┘    └───────────┘    └───────────┘
      │                │                │
      ▼                ▼                ▼
  Compensate       Compensate       (done)
  if needed        if needed
```

## Why Sagas?

### The Problem

In distributed systems, traditional ACID transactions don't work:

- Services have separate databases
- Network calls can fail or timeout
- Locking resources across services causes bottlenecks
- Two-phase commit (2PC) has availability problems

### The Solution

Sagas provide **eventual consistency** through:

1. **Local Transactions**: Each step commits to its own database
2. **Compensating Transactions**: Undo completed steps if a later step fails
3. **Event-Driven Coordination**: Steps communicate through events

## Saga Coordination Styles

### Choreography

Each service listens to events and decides what to do next. No central coordinator.

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│   Order     │         │  Inventory  │         │   Payment   │
│   Service   │         │   Service   │         │   Service   │
└──────┬──────┘         └──────┬──────┘         └──────┬──────┘
       │                       │                       │
       │ OrderCreated          │                       │
       │──────────────────────▶│                       │
       │                       │ InventoryReserved     │
       │                       │──────────────────────▶│
       │                       │                       │
       │                       │◀──────────────────────│
       │ PaymentConfirmed      │   PaymentConfirmed    │
       │◀──────────────────────│                       │
```

**Pros**: Simple, loosely coupled, no single point of failure
**Cons**: Hard to understand flow, difficult to debug, cyclic dependencies possible

### Orchestration

A central coordinator (saga orchestrator) tells each service what to do.

```
┌─────────────────────────────────────────────────────────┐
│                    Saga Orchestrator                     │
│                                                          │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐          │
│  │  Step 1  │───▶│  Step 2  │───▶│  Step 3  │          │
│  │ Reserve  │    │  Charge  │    │   Ship   │          │
│  └──────────┘    └──────────┘    └──────────┘          │
└────────┬─────────────┬─────────────────┬────────────────┘
         │             │                 │
         ▼             ▼                 ▼
    ┌─────────┐   ┌─────────┐      ┌─────────┐
    │Inventory│   │ Payment │      │Shipping │
    │ Service │   │ Service │      │ Service │
    └─────────┘   └─────────┘      └─────────┘
```

**Pros**: Clear flow, easier to debug, handles complex logic
**Cons**: Coordinator is a single point of failure, more coupling

**This project uses orchestration** for clarity and maintainability.

## Core Concepts

### Saga Definition

A saga defines the sequence of steps and their compensations:

```rust
// Planned for Phase 4: crates/saga/src/definition.rs
pub struct SagaDefinition<C> {
    pub name: &'static str,
    pub steps: Vec<SagaStep<C>>,
}

pub struct SagaStep<C> {
    pub name: &'static str,
    pub action: Box<dyn StepAction<C>>,
    pub compensation: Option<Box<dyn StepAction<C>>>,
}
```

### Saga State

Track the progress of each saga instance:

```rust
pub enum SagaState {
    Running,              // Executing steps
    Compensating,         // A step failed, undoing previous steps
    Completed,            // All steps succeeded
    Failed,               // Compensation complete (or failed)
}

pub struct SagaInstance {
    pub saga_id: SagaId,
    pub saga_type: String,
    pub state: SagaState,
    pub current_step: usize,
    pub completed_steps: Vec<String>,
    pub context: serde_json::Value,  // Saga-specific data
}
```

### Compensating Transactions

Each step that modifies state must have a compensation:

| Step | Action | Compensation |
|------|--------|--------------|
| Reserve Inventory | Decrement available stock | Increment available stock |
| Charge Payment | Create charge | Refund charge |
| Create Shipment | Book carrier | Cancel booking |

**Key properties of compensations:**

- **Semantic undo**: May not restore exact original state (refund vs. never charged)
- **Idempotent**: Safe to run multiple times
- **Never fail**: Should always succeed (or retry indefinitely)

## Order Fulfillment Saga (Planned)

```
┌─────────────────────────────────────────────────────────────────┐
│                    OrderFulfillmentSaga                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐    │
│  │   Reserve    │────▶│   Process    │────▶│   Create     │    │
│  │  Inventory   │     │   Payment    │     │  Shipment    │    │
│  └──────┬───────┘     └──────┬───────┘     └──────────────┘    │
│         │                    │                                   │
│         │ On Failure         │ On Failure                       │
│         ▼                    ▼                                   │
│  ┌──────────────┐     ┌──────────────┐                         │
│  │   Release    │◀────│    Refund    │                         │
│  │  Inventory   │     │   Payment    │                         │
│  └──────────────┘     └──────────────┘                         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Step Details

**Step 1: Reserve Inventory**
```rust
// Action
async fn reserve_inventory(ctx: &mut OrderContext) -> Result<(), SagaError> {
    for item in &ctx.order.items {
        inventory_service.reserve(item.product_id, item.quantity).await?;
    }
    Ok(())
}

// Compensation
async fn release_inventory(ctx: &mut OrderContext) -> Result<(), SagaError> {
    for item in &ctx.order.items {
        inventory_service.release(item.product_id, item.quantity).await?;
    }
    Ok(())
}
```

**Step 2: Process Payment**
```rust
// Action
async fn process_payment(ctx: &mut OrderContext) -> Result<(), SagaError> {
    let charge = payment_service.charge(ctx.customer_id, ctx.total).await?;
    ctx.charge_id = Some(charge.id);
    Ok(())
}

// Compensation
async fn refund_payment(ctx: &mut OrderContext) -> Result<(), SagaError> {
    if let Some(charge_id) = ctx.charge_id {
        payment_service.refund(charge_id).await?;
    }
    Ok(())
}
```

**Step 3: Create Shipment**
```rust
// Action (no compensation needed - it's the last step)
async fn create_shipment(ctx: &mut OrderContext) -> Result<(), SagaError> {
    let shipment = shipping_service.create(ctx.order_id, ctx.shipping_address).await?;
    ctx.shipment_id = Some(shipment.id);
    Ok(())
}
```

## Handling Failures

### Forward Recovery

For transient failures (network timeout, temporary unavailability):

```rust
impl SagaStep {
    pub fn with_retry(mut self, max_attempts: u32, backoff: Duration) -> Self {
        self.retry_policy = RetryPolicy::exponential(max_attempts, backoff);
        self
    }
}
```

### Backward Recovery (Compensation)

For permanent failures (insufficient funds, out of stock):

```
Step 1 ✓    Step 2 ✓    Step 3 ✗
   │           │           │
   │           │           │ Failed
   │           │◀──────────┘
   │           │ Compensate Step 2
   │◀──────────┘
   │ Compensate Step 1
   ▼
Saga Failed (but consistent)
```

### Pivot Point

Some sagas have a "point of no return" after which compensation is not possible:

```
┌────────────┐    ┌────────────┐    ┌────────────┐
│ Retryable  │───▶│   PIVOT    │───▶│  Retryable │
│            │    │   POINT    │    │  (no comp) │
└────────────┘    └────────────┘    └────────────┘

Before pivot: Can compensate backward
After pivot: Must retry forward until success
```

## Idempotency

Sagas must handle duplicate execution (network retries, restarts):

```rust
pub struct SagaContext {
    pub saga_id: SagaId,
    pub idempotency_keys: HashMap<String, String>,
}

impl SagaContext {
    /// Generate idempotency key for external calls
    pub fn key_for(&self, step: &str) -> String {
        format!("{}:{}", self.saga_id, step)
    }
}

// Usage
async fn process_payment(ctx: &mut SagaContext) -> Result<(), SagaError> {
    let idempotency_key = ctx.key_for("payment");
    payment_service.charge_idempotent(idempotency_key, amount).await
}
```

## Saga Events

Sagas emit events for observability and auditing:

```rust
pub enum SagaEvent {
    SagaStarted { saga_id: SagaId, saga_type: String },
    StepStarted { saga_id: SagaId, step: String },
    StepCompleted { saga_id: SagaId, step: String },
    StepFailed { saga_id: SagaId, step: String, error: String },
    CompensationStarted { saga_id: SagaId },
    CompensationCompleted { saga_id: SagaId, step: String },
    SagaCompleted { saga_id: SagaId },
    SagaFailed { saga_id: SagaId, reason: String },
}
```

## Implementation in This Project

### Current Status

| Component | Status | Location |
|-----------|--------|----------|
| SagaState | ✅ Complete | `crates/saga/src/state.rs` |
| SagaEvent | ✅ Complete | `crates/saga/src/events.rs` |
| SagaInstance (Aggregate) | ✅ Complete | `crates/saga/src/aggregate.rs` |
| SagaCoordinator | ✅ Complete | `crates/saga/src/coordinator.rs` |
| External Service Traits | ✅ Complete | `crates/saga/src/services/` |
| Order Fulfillment Constants | ✅ Complete | `crates/saga/src/order_fulfillment.rs` |

### Architecture

```rust
// Setup
let store = InMemoryEventStore::new();
let coordinator = SagaCoordinator::new(
    store.clone(),
    inventory_service,
    payment_service,
    shipping_service,
);

// Execute saga for an order
let saga_id = coordinator.execute_saga(order_id).await?;

// Load saga state (event-sourced)
let saga = coordinator.get_saga(saga_id).await?;
assert_eq!(saga.state(), SagaState::Completed);
```

## Best Practices

1. **Design compensations first**: Before implementing a step, define how to undo it

2. **Make steps idempotent**: Use idempotency keys for external calls

3. **Keep steps small**: Easier to compensate and retry

4. **Log everything**: Saga debugging requires detailed logs

5. **Set timeouts**: Prevent sagas from hanging indefinitely

6. **Handle partial failures**: A step might partially succeed

7. **Test compensation paths**: They're rarely exercised in production

## Further Reading

- [Event Sourcing](./event-sourcing.md) - Foundation for saga events
- [CQRS](./cqrs.md) - Commands trigger sagas
- [Architecture](./architecture.md) - How sagas fit in the system
