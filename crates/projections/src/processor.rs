//! Projection processor for feeding events to projections.

use event_store::{EventEnvelope, EventStore};
use futures_util::StreamExt;

use crate::Result;
use crate::projection::Projection;

/// Processes events from an event store and delivers them to projections.
///
/// The processor supports:
/// - Catch-up: replays all events from the store to bring projections up to date
/// - Single event delivery: delivers a new event to all projections
/// - Rebuild: resets all projections and replays from scratch
pub struct ProjectionProcessor<S: EventStore> {
    store: S,
    projections: Vec<Box<dyn Projection>>,
}

impl<S: EventStore> ProjectionProcessor<S> {
    /// Creates a new processor with the given event store.
    pub fn new(store: S) -> Self {
        Self {
            store,
            projections: Vec::new(),
        }
    }

    /// Registers a projection with this processor.
    pub fn register(&mut self, projection: Box<dyn Projection>) {
        self.projections.push(projection);
    }

    /// Returns the number of registered projections.
    pub fn projection_count(&self) -> usize {
        self.projections.len()
    }

    /// Runs catch-up processing: streams all events from the store and delivers
    /// them to each projection that hasn't already seen them.
    #[tracing::instrument(skip(self))]
    pub async fn run_catch_up(&self) -> Result<()> {
        let mut stream = self.store.stream_all_events().await?;
        let mut event_index: u64 = 0;

        while let Some(result) = stream.next().await {
            let event = result?;
            event_index += 1;

            for projection in &self.projections {
                let pos = projection.position().await;
                if pos.events_processed < event_index {
                    projection.handle(&event).await?;
                    metrics::counter!("projections_events_processed").increment(1);
                }
            }
        }

        tracing::info!(events_processed = event_index, "catch-up complete");

        Ok(())
    }

    /// Delivers a single event to all registered projections.
    #[tracing::instrument(skip(self, event), fields(event_type = %event.event_type))]
    pub async fn process_event(&self, event: &EventEnvelope) -> Result<()> {
        for projection in &self.projections {
            projection.handle(event).await?;
        }
        Ok(())
    }

    /// Resets all projections and replays all events from the store.
    #[tracing::instrument(skip(self))]
    pub async fn rebuild_all(&self) -> Result<()> {
        for projection in &self.projections {
            projection.reset().await?;
        }
        self.run_catch_up().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::ProjectionPosition;
    use async_trait::async_trait;
    use common::AggregateId;
    use event_store::{InMemoryEventStore, Version};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// A simple counting projection for testing.
    struct CountingProjection {
        count: Arc<RwLock<u64>>,
        position: Arc<RwLock<ProjectionPosition>>,
    }

    impl CountingProjection {
        fn new() -> Self {
            Self {
                count: Arc::new(RwLock::new(0)),
                position: Arc::new(RwLock::new(ProjectionPosition::zero())),
            }
        }
    }

    #[async_trait]
    impl Projection for CountingProjection {
        fn name(&self) -> &'static str {
            "CountingProjection"
        }

        async fn handle(&self, _event: &EventEnvelope) -> Result<()> {
            let mut count = self.count.write().await;
            *count += 1;
            let mut pos = self.position.write().await;
            *pos = pos.advance();
            Ok(())
        }

        async fn position(&self) -> ProjectionPosition {
            *self.position.read().await
        }

        async fn reset(&self) -> Result<()> {
            *self.count.write().await = 0;
            *self.position.write().await = ProjectionPosition::zero();
            Ok(())
        }
    }

    fn create_test_event(aggregate_id: AggregateId, version: Version) -> EventEnvelope {
        EventEnvelope::builder()
            .aggregate_id(aggregate_id)
            .aggregate_type("Order")
            .event_type("TestEvent")
            .version(version)
            .payload_raw(serde_json::json!({"test": true}))
            .build()
    }

