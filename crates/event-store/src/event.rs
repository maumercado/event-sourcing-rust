use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AggregateId;

/// Unique identifier for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(Uuid);

impl EventId {
    /// Creates a new random event ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates an event ID from an existing UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for EventId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<EventId> for Uuid {
    fn from(id: EventId) -> Self {
        id.0
    }
}

/// Version number for an aggregate, used for optimistic concurrency control.
///
/// Versions start at 1 for the first event and increment by 1 for each
/// subsequent event on an aggregate.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Version(i64);

impl Version {
    /// Creates a new version from a raw value.
    pub fn new(value: i64) -> Self {
        Self(value)
    }

    /// Returns the initial version (0) for a new aggregate.
    pub fn initial() -> Self {
        Self(0)
    }

    /// Returns the first version (1) for the first event.
    pub fn first() -> Self {
        Self(1)
    }

    /// Returns the next version.
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }

    /// Returns the raw version value.
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<i64> for Version {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<Version> for i64 {
    fn from(version: Version) -> Self {
        version.0
    }
}

/// An event envelope containing an event along with its metadata.
///
/// This structure wraps a domain event with all the information needed
/// for storage and retrieval in the event store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Unique identifier for this event.
    pub event_id: EventId,

    /// The type of the event (e.g., "OrderCreated", "OrderItemAdded").
    pub event_type: String,

    /// The aggregate this event belongs to.
    pub aggregate_id: AggregateId,

    /// The type of aggregate (e.g., "Order", "Customer").
    pub aggregate_type: String,

    /// The version of the aggregate after this event.
    pub version: Version,

    /// When the event was created.
    pub timestamp: DateTime<Utc>,

    /// The event payload as JSON.
    pub payload: serde_json::Value,

    /// Additional metadata about the event.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl EventEnvelope {
    /// Creates a new event envelope builder.
    pub fn builder() -> EventEnvelopeBuilder {
        EventEnvelopeBuilder::default()
    }
}

/// Builder for constructing event envelopes.
#[derive(Debug, Default)]
pub struct EventEnvelopeBuilder {
    event_id: Option<EventId>,
    event_type: Option<String>,
    aggregate_id: Option<AggregateId>,
    aggregate_type: Option<String>,
    version: Option<Version>,
    timestamp: Option<DateTime<Utc>>,
    payload: Option<serde_json::Value>,
    metadata: HashMap<String, serde_json::Value>,
}

impl EventEnvelopeBuilder {
    /// Sets the event ID. If not set, a new ID will be generated.
    pub fn event_id(mut self, id: EventId) -> Self {
        self.event_id = Some(id);
        self
    }

    /// Sets the event type.
    pub fn event_type(mut self, event_type: impl Into<String>) -> Self {
        self.event_type = Some(event_type.into());
        self
    }

    /// Sets the aggregate ID.
    pub fn aggregate_id(mut self, id: AggregateId) -> Self {
        self.aggregate_id = Some(id);
        self
    }

    /// Sets the aggregate type.
    pub fn aggregate_type(mut self, aggregate_type: impl Into<String>) -> Self {
        self.aggregate_type = Some(aggregate_type.into());
        self
    }

    /// Sets the version.
    pub fn version(mut self, version: Version) -> Self {
        self.version = Some(version);
        self
    }

    /// Sets the timestamp. If not set, the current time will be used.
    pub fn timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Sets the payload from a serializable value.
    pub fn payload<T: Serialize>(mut self, payload: &T) -> Result<Self, serde_json::Error> {
        self.payload = Some(serde_json::to_value(payload)?);
        Ok(self)
    }

    /// Sets the payload from a raw JSON value.
    pub fn payload_raw(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Adds a metadata entry.
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Builds the event envelope.
    ///
    /// # Panics
    ///
    /// Panics if required fields (event_type, aggregate_id, aggregate_type, version, payload)
    /// are not set.
    pub fn build(self) -> EventEnvelope {
        EventEnvelope {
            event_id: self.event_id.unwrap_or_default(),
            event_type: self.event_type.expect("event_type is required"),
            aggregate_id: self.aggregate_id.expect("aggregate_id is required"),
            aggregate_type: self.aggregate_type.expect("aggregate_type is required"),
            version: self.version.expect("version is required"),
            timestamp: self.timestamp.unwrap_or_else(Utc::now),
            payload: self.payload.expect("payload is required"),
            metadata: self.metadata,
        }
    }

    /// Tries to build the event envelope, returning None if required fields are missing.
    pub fn try_build(self) -> Option<EventEnvelope> {
        Some(EventEnvelope {
            event_id: self.event_id.unwrap_or_default(),
            event_type: self.event_type?,
            aggregate_id: self.aggregate_id?,
            aggregate_type: self.aggregate_type?,
            version: self.version?,
            timestamp: self.timestamp.unwrap_or_else(Utc::now),
            payload: self.payload?,
            metadata: self.metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_id_new_creates_unique_ids() {
        let id1 = EventId::new();
        let id2 = EventId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn version_ordering() {
        let v1 = Version::new(1);
        let v2 = Version::new(2);
        assert!(v1 < v2);
        assert_eq!(v1.next(), v2);
    }

    #[test]
    fn version_initial_and_first() {
        assert_eq!(Version::initial().as_i64(), 0);
        assert_eq!(Version::first().as_i64(), 1);
        assert_eq!(Version::initial().next(), Version::first());
    }

    #[test]
    fn event_envelope_builder() {
        let aggregate_id = AggregateId::new();
        let payload = serde_json::json!({"item": "test"});

        let envelope = EventEnvelope::builder()
            .event_type("TestEvent")
            .aggregate_id(aggregate_id)
            .aggregate_type("TestAggregate")
            .version(Version::first())
            .payload_raw(payload.clone())
            .metadata("correlation_id", serde_json::json!("123"))
            .build();

        assert_eq!(envelope.event_type, "TestEvent");
        assert_eq!(envelope.aggregate_id, aggregate_id);
        assert_eq!(envelope.aggregate_type, "TestAggregate");
        assert_eq!(envelope.version, Version::first());
        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.metadata.get("correlation_id"),
            Some(&serde_json::json!("123"))
        );
    }

    #[test]
    fn event_envelope_try_build_returns_none_on_missing_fields() {
        let result = EventEnvelope::builder().try_build();
        assert!(result.is_none());
    }
}
