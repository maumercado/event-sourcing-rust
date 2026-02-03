use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;

use crate::{AggregateId, EventEnvelope, EventQuery, Result, Snapshot, Version};

/// Options for appending events to the store.
#[derive(Debug, Clone, Default)]
pub struct AppendOptions {
    /// Expected version of the aggregate for optimistic concurrency control.
    /// If None, no version check is performed (use with caution).
    pub expected_version: Option<Version>,
}

impl AppendOptions {
    /// Creates options with no version check.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates options expecting the aggregate to be at a specific version.
    pub fn expect_version(version: Version) -> Self {
        Self {
            expected_version: Some(version),
        }
    }

    /// Creates options expecting the aggregate to not exist (new aggregate).
    pub fn expect_new() -> Self {
        Self {
            expected_version: Some(Version::initial()),
        }
    }
}

/// A stream of events.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<EventEnvelope>> + Send>>;

/// Core trait for event store implementations.
///
/// An event store is responsible for persisting and retrieving events.
/// All implementations must be thread-safe (Send + Sync).
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Appends events to the store.
    ///
    /// Events are appended atomically - either all succeed or none do.
    /// If `options.expected_version` is set, the operation will fail with
    /// `ConcurrencyConflict` if the current version doesn't match.
    ///
    /// Returns the new version of the aggregate after appending.
    async fn append(&self, events: Vec<EventEnvelope>, options: AppendOptions) -> Result<Version>;

    /// Retrieves all events for a specific aggregate.
    ///
    /// Events are returned in version order (oldest first).
    async fn get_events_for_aggregate(
        &self,
        aggregate_id: AggregateId,
    ) -> Result<Vec<EventEnvelope>>;

    /// Retrieves all events for an aggregate starting from a specific version.
    ///
    /// Useful when replaying from a snapshot.
    async fn get_events_for_aggregate_from_version(
        &self,
        aggregate_id: AggregateId,
        from_version: Version,
    ) -> Result<Vec<EventEnvelope>>;

    /// Retrieves events matching a query.
    async fn query_events(&self, query: EventQuery) -> Result<Vec<EventEnvelope>>;

    /// Retrieves events by type.
    async fn get_events_by_type(&self, event_type: &str) -> Result<Vec<EventEnvelope>>;

    /// Streams all events in the store.
    ///
    /// Events are returned in insertion order.
    async fn stream_all_events(&self) -> Result<EventStream>;

    /// Gets the current version of an aggregate.
    ///
    /// Returns None if the aggregate doesn't exist.
    async fn get_aggregate_version(&self, aggregate_id: AggregateId) -> Result<Option<Version>>;

    /// Saves a snapshot of an aggregate's state.
    ///
    /// If a snapshot already exists for this aggregate, it is replaced.
    async fn save_snapshot(&self, snapshot: Snapshot) -> Result<()>;

    /// Retrieves the latest snapshot for an aggregate.
    ///
    /// Returns None if no snapshot exists.
    async fn get_snapshot(&self, aggregate_id: AggregateId) -> Result<Option<Snapshot>>;
}

/// Extension trait providing convenience methods for event stores.
#[async_trait]
pub trait EventStoreExt: EventStore {
    /// Appends a single event to the store.
    async fn append_event(&self, event: EventEnvelope, options: AppendOptions) -> Result<Version> {
        self.append(vec![event], options).await
    }

    /// Checks if an aggregate exists (has any events).
    async fn aggregate_exists(&self, aggregate_id: AggregateId) -> Result<bool> {
        Ok(self.get_aggregate_version(aggregate_id).await?.is_some())
    }

    /// Loads an aggregate's events, optionally starting from a snapshot.
    ///
    /// If a snapshot exists, returns the snapshot and events after it.
    /// Otherwise, returns None and all events.
    async fn load_aggregate(
        &self,
        aggregate_id: AggregateId,
    ) -> Result<(Option<Snapshot>, Vec<EventEnvelope>)> {
        if let Some(snapshot) = self.get_snapshot(aggregate_id).await? {
            let events = self
                .get_events_for_aggregate_from_version(aggregate_id, snapshot.version.next())
                .await?;
            Ok((Some(snapshot), events))
        } else {
            let events = self.get_events_for_aggregate(aggregate_id).await?;
            Ok((None, events))
        }
    }
}

// Blanket implementation for all EventStore implementations
impl<T: EventStore + ?Sized> EventStoreExt for T {}

/// Error returned when building an invalid event for appending.
#[derive(Debug, Clone)]
pub struct AppendValidationError {
    pub message: String,
}

impl std::fmt::Display for AppendValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Append validation error: {}", self.message)
    }
}

impl std::error::Error for AppendValidationError {}

/// Validates events before appending.
pub fn validate_events_for_append(
    events: &[EventEnvelope],
) -> std::result::Result<(), AppendValidationError> {
    if events.is_empty() {
        return Err(AppendValidationError {
            message: "Cannot append empty event list".to_string(),
        });
    }

    // All events must be for the same aggregate
    let first = &events[0];
    for event in events.iter().skip(1) {
        if event.aggregate_id != first.aggregate_id {
            return Err(AppendValidationError {
                message: "All events must be for the same aggregate".to_string(),
            });
        }
        if event.aggregate_type != first.aggregate_type {
            return Err(AppendValidationError {
                message: "All events must have the same aggregate type".to_string(),
            });
        }
    }

    // Versions must be sequential
    let mut expected_version = first.version;
    for event in events.iter().skip(1) {
        expected_version = expected_version.next();
        if event.version != expected_version {
            return Err(AppendValidationError {
                message: format!(
                    "Event versions must be sequential. Expected {}, got {}",
                    expected_version, event.version
                ),
            });
        }
    }

    Ok(())
}
