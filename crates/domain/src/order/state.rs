//! Order state machine.

use serde::{Deserialize, Serialize};

/// The state of an order in its lifecycle.
///
/// State transitions:
/// ```text
/// Draft ──────┬──► Reserved ──► Processing ──► Completed
///             │        │            │
///             └────────┴────────────┴──► Cancelled
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum OrderState {
    /// Order is being created, items can be added/removed.
    #[default]
    Draft,

    /// Inventory has been reserved, awaiting payment.
    Reserved,

    /// Payment confirmed, order is being fulfilled.
    Processing,

    /// Order has been completed/shipped (terminal state).
    Completed,

    /// Order was cancelled (terminal state).
    Cancelled,
}

impl OrderState {
    /// Returns true if items can be modified in this state.
    pub fn can_modify_items(&self) -> bool {
        matches!(self, OrderState::Draft)
    }

    /// Returns true if the order can be submitted in this state.
    pub fn can_submit(&self) -> bool {
        matches!(self, OrderState::Draft)
    }

    /// Returns true if the order can be reserved in this state.
    pub fn can_reserve(&self) -> bool {
        matches!(self, OrderState::Draft)
    }

    /// Returns true if processing can start in this state.
    pub fn can_start_processing(&self) -> bool {
        matches!(self, OrderState::Reserved)
    }

    /// Returns true if the order can be completed in this state.
    pub fn can_complete(&self) -> bool {
        matches!(self, OrderState::Processing)
    }

    /// Returns true if the order can be cancelled in this state.
    pub fn can_cancel(&self) -> bool {
        matches!(
            self,
            OrderState::Draft | OrderState::Reserved | OrderState::Processing
        )
    }

    /// Returns true if this is a terminal state (no further transitions possible).
    pub fn is_terminal(&self) -> bool {
        matches!(self, OrderState::Completed | OrderState::Cancelled)
    }

    /// Returns the state name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderState::Draft => "Draft",
            OrderState::Reserved => "Reserved",
            OrderState::Processing => "Processing",
            OrderState::Completed => "Completed",
            OrderState::Cancelled => "Cancelled",
        }
    }
}

impl std::fmt::Display for OrderState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state_is_draft() {
        assert_eq!(OrderState::default(), OrderState::Draft);
    }

    #[test]
    fn test_draft_can_modify_items() {
        assert!(OrderState::Draft.can_modify_items());
        assert!(!OrderState::Reserved.can_modify_items());
        assert!(!OrderState::Processing.can_modify_items());
        assert!(!OrderState::Completed.can_modify_items());
        assert!(!OrderState::Cancelled.can_modify_items());
    }

    #[test]
    fn test_draft_can_submit() {
        assert!(OrderState::Draft.can_submit());
        assert!(!OrderState::Reserved.can_submit());
        assert!(!OrderState::Processing.can_submit());
        assert!(!OrderState::Completed.can_submit());
        assert!(!OrderState::Cancelled.can_submit());
    }

    #[test]
    fn test_reserved_can_start_processing() {
        assert!(!OrderState::Draft.can_start_processing());
        assert!(OrderState::Reserved.can_start_processing());
        assert!(!OrderState::Processing.can_start_processing());
        assert!(!OrderState::Completed.can_start_processing());
        assert!(!OrderState::Cancelled.can_start_processing());
    }

    #[test]
    fn test_processing_can_complete() {
        assert!(!OrderState::Draft.can_complete());
        assert!(!OrderState::Reserved.can_complete());
        assert!(OrderState::Processing.can_complete());
        assert!(!OrderState::Completed.can_complete());
        assert!(!OrderState::Cancelled.can_complete());
    }

    #[test]
    fn test_can_cancel_from_non_terminal_states() {
        assert!(OrderState::Draft.can_cancel());
        assert!(OrderState::Reserved.can_cancel());
        assert!(OrderState::Processing.can_cancel());
        assert!(!OrderState::Completed.can_cancel());
        assert!(!OrderState::Cancelled.can_cancel());
    }

    #[test]
    fn test_terminal_states() {
        assert!(!OrderState::Draft.is_terminal());
        assert!(!OrderState::Reserved.is_terminal());
        assert!(!OrderState::Processing.is_terminal());
        assert!(OrderState::Completed.is_terminal());
        assert!(OrderState::Cancelled.is_terminal());
    }

    #[test]
    fn test_display() {
        assert_eq!(OrderState::Draft.to_string(), "Draft");
        assert_eq!(OrderState::Reserved.to_string(), "Reserved");
        assert_eq!(OrderState::Processing.to_string(), "Processing");
        assert_eq!(OrderState::Completed.to_string(), "Completed");
        assert_eq!(OrderState::Cancelled.to_string(), "Cancelled");
    }

    #[test]
    fn test_serialization() {
        let state = OrderState::Processing;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: OrderState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }
}
