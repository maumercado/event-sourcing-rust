use common::AggregateId;
use criterion::{Criterion, criterion_group, criterion_main};
use domain::{
    AddItem, Aggregate, CreateOrder, CustomerId, Money, Order, OrderEvent, OrderItem, OrderService,
    SubmitOrder,
};
use event_store::{AppendOptions, EventEnvelope, InMemoryEventStore, Version, store::EventStore};

fn make_envelope(aggregate_id: AggregateId, version: i64, event: &OrderEvent) -> EventEnvelope {
    EventEnvelope::builder()
        .aggregate_id(aggregate_id)
        .aggregate_type("Order")
        .event_type(domain::DomainEvent::event_type(event))
        .version(Version::new(version))
        .payload(event)
        .unwrap()
        .build()
}

fn bench_create_order(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("domain/create_order", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = InMemoryEventStore::new();
                let service = OrderService::new(store);
                let cmd = CreateOrder::for_customer(CustomerId::new());
                service.create_order(cmd).await.unwrap();
            });
        });
    });
}

fn bench_add_item(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();
    let service = OrderService::new(store);
    let cmd = CreateOrder::for_customer(CustomerId::new());
    let order_id = cmd.order_id;
    rt.block_on(async { service.create_order(cmd).await.unwrap() });

    c.bench_function("domain/add_item", |b| {
        b.iter(|| {
            rt.block_on(async {
                let item =
                    OrderItem::new("SKU-BENCH", "Benchmark Widget", 1, Money::from_cents(1000));
                service
                    .add_item(AddItem::new(order_id, item))
                    .await
                    .unwrap();
            });
        });
    });
}

fn bench_full_command_cycle(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("domain/full_create_add_submit", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = InMemoryEventStore::new();
                let service = OrderService::new(store);
                let cmd = CreateOrder::for_customer(CustomerId::new());
                let order_id = cmd.order_id;
                service.create_order(cmd).await.unwrap();

                let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(1000));
                service
                    .add_item(AddItem::new(order_id, item))
                    .await
                    .unwrap();

                service
                    .submit_order(SubmitOrder::new(order_id))
                    .await
                    .unwrap();
            });
        });
    });
}

fn bench_aggregate_reconstruction(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();
    let agg_id = AggregateId::new();
    let customer_id = CustomerId::new();

    // Pre-populate: 1 create + 50 add-item events
    rt.block_on(async {
        let created = OrderEvent::order_created(agg_id, customer_id);
        let mut events = vec![make_envelope(agg_id, 1, &created)];
        for v in 2..=51 {
            let item = OrderItem::new(
                format!("SKU-{v:03}").as_str(),
                format!("Product {v}").as_str(),
                1,
                Money::from_cents(100 * v),
            );
            let added = OrderEvent::item_added(&item);
            events.push(make_envelope(agg_id, v, &added));
        }
        store.append(events, AppendOptions::new()).await.unwrap();
    });

    c.bench_function("domain/reconstruct_50_events", |b| {
        b.iter(|| {
            rt.block_on(async {
                let events = store.get_events_for_aggregate(agg_id).await.unwrap();
                let mut order = Order::default();
                for event in &events {
                    let domain_event: OrderEvent =
                        serde_json::from_value(event.payload.clone()).unwrap();
                    order.apply(domain_event);
                }
            });
        });
    });
}

fn bench_aggregate_reconstruction_100(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryEventStore::new();
    let agg_id = AggregateId::new();
    let customer_id = CustomerId::new();

    rt.block_on(async {
        let created = OrderEvent::order_created(agg_id, customer_id);
        let mut events = vec![make_envelope(agg_id, 1, &created)];
        for v in 2..=100 {
            let item = OrderItem::new(
                format!("SKU-{v:03}").as_str(),
                format!("Product {v}").as_str(),
                1,
                Money::from_cents(100 * v),
            );
            let added = OrderEvent::item_added(&item);
            events.push(make_envelope(agg_id, v, &added));
        }
        store.append(events, AppendOptions::new()).await.unwrap();
    });

    c.bench_function("domain/reconstruct_100_events", |b| {
        b.iter(|| {
            rt.block_on(async {
                let events = store.get_events_for_aggregate(agg_id).await.unwrap();
                let mut order = Order::default();
                for event in &events {
                    let domain_event: OrderEvent =
                        serde_json::from_value(event.payload.clone()).unwrap();
                    order.apply(domain_event);
                }
            });
        });
    });
}

criterion_group!(
    benches,
    bench_create_order,
    bench_add_item,
    bench_full_command_cycle,
    bench_aggregate_reconstruction,
    bench_aggregate_reconstruction_100,
);
criterion_main!(benches);
