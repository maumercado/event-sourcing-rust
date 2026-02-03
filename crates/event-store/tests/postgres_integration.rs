//! PostgreSQL integration tests
//!
//! These tests use a shared PostgreSQL container for efficiency.
//! Run with:
//!
//! ```bash
//! cargo test -p event-store --test postgres_integration -- --test-threads=1
//! ```

use std::sync::Arc;

use event_store::{
    AggregateId, AppendOptions, EventEnvelope, EventQuery, EventStore, EventStoreExt,
    PostgresEventStore, Snapshot, Version,
};
use sqlx::PgPool;
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::OnceCell;

/// Shared container info - container stays alive for all tests
struct ContainerInfo {
    #[allow(dead_code)] // Container must stay alive for tests
    container: ContainerAsync<Postgres>,
    connection_string: String,
}

/// Global shared container
static CONTAINER: OnceCell<Arc<ContainerInfo>> = OnceCell::const_new();

async fn get_container_info() -> Arc<ContainerInfo> {
    CONTAINER
        .get_or_init(|| async {
            let container = Postgres::default().start().await.unwrap();

            let host = container.get_host().await.unwrap();
            let port = container.get_host_port_ipv4(5432).await.unwrap();

            let connection_string =
                format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

            // Create a temporary pool just for migrations
            let temp_pool = PgPool::connect(&connection_string).await.unwrap();

            // Run migrations using raw_sql to execute multiple statements
            sqlx::raw_sql(include_str!(
                "../../../migrations/001_create_events_table.sql"
            ))
            .execute(&temp_pool)
            .await
            .unwrap();

            temp_pool.close().await;

            Arc::new(ContainerInfo {
                container,
                connection_string,
            })
        })
        .await
        .clone()
}

/// Get a fresh store with its own pool and cleared tables
async fn get_test_store() -> PostgresEventStore {
    let info = get_container_info().await;

    // Create a fresh pool for each test to avoid connection issues
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&info.connection_string)
        .await
        .unwrap();

    // Clear tables for test isolation
    sqlx::query("TRUNCATE TABLE events, snapshots")
        .execute(&pool)
        .await
        .unwrap();

    PostgresEventStore::new(pool)
}

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
async fn append_and_retrieve_events() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    let event = create_test_event(aggregate_id, Version::first(), "TestEvent");
    let result = store.append(vec![event], AppendOptions::expect_new()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Version::first());

    let events = store.get_events_for_aggregate(aggregate_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "TestEvent");
    assert_eq!(events[0].version, Version::first());
}

#[tokio::test]
async fn append_multiple_events_atomically() {
    let store = get_test_store().await;
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
    assert_eq!(stored[0].version, Version::new(1));
    assert_eq!(stored[1].version, Version::new(2));
    assert_eq!(stored[2].version, Version::new(3));
}

#[tokio::test]
async fn optimistic_concurrency_conflict() {
    let store = get_test_store().await;
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

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        event_store::EventStoreError::ConcurrencyConflict { .. }
    ));
}

#[tokio::test]
async fn optimistic_concurrency_success() {
    let store = get_test_store().await;
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

    let version = store.get_aggregate_version(aggregate_id).await.unwrap();
    assert_eq!(version, Some(Version::new(2)));
}

#[tokio::test]
async fn get_events_from_version() {
    let store = get_test_store().await;
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
    let store = get_test_store().await;
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
async fn query_events_with_filters() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    let events = vec![
        create_test_event(aggregate_id, Version::new(1), "Event1"),
        create_test_event(aggregate_id, Version::new(2), "Event2"),
        create_test_event(aggregate_id, Version::new(3), "Event3"),
    ];
    store.append(events, AppendOptions::new()).await.unwrap();

    // Query with version range
    let query = EventQuery::new()
        .aggregate_id(aggregate_id)
        .from_version(Version::new(2))
        .to_version(Version::new(2));

    let results = store.query_events(query).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].version, Version::new(2));
}

#[tokio::test]
async fn query_events_with_limit_and_offset() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    let events = vec![
        create_test_event(aggregate_id, Version::new(1), "Event1"),
        create_test_event(aggregate_id, Version::new(2), "Event2"),
        create_test_event(aggregate_id, Version::new(3), "Event3"),
        create_test_event(aggregate_id, Version::new(4), "Event4"),
        create_test_event(aggregate_id, Version::new(5), "Event5"),
    ];
    store.append(events, AppendOptions::new()).await.unwrap();

    let query = EventQuery::new()
        .aggregate_id(aggregate_id)
        .limit(2)
        .offset(1);

    let results = store.query_events(query).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].version, Version::new(2));
    assert_eq!(results[1].version, Version::new(3));
}

