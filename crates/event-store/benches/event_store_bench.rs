use common::AggregateId;
use criterion::{Criterion, criterion_group, criterion_main};
use event_store::{
    AppendOptions, EventEnvelope, EventStoreExt, InMemoryEventStore, Version, store::EventStore,
};

fn make_event(aggregate_id: AggregateId, version: i64) -> EventEnvelope {
    EventEnvelope::builder()
        .aggregate_id(aggregate_id)
        .aggregate_type("Order")
        .event_type("OrderCreated")
        .version(Version::new(version))
        .payload_raw(serde_json::json!({
            "type": "OrderCreated",
            "data": {
                "order_id": aggregate_id.to_string(),
                "customer_id": "00000000-0000-0000-0000-000000000001"
            }
        }))
        .build()
}

fn bench_append_single_event(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("event_store/append_single_event", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = InMemoryEventStore::new();
                let agg_id = AggregateId::new();
                let event = make_event(agg_id, 1);
                store
                    .append(vec![event], AppendOptions::new())
                    .await
                    .unwrap();
            });
        });
    });
}

fn bench_append_batch_10(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("event_store/append_batch_10", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = InMemoryEventStore::new();
                let agg_id = AggregateId::new();
                let events: Vec<EventEnvelope> = (1..=10).map(|v| make_event(agg_id, v)).collect();
                store.append(events, AppendOptions::new()).await.unwrap();
            });
        });
    });
}

fn bench_append_with_version_check(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("event_store/append_with_version_check", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = InMemoryEventStore::new();
                let agg_id = AggregateId::new();
                let event = make_event(agg_id, 1);
                store
                    .append(vec![event], AppendOptions::expect_new())
                    .await
                    .unwrap();
            });
        });
    });
}

fn bench_get_events_for_aggregate(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();
    let agg_id = AggregateId::new();

    // Pre-populate with 100 events
    rt.block_on(async {
        let events: Vec<EventEnvelope> = (1..=100).map(|v| make_event(agg_id, v)).collect();
        store.append(events, AppendOptions::new()).await.unwrap();
    });

    c.bench_function("event_store/get_events_100", |b| {
        b.iter(|| {
            rt.block_on(async {
                store.get_events_for_aggregate(agg_id).await.unwrap();
            });
        });
    });
}

fn bench_get_events_from_version(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();
    let agg_id = AggregateId::new();

    // Pre-populate with 100 events
    rt.block_on(async {
        let events: Vec<EventEnvelope> = (1..=100).map(|v| make_event(agg_id, v)).collect();
        store.append(events, AppendOptions::new()).await.unwrap();
    });

    c.bench_function("event_store/get_events_from_version_50", |b| {
        b.iter(|| {
            rt.block_on(async {
                store
                    .get_events_for_aggregate_from_version(agg_id, Version::new(50))
                    .await
                    .unwrap();
            });
        });
    });
}

fn bench_stream_all_events(c: &mut Criterion) {
    use futures_util::StreamExt;

    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();

    // Pre-populate with 1000 events across 10 aggregates
    rt.block_on(async {
        for _ in 0..10 {
            let agg_id = AggregateId::new();
            let events: Vec<EventEnvelope> = (1..=100).map(|v| make_event(agg_id, v)).collect();
            store.append(events, AppendOptions::new()).await.unwrap();
        }
    });

    c.bench_function("event_store/stream_1000_events", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut stream = store.stream_all_events().await.unwrap();
                let mut count = 0;
                while let Some(result) = stream.next().await {
                    result.unwrap();
                    count += 1;
                }
                assert_eq!(count, 1000);
            });
        });
    });
}

fn bench_append_event_ext(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("event_store/append_single_via_ext", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = InMemoryEventStore::new();
                let agg_id = AggregateId::new();
                let event = make_event(agg_id, 1);
                store
                    .append_event(event, AppendOptions::new())
                    .await
                    .unwrap();
            });
        });
    });
}

criterion_group!(
    benches,
    bench_append_single_event,
    bench_append_batch_10,
    bench_append_with_version_check,
    bench_get_events_for_aggregate,
    bench_get_events_from_version,
    bench_stream_all_events,
    bench_append_event_ext,
);
criterion_main!(benches);
