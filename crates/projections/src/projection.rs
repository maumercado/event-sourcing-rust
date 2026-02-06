//! Core projection trait and position tracking.

use async_trait::async_trait;
use event_store::EventEnvelope;

use crate::Result;

/// Tracks how many events a projection has processed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProjectionPosition {
    /// Number of events processed by this projection.
    pub events_processed: u64,
}

impl ProjectionPosition {
    /// Creates a new position at zero.
    pub fn zero() -> Self {
        Self {
            events_processed: 0,
        }
    }

    /// Advances the position by one event.
    pub fn advance(&self) -> Self {
        Self {
            events_processed: self.events_processed + 1,
        }
    }
}

impl std::fmt::Display for ProjectionPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "position({})", self.events_processed)
    }
}

/// A projection that processes events and updates a read model.
///
/// Projections are the mechanism by which events are transformed into
/// denormalized read models optimized for queries.
#[async_trait]
pub trait Projection: Send + Sync {
    /// Returns the name of this projection.
    fn name(&self) -> &'static str;

    /// Handles a single event, updating the projection's read model.
    async fn handle(&self, event: &EventEnvelope) -> Result<()>;

    /// Returns the current position of this projection.
    async fn position(&self) -> ProjectionPosition;

    /// Resets the projection to its initial state.
    async fn reset(&self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_starts_at_zero() {
        let pos = ProjectionPosition::zero();
        assert_eq!(pos.events_processed, 0);
    }

    #[test]
    fn position_advances() {
        let pos = ProjectionPosition::zero();
        let pos = pos.advance();
        assert_eq!(pos.events_processed, 1);
        let pos = pos.advance();
        assert_eq!(pos.events_processed, 2);
    }

    #[test]
    fn position_display() {
        let pos = ProjectionPosition {
            events_processed: 42,
        };
        assert_eq!(pos.to_string(), "position(42)");
    }
}
