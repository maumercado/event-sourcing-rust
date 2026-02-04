//! Command handling infrastructure.

use std::marker::PhantomData;

use common::AggregateId;
use event_store::{AppendOptions, EventEnvelope, EventStore, EventStoreExt, Snapshot, Version};
use serde::Serialize;

use crate::aggregate::{Aggregate, DomainEvent, SnapshotCapable};
use crate::error::DomainError;

/// Result of command execution.
#[derive(Debug)]
pub struct CommandResult<A: Aggregate> {
    /// The aggregate after applying the new events.
    pub aggregate: A,

    /// The events that were generated and persisted.
    pub events: Vec<A::Event>,

    /// The new version of the aggregate after the command.
    pub new_version: Version,
}

/// Trait for commands that can be executed against an aggregate.
///
/// Commands represent an intention to perform an action. They may be rejected
/// if the aggregate's current state doesn't allow the action.
pub trait Command: Send + Sync {
    /// The type of aggregate this command targets.
    type Aggregate: Aggregate;

    /// Returns the ID of the aggregate this command targets.
    fn aggregate_id(&self) -> AggregateId;
}

/// Handler for executing commands against aggregates.
///
/// The handler is responsible for:
/// 1. Loading the aggregate from the event store (with optional snapshot)
/// 2. Executing the command to produce events
/// 3. Persisting the events to the event store
/// 4. Optionally saving a snapshot
pub struct CommandHandler<S, A>
where
    S: EventStore,
    A: Aggregate,
{
    store: S,
    _phantom: PhantomData<A>,
}

impl<S, A> CommandHandler<S, A>
where
    S: EventStore,
    A: Aggregate,
{
    /// Creates a new command handler with the given event store.
    pub fn new(store: S) -> Self {
        Self {
            store,
            _phantom: PhantomData,
        }
    }

    /// Returns a reference to the underlying event store.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Loads an aggregate from the event store.
    ///
    /// If the aggregate doesn't exist, returns a default instance.
    pub async fn load(&self, aggregate_id: AggregateId) -> Result<A, DomainError>
    where
        A: for<'de> serde::Deserialize<'de>,
        A::Event: for<'de> serde::Deserialize<'de>,
    {
        let (snapshot, events) = self.store.load_aggregate(aggregate_id).await?;

        let mut aggregate = if let Some(snapshot) = snapshot {
            self.restore_from_snapshot(snapshot)?
        } else {
            A::default()
        };

        // Apply events after snapshot
        for envelope in events {
            let event: A::Event = serde_json::from_value(envelope.payload)?;
            aggregate.apply(event);
            aggregate.set_version(envelope.version);
        }

        Ok(aggregate)
    }

    /// Loads an aggregate, returning None if it doesn't exist.
    pub async fn load_existing(&self, aggregate_id: AggregateId) -> Result<Option<A>, DomainError>
    where
        A: for<'de> serde::Deserialize<'de>,
        A::Event: for<'de> serde::Deserialize<'de>,
    {
        let aggregate = self.load(aggregate_id).await?;
        if aggregate.id().is_some() {
            Ok(Some(aggregate))
        } else {
            Ok(None)
        }
    }

    /// Executes a command and persists the resulting events.
    ///
    /// The command function receives the current aggregate state and returns
    /// either a list of events to apply, or an error.
    pub async fn execute<F>(
        &self,
        aggregate_id: AggregateId,
        command_fn: F,
    ) -> Result<CommandResult<A>, DomainError>
    where
        A: for<'de> serde::Deserialize<'de>,
        A::Event: for<'de> serde::Deserialize<'de> + Serialize,
        F: FnOnce(&A) -> Result<Vec<A::Event>, A::Error>,
        DomainError: From<A::Error>,
    {
        let mut aggregate = self.load(aggregate_id).await?;
        let current_version = aggregate.version();

        // Execute command to get events
        let events = command_fn(&aggregate)?;

        if events.is_empty() {
            return Ok(CommandResult {
                aggregate,
                events: vec![],
                new_version: current_version,
            });
        }

        // Build envelopes for persistence
        let envelopes = self.build_envelopes(aggregate_id, current_version, &events)?;

        // Persist events with optimistic concurrency
        let options = if current_version == Version::initial() {
            AppendOptions::expect_new()
        } else {
            AppendOptions::expect_version(current_version)
        };

        let new_version = self.store.append(envelopes, options).await?;

        // Apply events to aggregate
        for event in &events {
            aggregate.apply(event.clone());
        }
        aggregate.set_version(new_version);

        Ok(CommandResult {
            aggregate,
            events,
            new_version,
        })
    }

    /// Builds event envelopes from domain events.
    fn build_envelopes(
        &self,
        aggregate_id: AggregateId,
        current_version: Version,
        events: &[A::Event],
    ) -> Result<Vec<EventEnvelope>, DomainError>
    where
        A::Event: Serialize,
    {
        let mut envelopes = Vec::with_capacity(events.len());
        let mut version = current_version;

        for event in events {
            version = version.next();
            let envelope = EventEnvelope::builder()
                .aggregate_id(aggregate_id)
                .aggregate_type(A::aggregate_type())
                .event_type(event.event_type())
                .version(version)
                .payload(event)?
                .build();
            envelopes.push(envelope);
        }

        Ok(envelopes)
    }

    fn restore_from_snapshot(&self, snapshot: Snapshot) -> Result<A, DomainError>
    where
        A: for<'de> serde::Deserialize<'de>,
    {
        let aggregate: A = serde_json::from_value(snapshot.state)?;
        Ok(aggregate)
    }
}

