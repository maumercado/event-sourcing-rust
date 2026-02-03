use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{
    AggregateId, EventEnvelope, EventQuery, EventStoreError, Result, Snapshot, Version,
    store::{AppendOptions, EventStore, EventStream, validate_events_for_append},
};

/// In-memory event store implementation for testing.
///
/// This implementation stores all events in memory and provides
/// the same interface as the PostgreSQL implementation.
#[derive(Clone, Default)]
pub struct InMemoryEventStore {
    events: Arc<RwLock<Vec<EventEnvelope>>>,
    snapshots: Arc<RwLock<HashMap<AggregateId, Snapshot>>>,
}

impl InMemoryEventStore {
    /// Creates a new empty in-memory event store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the total number of events stored.
    pub async fn event_count(&self) -> usize {
        self.events.read().await.len()
    }

    /// Clears all events and snapshots.
    pub async fn clear(&self) {
        self.events.write().await.clear();
        self.snapshots.write().await.clear();
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn append(&self, events: Vec<EventEnvelope>, options: AppendOptions) -> Result<Version> {
        validate_events_for_append(&events).map_err(|e| {
            EventStoreError::Serialization(serde_json::Error::io(std::io::Error::other(e.message)))
        })?;

        let first_event = &events[0];
        let aggregate_id = first_event.aggregate_id;

        let mut store = self.events.write().await;

        // Get current version for this aggregate
        let current_version = store
            .iter()
            .filter(|e| e.aggregate_id == aggregate_id)
            .map(|e| e.version)
            .max()
            .unwrap_or(Version::initial());

        // Check expected version if specified
        if let Some(expected) = options.expected_version
            && current_version != expected
        {
            return Err(EventStoreError::ConcurrencyConflict {
                aggregate_id,
                expected,
                actual: current_version,
            });
        }

        // Check for version conflicts (unique constraint simulation)
        let first_new_version = first_event.version;
        if first_new_version <= current_version && current_version != Version::initial() {
            return Err(EventStoreError::ConcurrencyConflict {
                aggregate_id,
                expected: options.expected_version.unwrap_or(current_version),
                actual: current_version,
            });
        }

        // Store all events
        let last_version = events
            .last()
            .map(|e| e.version)
            .unwrap_or(Version::initial());
        store.extend(events);

        Ok(last_version)
    }

    async fn get_events_for_aggregate(
        &self,
        aggregate_id: AggregateId,
    ) -> Result<Vec<EventEnvelope>> {
        let store = self.events.read().await;
        let mut events: Vec<_> = store
            .iter()
            .filter(|e| e.aggregate_id == aggregate_id)
            .cloned()
            .collect();
        events.sort_by_key(|e| e.version);
        Ok(events)
    }

    async fn get_events_for_aggregate_from_version(
        &self,
        aggregate_id: AggregateId,
        from_version: Version,
    ) -> Result<Vec<EventEnvelope>> {
        let store = self.events.read().await;
        let mut events: Vec<_> = store
            .iter()
            .filter(|e| e.aggregate_id == aggregate_id && e.version >= from_version)
            .cloned()
            .collect();
        events.sort_by_key(|e| e.version);
        Ok(events)
    }

    async fn query_events(&self, query: EventQuery) -> Result<Vec<EventEnvelope>> {
        let store = self.events.read().await;
        let mut events: Vec<_> = store
            .iter()
            .filter(|e| {
                if let Some(id) = query.aggregate_id
                    && e.aggregate_id != id
                {
                    return false;
                }
                if let Some(ref agg_type) = query.aggregate_type
                    && &e.aggregate_type != agg_type
                {
                    return false;
                }
                if let Some(ref types) = query.event_types
                    && !types.contains(&e.event_type)
                {
                    return false;
                }
                if let Some(from) = query.from_version
                    && e.version < from
                {
                    return false;
                }
                if let Some(to) = query.to_version
                    && e.version > to
                {
                    return false;
                }
                if let Some(from) = query.from_timestamp
                    && e.timestamp < from
                {
                    return false;
                }
                if let Some(to) = query.to_timestamp
                    && e.timestamp > to
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        // Sort by timestamp then version
        events.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.version.cmp(&b.version))
        });

        // Apply offset and limit
        let offset = query.offset.unwrap_or(0);
        let events: Vec<_> = events.into_iter().skip(offset).collect();

        let events = if let Some(limit) = query.limit {
            events.into_iter().take(limit).collect()
        } else {
            events
        };

        Ok(events)
    }

