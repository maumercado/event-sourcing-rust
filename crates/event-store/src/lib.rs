pub mod error;
pub mod event;
pub mod postgres;
pub mod query;
pub mod snapshot;
pub mod store;

pub use common::AggregateId;
pub use error::{EventStoreError, Result};
pub use event::{EventEnvelope, EventEnvelopeBuilder, EventId, Version};
pub use postgres::PostgresEventStore;
pub use query::EventQuery;
pub use snapshot::Snapshot;
pub use store::{AppendOptions, EventStore, EventStoreExt, EventStream};
