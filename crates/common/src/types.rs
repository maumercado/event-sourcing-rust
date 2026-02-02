use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an aggregate instance.
///
/// Wraps a UUID to provide type safety and prevent mixing up
/// aggregate IDs with other UUID-based identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AggregateId(Uuid);

impl AggregateId {
    /// Creates a new random aggregate ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates an aggregate ID from an existing UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for AggregateId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AggregateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for AggregateId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<AggregateId> for Uuid {
    fn from(id: AggregateId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_id_new_creates_unique_ids() {
        let id1 = AggregateId::new();
        let id2 = AggregateId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn aggregate_id_from_uuid_preserves_value() {
        let uuid = Uuid::new_v4();
        let id = AggregateId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), uuid);
    }

    #[test]
    fn aggregate_id_serialization_roundtrip() {
        let id = AggregateId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: AggregateId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }
}