#[tokio::test]
async fn snapshot_save_and_retrieve() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    let snapshot = Snapshot::new(
        aggregate_id,
        "TestAggregate",
        Version::new(5),
        serde_json::json!({"state": "saved"}),
    );

    store.save_snapshot(snapshot).await.unwrap();

    let retrieved = store.get_snapshot(aggregate_id).await.unwrap();
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.aggregate_id, aggregate_id);
    assert_eq!(retrieved.version, Version::new(5));
    assert_eq!(retrieved.state, serde_json::json!({"state": "saved"}));
}

#[tokio::test]
async fn snapshot_update_replaces_existing() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    let snapshot1 = Snapshot::new(
        aggregate_id,
        "TestAggregate",
        Version::new(5),
        serde_json::json!({"state": "first"}),
    );
    store.save_snapshot(snapshot1).await.unwrap();

    let snapshot2 = Snapshot::new(
        aggregate_id,
        "TestAggregate",
        Version::new(10),
        serde_json::json!({"state": "second"}),
    );
    store.save_snapshot(snapshot2).await.unwrap();

    let retrieved = store.get_snapshot(aggregate_id).await.unwrap().unwrap();
    assert_eq!(retrieved.version, Version::new(10));
    assert_eq!(retrieved.state, serde_json::json!({"state": "second"}));
}

#[tokio::test]
async fn snapshot_not_found() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    let result = store.get_snapshot(aggregate_id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn stream_all_events() {
    use futures_util::StreamExt;

    let store = get_test_store().await;
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
    assert!(events.iter().all(|e| e.is_ok()));
}

#[tokio::test]
async fn aggregate_exists_extension() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    // Doesn't exist yet
    assert!(!store.aggregate_exists(aggregate_id).await.unwrap());

    // Add an event
    let event = create_test_event(aggregate_id, Version::first(), "Event1");
    store
        .append(vec![event], AppendOptions::new())
        .await
        .unwrap();

    // Now exists
    assert!(store.aggregate_exists(aggregate_id).await.unwrap());
}

#[tokio::test]
async fn load_aggregate_without_snapshot() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    let events = vec![
        create_test_event(aggregate_id, Version::new(1), "Event1"),
        create_test_event(aggregate_id, Version::new(2), "Event2"),
    ];
    store.append(events, AppendOptions::new()).await.unwrap();

    let (snapshot, events) = store.load_aggregate(aggregate_id).await.unwrap();
    assert!(snapshot.is_none());
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn load_aggregate_with_snapshot() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    // Add initial events
    let events = vec![
        create_test_event(aggregate_id, Version::new(1), "Event1"),
        create_test_event(aggregate_id, Version::new(2), "Event2"),
        create_test_event(aggregate_id, Version::new(3), "Event3"),
    ];
    store.append(events, AppendOptions::new()).await.unwrap();

    // Save snapshot at version 2
    let snapshot = Snapshot::new(
        aggregate_id,
        "TestAggregate",
        Version::new(2),
        serde_json::json!({"state": "at_v2"}),
    );
    store.save_snapshot(snapshot).await.unwrap();

    // Add more events
    let more_events = vec![
        create_test_event(aggregate_id, Version::new(4), "Event4"),
        create_test_event(aggregate_id, Version::new(5), "Event5"),
    ];
    store
        .append(more_events, AppendOptions::new())
        .await
        .unwrap();

    // Load should return snapshot and events after it
    let (snapshot, events) = store.load_aggregate(aggregate_id).await.unwrap();
    assert!(snapshot.is_some());
    assert_eq!(snapshot.unwrap().version, Version::new(2));
    // Events from version 3 onwards
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].version, Version::new(3));
}

#[tokio::test]
async fn unique_constraint_prevents_duplicate_versions() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    // First event at version 1
    let event1 = create_test_event(aggregate_id, Version::first(), "Event1");
    store
        .append(vec![event1], AppendOptions::new())
        .await
        .unwrap();

    // Try to insert another event at version 1 (should fail)
    let event2 = create_test_event(aggregate_id, Version::first(), "Event2");
    let result = store.append(vec![event2], AppendOptions::new()).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn event_metadata_preserved() {
    let store = get_test_store().await;
    let aggregate_id = AggregateId::new();

    let event = EventEnvelope::builder()
        .aggregate_id(aggregate_id)
        .aggregate_type("TestAggregate")
        .event_type("TestEvent")
        .version(Version::first())
        .payload_raw(serde_json::json!({"data": "test"}))
        .metadata("correlation_id", serde_json::json!("corr-123"))
        .metadata("causation_id", serde_json::json!("cause-456"))
        .build();

    store
        .append(vec![event], AppendOptions::new())
        .await
        .unwrap();

    let events = store.get_events_for_aggregate(aggregate_id).await.unwrap();
    assert_eq!(events.len(), 1);

    let retrieved = &events[0];
    assert_eq!(
        retrieved.metadata.get("correlation_id"),
        Some(&serde_json::json!("corr-123"))
    );
    assert_eq!(
        retrieved.metadata.get("causation_id"),
        Some(&serde_json::json!("cause-456"))
    );
}
