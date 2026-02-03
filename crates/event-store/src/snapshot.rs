use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AggregateId, Version};

/// A snapshot of an aggregate's state at a specific version.
///
/// Snapshots are used to optimize aggregate reconstruction by providing
/// a starting point, avoiding the need to replay all events from the beginning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// The aggregate this snapshot belongs to.
    pub aggregate_id: AggregateId,

    /// The type of aggregate (e.g., "Order", "Customer").
    pub aggregate_type: String,

    /// The version of the aggregate at the time of the snapshot.
    pub version: Version,

    /// When the snapshot was created.
    pub timestamp: DateTime<Utc>,

    /// The serialized aggregate state.
    pub state: serde_json::Value,
}

impl Snapshot {
    /// Creates a new snapshot.
    pub fn new(
        aggregate_id: AggregateId,
        aggregate_type: impl Into<String>,
        version: Version,
        state: serde_json::Value,
    ) -> Self {
        Self {
            aggregate_id,
            aggregate_type: aggregate_type.into(),
            version,
            timestamp: Utc::now(),
            state,
        }
    }

    /// Creates a snapshot from a serializable state.
    pub fn from_state<T: Serialize>(
        aggregate_id: AggregateId,
        aggregate_type: impl Into<String>,
        version: Version,
        state: &T,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            aggregate_id,
            aggregate_type: aggregate_type.into(),
            version,
            timestamp: Utc::now(),
            state: serde_json::to_value(state)?,
        })
    }

    /// Deserializes the snapshot state into a concrete type.
    pub fn into_state<T: for<'de> Deserialize<'de>>(self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.state)
    }

    /// Gets a reference to the state as JSON.
    pub fn state_ref(&self) -> &serde_json::Value {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
        name: String,
    }

    #[test]
    fn snapshot_new() {
        let id = AggregateId::new();
        let state = serde_json::json!({"value": 42});

        let snapshot = Snapshot::new(id, "TestAggregate", Version::new(5), state.clone());

        assert_eq!(snapshot.aggregate_id, id);
        assert_eq!(snapshot.aggregate_type, "TestAggregate");
        assert_eq!(snapshot.version, Version::new(5));
        assert_eq!(snapshot.state, state);
    }

    #[test]
    fn snapshot_from_state_and_into_state() {
        let id = AggregateId::new();
        let original = TestState {
            value: 42,
            name: "test".to_string(),
        };

        let snapshot =
            Snapshot::from_state(id, "TestAggregate", Version::new(5), &original).unwrap();

        let restored: TestState = snapshot.into_state().unwrap();
        assert_eq!(restored, original);
    }
}
