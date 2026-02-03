pub mod error;
pub mod event;
pub mod query;
pub mod snapshot;

pub use common::AggregateId;
pub use error::{EventStoreError, Result};
pub use event::{EventEnvelope, EventEnvelopeBuilder, EventId, Version};
pub use query::EventQuery;
pub use snapshot::Snapshot;