    async fn get_events_by_type(&self, event_type: &str) -> Result<Vec<EventEnvelope>> {
        let store = self.events.read().await;
        let mut events: Vec<_> = store
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect();
        events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        Ok(events)
    }

    async fn stream_all_events(&self) -> Result<EventStream> {
        use futures_util::stream;

        let store = self.events.read().await;
        let mut events = store.clone();
        events.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.event_id.as_uuid().cmp(&b.event_id.as_uuid()))
        });

        let stream = stream::iter(events.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }

    async fn get_aggregate_version(&self, aggregate_id: AggregateId) -> Result<Option<Version>> {
        let store = self.events.read().await;
        let version = store
            .iter()
            .filter(|e| e.aggregate_id == aggregate_id)
            .map(|e| e.version)
            .max();
        Ok(version)
    }

    async fn save_snapshot(&self, snapshot: Snapshot) -> Result<()> {
        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(snapshot.aggregate_id, snapshot);
        Ok(())
    }

    async fn get_snapshot(&self, aggregate_id: AggregateId) -> Result<Option<Snapshot>> {
        let snapshots = self.snapshots.read().await;
        Ok(snapshots.get(&aggregate_id).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(
        aggregate_id: AggregateId,
        version: Version,
        event_type: &str,
    ) -> EventEnvelope {
        EventEnvelope::builder()
            .aggregate_id(aggregate_id)
            .aggregate_type("TestAggregate")
            .event_type(event_type)
            .version(version)
            .payload_raw(serde_json::json!({"test": true}))
            .build()
    }

    #[tokio::test]
    async fn append_single_event() {
        let store = InMemoryEventStore::new();
        let aggregate_id = AggregateId::new();
        let event = create_test_event(aggregate_id, Version::first(), "TestEvent");

        let result = store.append(vec![event], AppendOptions::expect_new()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Version::first());

        let events = store.get_events_for_aggregate(aggregate_id).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn append_multiple_events() {
        let store = InMemoryEventStore::new();
        let aggregate_id = AggregateId::new();

        let events = vec![
            create_test_event(aggregate_id, Version::new(1), "Event1"),
            create_test_event(aggregate_id, Version::new(2), "Event2"),
            create_test_event(aggregate_id, Version::new(3), "Event3"),
        ];

        let result = store.append(events, AppendOptions::expect_new()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Version::new(3));

        let stored = store.get_events_for_aggregate(aggregate_id).await.unwrap();
        assert_eq!(stored.len(), 3);
    }

    #[tokio::test]
    async fn concurrency_conflict_on_wrong_version() {
        let store = InMemoryEventStore::new();
        let aggregate_id = AggregateId::new();

        // First event
        let event1 = create_test_event(aggregate_id, Version::first(), "Event1");
        store
            .append(vec![event1], AppendOptions::expect_new())
            .await
            .unwrap();

        // Try to append with wrong expected version
        let event2 = create_test_event(aggregate_id, Version::new(2), "Event2");
        let result = store
            .append(
                vec![event2],
                AppendOptions::expect_version(Version::initial()),
            )
            .await;

        assert!(matches!(
            result,
            Err(EventStoreError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn concurrency_conflict_success() {
        let store = InMemoryEventStore::new();
        let aggregate_id = AggregateId::new();

        // First event
        let event1 = create_test_event(aggregate_id, Version::first(), "Event1");
        store
            .append(vec![event1], AppendOptions::expect_new())
            .await
            .unwrap();

        // Append with correct expected version
        let event2 = create_test_event(aggregate_id, Version::new(2), "Event2");
        let result = store
            .append(
                vec![event2],
                AppendOptions::expect_version(Version::first()),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn get_events_from_version() {
        let store = InMemoryEventStore::new();
        let aggregate_id = AggregateId::new();

        let events = vec![
            create_test_event(aggregate_id, Version::new(1), "Event1"),
            create_test_event(aggregate_id, Version::new(2), "Event2"),
            create_test_event(aggregate_id, Version::new(3), "Event3"),
        ];
        store.append(events, AppendOptions::new()).await.unwrap();

        let from_v2 = store
            .get_events_for_aggregate_from_version(aggregate_id, Version::new(2))
            .await
            .unwrap();
        assert_eq!(from_v2.len(), 2);
        assert_eq!(from_v2[0].version, Version::new(2));
        assert_eq!(from_v2[1].version, Version::new(3));
    }

    #[tokio::test]
    async fn get_events_by_type() {
        let store = InMemoryEventStore::new();
        let id1 = AggregateId::new();
        let id2 = AggregateId::new();

        store
            .append(
                vec![create_test_event(id1, Version::first(), "OrderCreated")],
                AppendOptions::new(),
            )
            .await
            .unwrap();
        store
            .append(
                vec![create_test_event(id2, Version::first(), "OrderShipped")],
                AppendOptions::new(),
            )
            .await
            .unwrap();
        store
            .append(
                vec![create_test_event(id1, Version::new(2), "OrderCreated")],
                AppendOptions::new(),
            )
            .await
            .unwrap();

        let created = store.get_events_by_type("OrderCreated").await.unwrap();
        assert_eq!(created.len(), 2);

        let shipped = store.get_events_by_type("OrderShipped").await.unwrap();
        assert_eq!(shipped.len(), 1);
    }

    #[tokio::test]
    async fn snapshot_save_and_retrieve() {
        let store = InMemoryEventStore::new();
        let aggregate_id = AggregateId::new();

        let snapshot = Snapshot::new(
            aggregate_id,
            "TestAggregate",
            Version::new(5),
            serde_json::json!({"state": "saved"}),
        );

        store.save_snapshot(snapshot.clone()).await.unwrap();

        let retrieved = store.get_snapshot(aggregate_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.aggregate_id, aggregate_id);
        assert_eq!(retrieved.version, Version::new(5));
    }

    #[tokio::test]
    async fn snapshot_not_found() {
        let store = InMemoryEventStore::new();
        let aggregate_id = AggregateId::new();

        let result = store.get_snapshot(aggregate_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn query_events_with_filters() {
        let store = InMemoryEventStore::new();
        let id1 = AggregateId::new();

        let events = vec![
            create_test_event(id1, Version::new(1), "Event1"),
            create_test_event(id1, Version::new(2), "Event2"),
            create_test_event(id1, Version::new(3), "Event3"),
        ];
        store.append(events, AppendOptions::new()).await.unwrap();

        // Query with version range
        let query = EventQuery::new()
            .aggregate_id(id1)
            .from_version(Version::new(2))
            .to_version(Version::new(2));

        let results = store.query_events(query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].version, Version::new(2));
    }

    #[tokio::test]
    async fn stream_all_events() {
        use futures_util::StreamExt;

        let store = InMemoryEventStore::new();
        let id1 = AggregateId::new();
        let id2 = AggregateId::new();

        store
            .append(
                vec![create_test_event(id1, Version::first(), "Event1")],
                AppendOptions::new(),
            )
            .await
            .unwrap();
        store
            .append(
                vec![create_test_event(id2, Version::first(), "Event2")],
                AppendOptions::new(),
            )
            .await
            .unwrap();

        let stream = store.stream_all_events().await.unwrap();
        let events: Vec<_> = stream.collect().await;
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn get_aggregate_version() {
        let store = InMemoryEventStore::new();
        let aggregate_id = AggregateId::new();

        // No events yet
        let version = store.get_aggregate_version(aggregate_id).await.unwrap();
        assert!(version.is_none());

        // Add some events
        let events = vec![
            create_test_event(aggregate_id, Version::new(1), "Event1"),
            create_test_event(aggregate_id, Version::new(2), "Event2"),
        ];
        store.append(events, AppendOptions::new()).await.unwrap();

        let version = store.get_aggregate_version(aggregate_id).await.unwrap();
        assert_eq!(version, Some(Version::new(2)));
    }
}
