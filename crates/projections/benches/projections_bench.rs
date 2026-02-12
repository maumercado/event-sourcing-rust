use common::AggregateId;
use criterion::{Criterion, criterion_group, criterion_main};
use domain::{CustomerId, DomainEvent, Money, OrderEvent, OrderItem};
use event_store::{AppendOptions, EventEnvelope, InMemoryEventStore, Version, store::EventStore};
use projections::{CurrentOrdersView, Projection, ProjectionProcessor};

use std::sync::Arc;

fn make_envelope(aggregate_id: AggregateId, version: i64, event: &OrderEvent) -> EventEnvelope {
    EventEnvelope::builder()
        .aggregate_id(aggregate_id)
        .aggregate_type("Order")
        .event_type(DomainEvent::event_type(event))
        .version(Version::new(version))
        .payload(event)
        .unwrap()
        .build()
}

/// Populate a store with N orders, each having 3 events (created + item_added + submitted).
async fn populate_store(store: &InMemoryEventStore, n: usize) {
    for _ in 0..n {
        let agg_id = AggregateId::new();
        let customer_id = CustomerId::new();
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));

        let created = OrderEvent::order_created(agg_id, customer_id);
        let added = OrderEvent::item_added(&item);
        let submitted = OrderEvent::order_submitted(Money::from_cents(2000), 1);

        let events = vec![
            make_envelope(agg_id, 1, &created),
            make_envelope(agg_id, 2, &added),
            make_envelope(agg_id, 3, &submitted),
        ];
        store.append(events, AppendOptions::new()).await.unwrap();
    }
}

fn bench_catch_up_100_orders(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();

    rt.block_on(populate_store(&store, 100));

    c.bench_function("projections/catch_up_300_events", |b| {
        b.iter(|| {
            rt.block_on(async {
                let view = CurrentOrdersView::new();
                let mut processor = ProjectionProcessor::new(store.clone());
                processor.register(Box::new(view.clone()) as Box<dyn Projection>);
                processor.run_catch_up().await.unwrap();
            });
        });
    });
}

fn bench_catch_up_1000_orders(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();

    rt.block_on(populate_store(&store, 1000));

    c.bench_function("projections/catch_up_3000_events", |b| {
        b.iter(|| {
            rt.block_on(async {
                let view = CurrentOrdersView::new();
                let mut processor = ProjectionProcessor::new(store.clone());
                processor.register(Box::new(view.clone()) as Box<dyn Projection>);
                processor.run_catch_up().await.unwrap();
            });
        });
    });
}

fn bench_process_single_event(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let view = Arc::new(CurrentOrdersView::new());

    c.bench_function("projections/process_single_event", |b| {
        b.iter(|| {
            rt.block_on(async {
                let agg_id = AggregateId::new();
                let customer_id = CustomerId::new();
                let event = OrderEvent::order_created(agg_id, customer_id);
                let envelope = make_envelope(agg_id, 1, &event);
                view.handle(&envelope).await.unwrap();
            });
        });
    });
}

fn bench_query_all_orders(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();
    let view = Arc::new(CurrentOrdersView::new());

    // Pre-populate with 100 orders
    rt.block_on(async {
        populate_store(&store, 100).await;
        let mut processor = ProjectionProcessor::new(store);
        processor.register(Box::new(view.as_ref().clone()) as Box<dyn Projection>);
        processor.run_catch_up().await.unwrap();
    });

    c.bench_function("projections/query_all_100_orders", |b| {
        b.iter(|| {
            rt.block_on(async {
                view.get_all_orders().await;
            });
        });
    });
}

fn bench_query_orders_by_customer(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();
    let view = Arc::new(CurrentOrdersView::new());
    let target_customer = CustomerId::new();

    // Pre-populate: 5 orders for target customer, 95 for others
    rt.block_on(async {
        for _ in 0..5 {
            let agg_id = AggregateId::new();
            let created = OrderEvent::order_created(agg_id, target_customer);
            let item = OrderItem::new("SKU-001", "Widget", 1, Money::from_cents(500));
            let added = OrderEvent::item_added(&item);
            let events = vec![
                make_envelope(agg_id, 1, &created),
                make_envelope(agg_id, 2, &added),
            ];
            store.append(events, AppendOptions::new()).await.unwrap();
        }
        populate_store(&store, 95).await;

        let mut processor = ProjectionProcessor::new(store);
        processor.register(Box::new(view.as_ref().clone()) as Box<dyn Projection>);
        processor.run_catch_up().await.unwrap();
    });

    c.bench_function("projections/query_by_customer", |b| {
        b.iter(|| {
            rt.block_on(async {
                view.get_orders_by_customer(target_customer).await;
            });
        });
    });
}

fn bench_rebuild_100_orders(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();
    let view = Arc::new(CurrentOrdersView::new());

    rt.block_on(async {
        populate_store(&store, 100).await;
    });

    let mut processor = ProjectionProcessor::new(store);
    processor.register(Box::new(view.as_ref().clone()) as Box<dyn Projection>);
    let processor = Arc::new(processor);

    c.bench_function("projections/rebuild_300_events", |b| {
        b.iter(|| {
            rt.block_on(async {
                processor.rebuild_all().await.unwrap();
            });
        });
    });
}

criterion_group!(
    benches,
    bench_catch_up_100_orders,
    bench_catch_up_1000_orders,
    bench_process_single_event,
    bench_query_all_orders,
    bench_query_orders_by_customer,
    bench_rebuild_100_orders,
);
criterion_main!(benches);
