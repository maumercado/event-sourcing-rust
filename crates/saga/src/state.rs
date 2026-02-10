//! Saga state machine.

use serde::{Deserialize, Serialize};

/// The state of a saga in its lifecycle.
///
/// State transitions:
/// ```text
/// NotStarted ──► Running ──┬──► Completed
///                          └──► Compensating ──► Failed
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SagaState {
    /// Saga has not started yet.
    #[default]
    NotStarted,

    /// Saga steps are being executed.
    Running,

    /// A step failed and compensating transactions are in progress.
    Compensating,

    /// All steps completed successfully (terminal state).
    Completed,

    /// Compensation finished after a failure (terminal state).
    Failed,
}

impl SagaState {
    /// Returns true if the saga can begin running.
    pub fn can_run(&self) -> bool {
        matches!(self, SagaState::NotStarted)
    }

    /// Returns true if the saga can begin compensation.
    pub fn can_compensate(&self) -> bool {
        matches!(self, SagaState::Running)
    }

    /// Returns true if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, SagaState::Completed | SagaState::Failed)
    }

    /// Returns the state name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            SagaState::NotStarted => "NotStarted",
            SagaState::Running => "Running",
            SagaState::Compensating => "Compensating",
            SagaState::Completed => "Completed",
            SagaState::Failed => "Failed",
        }
    }
}

impl std::fmt::Display for SagaState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state_is_not_started() {
        assert_eq!(SagaState::default(), SagaState::NotStarted);
    }

    #[test]
    fn test_can_run() {
        assert!(SagaState::NotStarted.can_run());
        assert!(!SagaState::Running.can_run());
        assert!(!SagaState::Compensating.can_run());
        assert!(!SagaState::Completed.can_run());
        assert!(!SagaState::Failed.can_run());
    }

    #[test]
    fn test_can_compensate() {
        assert!(!SagaState::NotStarted.can_compensate());
        assert!(SagaState::Running.can_compensate());
        assert!(!SagaState::Compensating.can_compensate());
        assert!(!SagaState::Completed.can_compensate());
        assert!(!SagaState::Failed.can_compensate());
    }

    #[test]
    fn test_terminal_states() {
        assert!(!SagaState::NotStarted.is_terminal());
        assert!(!SagaState::Running.is_terminal());
        assert!(!SagaState::Compensating.is_terminal());
        assert!(SagaState::Completed.is_terminal());
        assert!(SagaState::Failed.is_terminal());
    }

    #[test]
    fn test_display() {
        assert_eq!(SagaState::NotStarted.to_string(), "NotStarted");
        assert_eq!(SagaState::Running.to_string(), "Running");
        assert_eq!(SagaState::Compensating.to_string(), "Compensating");
        assert_eq!(SagaState::Completed.to_string(), "Completed");
        assert_eq!(SagaState::Failed.to_string(), "Failed");
    }

    #[test]
    fn test_serialization() {
        let state = SagaState::Running;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SagaState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }
}
