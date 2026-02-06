# Documentation

This documentation explains the architectural patterns used in this project and how they work together.

## Patterns

| Pattern | Description | Status |
|---------|-------------|--------|
| [Event Sourcing](./event-sourcing.md) | Store state as a sequence of events | Implemented |
| [CQRS](./cqrs.md) | Separate read and write models | Implemented |
| [Saga Pattern](./saga-pattern.md) | Manage distributed transactions | Planned (Phase 4) |
| [Architecture](./architecture.md) | How patterns work together | Overview |

## Architecture Overview

See [Architecture](./architecture.md) for how these patterns work together in this system, including data flow diagrams and implementation details.

## Quick Navigation

### By Concept

- **Want to understand Event Sourcing?** → Start with [Event Sourcing](./event-sourcing.md), then look at `crates/event-store/`
- **Want to understand Aggregates?** → Read [Event Sourcing § Aggregates](./event-sourcing.md#aggregates), then look at `crates/domain/src/order/aggregate.rs`
- **Want to understand CQRS?** → Start with [CQRS](./cqrs.md), then look at command handling in `crates/domain/src/command.rs`
- **Want to understand Sagas?** → Read [Saga Pattern](./saga-pattern.md) (implementation coming in Phase 4)

### By Code Location

| Directory | Purpose | Related Docs |
|-----------|---------|--------------|
| `crates/event-store/` | Event persistence layer | [Event Sourcing](./event-sourcing.md) |
| `crates/domain/` | Domain logic, aggregates, commands | [Event Sourcing](./event-sourcing.md), [CQRS](./cqrs.md) |
| `crates/domain/src/order/` | Order aggregate implementation | [Architecture](./architecture.md) |
| `crates/projections/` | CQRS read models and projections | [CQRS](./cqrs.md), [Architecture](./architecture.md) |