    #[tokio::test]
    async fn test_catch_up_processes_all_events() {
        let store = InMemoryEventStore::new();
        let agg_id = AggregateId::new();

        // Add some events to the store
        let events = vec![
            create_test_event(agg_id, Version::new(1)),
            create_test_event(agg_id, Version::new(2)),
            create_test_event(agg_id, Version::new(3)),
        ];
        store
            .append(events, event_store::AppendOptions::new())
            .await
            .unwrap();

        let counting = Arc::new(CountingProjection::new());
        let mut processor = ProjectionProcessor::new(store);
        processor.register(Box::new(CountingProjection::new()));

        // Use a shared projection to verify
        let shared = Arc::clone(&counting);
        processor.projections.clear();

        // We need to register the Arc-wrapped projection differently
        // Instead, let's test via the processor directly
        let store2 = InMemoryEventStore::new();
        let events2 = vec![
            create_test_event(agg_id, Version::new(1)),
            create_test_event(agg_id, Version::new(2)),
            create_test_event(agg_id, Version::new(3)),
        ];
        store2
            .append(events2, event_store::AppendOptions::new())
            .await
            .unwrap();

        let projection = CountingProjection::new();
        let count_ref = Arc::clone(&projection.count);
        let mut processor = ProjectionProcessor::new(store2);
        processor.projections.push(Box::new(projection));

        processor.run_catch_up().await.unwrap();

        assert_eq!(*count_ref.read().await, 3);
        drop(shared);
        drop(counting);
    }

    #[tokio::test]
    async fn test_process_single_event() {
        let store = InMemoryEventStore::new();
        let projection = CountingProjection::new();
        let count_ref = Arc::clone(&projection.count);

        let mut processor = ProjectionProcessor::new(store);
        processor.projections.push(Box::new(projection));

        let event = create_test_event(AggregateId::new(), Version::new(1));
        processor.process_event(&event).await.unwrap();

        assert_eq!(*count_ref.read().await, 1);
    }

    #[tokio::test]
    async fn test_rebuild_resets_and_replays() {
        let store = InMemoryEventStore::new();
        let agg_id = AggregateId::new();

        let events = vec![
            create_test_event(agg_id, Version::new(1)),
            create_test_event(agg_id, Version::new(2)),
        ];
        store
            .append(events, event_store::AppendOptions::new())
            .await
            .unwrap();

        let projection = CountingProjection::new();
        let count_ref = Arc::clone(&projection.count);
        let pos_ref = Arc::clone(&projection.position);

        let mut processor = ProjectionProcessor::new(store);
        processor.projections.push(Box::new(projection));

        // First catch-up
        processor.run_catch_up().await.unwrap();
        assert_eq!(*count_ref.read().await, 2);

        // Rebuild should reset and replay
        processor.rebuild_all().await.unwrap();
        assert_eq!(*count_ref.read().await, 2);
        assert_eq!(pos_ref.read().await.events_processed, 2);
    }

    #[tokio::test]
    async fn test_catch_up_skips_already_processed() {
        let store = InMemoryEventStore::new();
        let agg_id = AggregateId::new();

        let events = vec![
            create_test_event(agg_id, Version::new(1)),
            create_test_event(agg_id, Version::new(2)),
            create_test_event(agg_id, Version::new(3)),
        ];
        store
            .append(events, event_store::AppendOptions::new())
            .await
            .unwrap();

        let projection = CountingProjection::new();
        let count_ref = Arc::clone(&projection.count);

        let mut processor = ProjectionProcessor::new(store);
        processor.projections.push(Box::new(projection));

        // First catch-up
        processor.run_catch_up().await.unwrap();
        assert_eq!(*count_ref.read().await, 3);

        // Second catch-up should not re-process
        processor.run_catch_up().await.unwrap();
        assert_eq!(*count_ref.read().await, 3);
    }

    #[tokio::test]
    async fn test_empty_store_catch_up() {
        let store = InMemoryEventStore::new();
        let projection = CountingProjection::new();
        let count_ref = Arc::clone(&projection.count);

        let mut processor = ProjectionProcessor::new(store);
        processor.projections.push(Box::new(projection));

        processor.run_catch_up().await.unwrap();
        assert_eq!(*count_ref.read().await, 0);
    }

    #[tokio::test]
    async fn test_multiple_projections() {
        let store = InMemoryEventStore::new();
        let agg_id = AggregateId::new();

        let events = vec![
            create_test_event(agg_id, Version::new(1)),
            create_test_event(agg_id, Version::new(2)),
        ];
        store
            .append(events, event_store::AppendOptions::new())
            .await
            .unwrap();

        let proj1 = CountingProjection::new();
        let proj2 = CountingProjection::new();
        let count1 = Arc::clone(&proj1.count);
        let count2 = Arc::clone(&proj2.count);

        let mut processor = ProjectionProcessor::new(store);
        processor.projections.push(Box::new(proj1));
        processor.projections.push(Box::new(proj2));

        processor.run_catch_up().await.unwrap();

        assert_eq!(*count1.read().await, 2);
        assert_eq!(*count2.read().await, 2);
    }
}
