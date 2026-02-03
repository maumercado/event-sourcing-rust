use chrono::{DateTime, Utc};

use crate::{AggregateId, Version};

/// Builder for constructing event queries.
///
/// Allows filtering events by various criteria such as aggregate ID,
/// event type, version range, and time range.
#[derive(Debug, Clone, Default)]
pub struct EventQuery {
    /// Filter by aggregate ID.
    pub aggregate_id: Option<AggregateId>,

    /// Filter by aggregate type.
    pub aggregate_type: Option<String>,

    /// Filter by event types (any of these types).
    pub event_types: Option<Vec<String>>,

    /// Filter by minimum version (inclusive).
    pub from_version: Option<Version>,

    /// Filter by maximum version (inclusive).
    pub to_version: Option<Version>,

    /// Filter by events after this timestamp (inclusive).
    pub from_timestamp: Option<DateTime<Utc>>,

    /// Filter by events before this timestamp (inclusive).
    pub to_timestamp: Option<DateTime<Utc>>,

    /// Maximum number of events to return.
    pub limit: Option<usize>,

    /// Number of events to skip.
    pub offset: Option<usize>,
}

impl EventQuery {
    /// Creates a new empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a query for a specific aggregate.
    pub fn for_aggregate(aggregate_id: AggregateId) -> Self {
        Self {
            aggregate_id: Some(aggregate_id),
            ..Default::default()
        }
    }

    /// Creates a query for events of a specific type.
    pub fn for_event_type(event_type: impl Into<String>) -> Self {
        Self {
            event_types: Some(vec![event_type.into()]),
            ..Default::default()
        }
    }

    /// Filters by aggregate ID.
    pub fn aggregate_id(mut self, id: AggregateId) -> Self {
        self.aggregate_id = Some(id);
        self
    }

    /// Filters by aggregate type.
    pub fn aggregate_type(mut self, aggregate_type: impl Into<String>) -> Self {
        self.aggregate_type = Some(aggregate_type.into());
        self
    }

    /// Filters by event type.
    pub fn event_type(mut self, event_type: impl Into<String>) -> Self {
        self.event_types = Some(vec![event_type.into()]);
        self
    }

    /// Filters by multiple event types (any of these).
    pub fn event_types(mut self, event_types: Vec<String>) -> Self {
        self.event_types = Some(event_types);
        self
    }

    /// Filters to events starting from this version (inclusive).
    pub fn from_version(mut self, version: Version) -> Self {
        self.from_version = Some(version);
        self
    }

    /// Filters to events up to this version (inclusive).
    pub fn to_version(mut self, version: Version) -> Self {
        self.to_version = Some(version);
        self
    }

    /// Filters to events after this timestamp (inclusive).
    pub fn from_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.from_timestamp = Some(timestamp);
        self
    }

    /// Filters to events before this timestamp (inclusive).
    pub fn to_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.to_timestamp = Some(timestamp);
        self
    }

    /// Limits the number of events returned.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skips this many events before returning results.
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_for_aggregate() {
        let id = AggregateId::new();
        let query = EventQuery::for_aggregate(id);

        assert_eq!(query.aggregate_id, Some(id));
        assert!(query.event_types.is_none());
    }

    #[test]
    fn query_for_event_type() {
        let query = EventQuery::for_event_type("OrderCreated");

        assert!(query.aggregate_id.is_none());
        assert_eq!(query.event_types, Some(vec!["OrderCreated".to_string()]));
    }

    #[test]
    fn query_builder_chain() {
        let id = AggregateId::new();
        let query = EventQuery::new()
            .aggregate_id(id)
            .event_type("OrderCreated")
            .from_version(Version::new(1))
            .to_version(Version::new(10))
            .limit(100)
            .offset(0);

        assert_eq!(query.aggregate_id, Some(id));
        assert_eq!(query.event_types, Some(vec!["OrderCreated".to_string()]));
        assert_eq!(query.from_version, Some(Version::new(1)));
        assert_eq!(query.to_version, Some(Version::new(10)));
        assert_eq!(query.limit, Some(100));
        assert_eq!(query.offset, Some(0));
    }
}