impl<S, A> CommandHandler<S, A>
where
    S: EventStore,
    A: SnapshotCapable,
{
    /// Executes a command and optionally saves a snapshot.
    pub async fn execute_with_snapshot<F>(
        &self,
        aggregate_id: AggregateId,
        command_fn: F,
    ) -> Result<CommandResult<A>, DomainError>
    where
        A: for<'de> serde::Deserialize<'de>,
        A::Event: for<'de> serde::Deserialize<'de> + Serialize,
        F: FnOnce(&A) -> Result<Vec<A::Event>, A::Error>,
        DomainError: From<A::Error>,
    {
        let result = self.execute(aggregate_id, command_fn).await?;

        // Save snapshot if needed
        if result.aggregate.should_snapshot() {
            let snapshot = Snapshot::from_state(
                aggregate_id,
                A::aggregate_type(),
                result.new_version,
                &result.aggregate,
            )?;
            self.store.save_snapshot(snapshot).await?;
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use event_store::InMemoryEventStore;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestEvent {
        Created { name: String },
        Updated { value: i32 },
    }

    impl DomainEvent for TestEvent {
        fn event_type(&self) -> &'static str {
            match self {
                TestEvent::Created { .. } => "TestCreated",
                TestEvent::Updated { .. } => "TestUpdated",
            }
        }
    }

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    struct TestAggregate {
        id: Option<AggregateId>,
        name: String,
        value: i32,
        version: Version,
    }

    #[derive(Debug, thiserror::Error)]
    enum TestError {
        #[error("not initialized")]
        NotInitialized,
        #[error("invalid value: {0}")]
        InvalidValue(i32),
    }

    impl Aggregate for TestAggregate {
        type Event = TestEvent;
        type Error = TestError;

        fn aggregate_type() -> &'static str {
            "TestAggregate"
        }

        fn id(&self) -> Option<AggregateId> {
            self.id
        }

        fn version(&self) -> Version {
            self.version
        }

        fn set_version(&mut self, version: Version) {
            self.version = version;
        }

        fn apply(&mut self, event: Self::Event) {
            match event {
                TestEvent::Created { name } => {
                    if self.id.is_none() {
                        self.id = Some(AggregateId::new());
                    }
                    self.name = name;
                }
                TestEvent::Updated { value } => {
                    self.value = value;
                }
            }
        }
    }

    impl From<TestError> for DomainError {
        fn from(e: TestError) -> Self {
            DomainError::AggregateNotFound {
                aggregate_type: "TestAggregate",
                aggregate_id: format!("{:?}", e),
            }
        }
    }

    #[tokio::test]
    async fn test_execute_creates_aggregate() {
        let store = InMemoryEventStore::new();
        let handler: CommandHandler<_, TestAggregate> = CommandHandler::new(store);
        let aggregate_id = AggregateId::new();

        let result = handler
            .execute(aggregate_id, |_agg| {
                Ok(vec![TestEvent::Created {
                    name: "Test".to_string(),
                }])
            })
            .await
            .unwrap();

        assert_eq!(result.events.len(), 1);
        assert_eq!(result.new_version, Version::first());
        assert!(result.aggregate.id().is_some());
        assert_eq!(result.aggregate.name, "Test");
    }

    #[tokio::test]
    async fn test_execute_updates_aggregate() {
        let store = InMemoryEventStore::new();
        let handler: CommandHandler<_, TestAggregate> = CommandHandler::new(store);
        let aggregate_id = AggregateId::new();

        // Create
        handler
            .execute(aggregate_id, |_| {
                Ok(vec![TestEvent::Created {
                    name: "Test".to_string(),
                }])
            })
            .await
            .unwrap();

        // Update
        let result = handler
            .execute(aggregate_id, |_| Ok(vec![TestEvent::Updated { value: 42 }]))
            .await
            .unwrap();

        assert_eq!(result.events.len(), 1);
        assert_eq!(result.new_version, Version::new(2));
        assert_eq!(result.aggregate.value, 42);
    }

    #[tokio::test]
    async fn test_execute_returns_error_on_invalid_command() {
        let store = InMemoryEventStore::new();
        let handler: CommandHandler<_, TestAggregate> = CommandHandler::new(store);
        let aggregate_id = AggregateId::new();

        let result = handler
            .execute(aggregate_id, |_| Err(TestError::InvalidValue(-1)))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_existing_returns_none_for_new() {
        let store = InMemoryEventStore::new();
        let handler: CommandHandler<_, TestAggregate> = CommandHandler::new(store);
        let aggregate_id = AggregateId::new();

        let result = handler.load_existing(aggregate_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_load_existing_returns_some_for_existing() {
        let store = InMemoryEventStore::new();
        let handler: CommandHandler<_, TestAggregate> = CommandHandler::new(store);
        let aggregate_id = AggregateId::new();

        // Create aggregate
        handler
            .execute(aggregate_id, |_| {
                Ok(vec![TestEvent::Created {
                    name: "Test".to_string(),
                }])
            })
            .await
            .unwrap();

        let result = handler.load_existing(aggregate_id).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "Test");
    }

    #[tokio::test]
    async fn test_empty_events_returns_without_persisting() {
        let store = InMemoryEventStore::new();
        let handler: CommandHandler<_, TestAggregate> = CommandHandler::new(store.clone());
        let aggregate_id = AggregateId::new();

        let result = handler.execute(aggregate_id, |_| Ok(vec![])).await.unwrap();

        assert!(result.events.is_empty());
        assert_eq!(result.new_version, Version::initial());
        assert_eq!(store.event_count().await, 0);
    }
}
