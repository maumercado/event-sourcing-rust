//! Core aggregate and domain event traits.

use common::AggregateId;
use event_store::Version;
use serde::{Serialize, de::DeserializeOwned};

/// Trait for domain events.
///
/// Domain events represent facts that have happened in the domain.
/// They are immutable and should be named in past tense.
pub trait DomainEvent: Serialize + DeserializeOwned + Send + Sync + Clone {
    /// Returns the event type name.
    ///
    /// This is used for serialization and event store filtering.
    fn event_type(&self) -> &'static str;
}

/// Trait for aggregates in an event-sourced system.
///
/// An aggregate is a cluster of domain objects that can be treated as a single unit.
/// The aggregate root ensures consistency of changes being made within the aggregate.
///
/// In event sourcing, aggregates:
/// - Are rebuilt by replaying events
/// - Generate events from commands
/// - Apply events to update state (pure, deterministic)
pub trait Aggregate: Default + Send + Sync + Sized {
    /// The type of events this aggregate produces and consumes.
    type Event: DomainEvent;

    /// The type of errors this aggregate can produce.
    type Error: std::error::Error + Send + Sync;

    /// Returns the aggregate type name.
    ///
    /// Used for event store organization and routing.
    fn aggregate_type() -> &'static str;

    /// Returns the aggregate's unique identifier.
    ///
    /// Returns None for a new, uninitialized aggregate.
    fn id(&self) -> Option<AggregateId>;

    /// Returns the current version of the aggregate.
    ///
    /// Version starts at 0 for a new aggregate and increments with each event.
    fn version(&self) -> Version;

    /// Sets the aggregate version.
    ///
    /// Called by the command handler after loading events.
    fn set_version(&mut self, version: Version);

    /// Applies an event to the aggregate, updating its state.
    ///
    /// This method must be pure and deterministic:
    /// - Given the same state and event, it must always produce the same new state
    /// - It must not have side effects
    /// - It must not fail (events represent facts that have happened)
    fn apply(&mut self, event: Self::Event);

    /// Applies multiple events in sequence.
    fn apply_events(&mut self, events: impl IntoIterator<Item = Self::Event>) {
        for event in events {
            self.apply(event);
        }
    }
}

/// Trait for aggregates that support snapshotting.
///
/// Snapshotting is an optimization to avoid replaying all events when loading
/// an aggregate. The aggregate state is periodically serialized and stored.
pub trait SnapshotCapable: Aggregate + Serialize + DeserializeOwned {
    /// Returns the snapshot interval (number of events between snapshots).
    ///
    /// A value of 100 means a snapshot is taken every 100 events.
    fn snapshot_interval() -> usize {
        100
    }

    /// Returns whether a snapshot should be taken given the current version.
    fn should_snapshot(&self) -> bool {
        self.version().as_i64() > 0
            && (self.version().as_i64() as usize).is_multiple_of(Self::snapshot_interval())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestEvent {
        Created { id: String },
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
        value: i32,
        version: Version,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("test error")]
    struct TestError;

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
                TestEvent::Created { .. } => {
                    if self.id.is_none() {
                        self.id = Some(AggregateId::new());
                    }
                }
                TestEvent::Updated { value } => {
                    self.value = value;
                }
            }
        }
    }

    impl SnapshotCapable for TestAggregate {}

    #[test]
    fn test_aggregate_apply_events() {
        let mut aggregate = TestAggregate::default();
        let events = vec![
            TestEvent::Created {
                id: "test".to_string(),
            },
            TestEvent::Updated { value: 42 },
        ];

        aggregate.apply_events(events);

        assert!(aggregate.id().is_some());
        assert_eq!(aggregate.value, 42);
    }

    #[test]
    fn test_domain_event_type() {
        let event = TestEvent::Created {
            id: "test".to_string(),
        };
        assert_eq!(event.event_type(), "TestCreated");

        let event = TestEvent::Updated { value: 42 };
        assert_eq!(event.event_type(), "TestUpdated");
    }

    #[test]
    fn test_snapshot_interval() {
        let mut aggregate = TestAggregate::default();
        assert!(!aggregate.should_snapshot());

        aggregate.set_version(Version::new(100));
        assert!(aggregate.should_snapshot());

        aggregate.set_version(Version::new(101));
        assert!(!aggregate.should_snapshot());
    }
}
