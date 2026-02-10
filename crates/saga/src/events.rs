//! Saga domain events.

use chrono::{DateTime, Utc};
use common::AggregateId;
use domain::DomainEvent;
use serde::{Deserialize, Serialize};

/// Events that can occur during saga execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SagaEvent {
    /// Saga execution started.
    SagaStarted(SagaStartedData),

    /// A saga step started execution.
    StepStarted(StepData),

    /// A saga step completed successfully.
    StepCompleted(StepCompletedData),

    /// A saga step failed.
    StepFailed(StepFailedData),

    /// Compensation started after a step failure.
    CompensationStarted(CompensationData),

    /// A compensation step completed successfully.
    CompensationStepCompleted(StepData),

    /// A compensation step failed (logged, compensation continues).
    CompensationStepFailed(StepFailedData),

    /// Saga completed successfully.
    SagaCompleted(SagaCompletedData),

    /// Saga failed after compensation.
    SagaFailed(SagaFailedData),
}

impl DomainEvent for SagaEvent {
    fn event_type(&self) -> &'static str {
        match self {
            SagaEvent::SagaStarted(_) => "SagaStarted",
            SagaEvent::StepStarted(_) => "StepStarted",
            SagaEvent::StepCompleted(_) => "StepCompleted",
            SagaEvent::StepFailed(_) => "StepFailed",
            SagaEvent::CompensationStarted(_) => "CompensationStarted",
            SagaEvent::CompensationStepCompleted(_) => "CompensationStepCompleted",
            SagaEvent::CompensationStepFailed(_) => "CompensationStepFailed",
            SagaEvent::SagaCompleted(_) => "SagaCompleted",
            SagaEvent::SagaFailed(_) => "SagaFailed",
        }
    }
}

/// Data for SagaStarted event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStartedData {
    /// The saga instance ID.
    pub saga_id: AggregateId,
    /// The order being fulfilled.
    pub order_id: AggregateId,
    /// The type of saga (e.g., "OrderFulfillment").
    pub saga_type: String,
    /// When the saga started.
    pub started_at: DateTime<Utc>,
}

/// Data for step started/completed events (just the step name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepData {
    /// The step name.
    pub step_name: String,
}

/// Data for StepCompleted event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepCompletedData {
    /// The step name.
    pub step_name: String,
    /// Reservation ID (set after reserve_inventory step).
    pub reservation_id: Option<String>,
    /// Payment ID (set after process_payment step).
    pub payment_id: Option<String>,
    /// Tracking number (set after create_shipment step).
    pub tracking_number: Option<String>,
}

/// Data for StepFailed event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepFailedData {
    /// The step that failed.
    pub step_name: String,
    /// Error message describing the failure.
    pub error: String,
}

/// Data for CompensationStarted event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationData {
    /// The step that triggered compensation.
    pub from_step: String,
}

/// Data for SagaCompleted event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaCompletedData {
    /// When the saga completed.
    pub completed_at: DateTime<Utc>,
}

/// Data for SagaFailed event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaFailedData {
    /// Reason for failure.
    pub reason: String,
    /// When the saga failed.
    pub failed_at: DateTime<Utc>,
}

// Convenience constructors
impl SagaEvent {
    /// Creates a SagaStarted event.
    pub fn saga_started(
        saga_id: AggregateId,
        order_id: AggregateId,
        saga_type: impl Into<String>,
    ) -> Self {
        SagaEvent::SagaStarted(SagaStartedData {
            saga_id,
            order_id,
            saga_type: saga_type.into(),
            started_at: Utc::now(),
        })
    }

    /// Creates a StepStarted event.
    pub fn step_started(step_name: impl Into<String>) -> Self {
        SagaEvent::StepStarted(StepData {
            step_name: step_name.into(),
        })
    }

    /// Creates a StepCompleted event.
    pub fn step_completed(
        step_name: impl Into<String>,
        reservation_id: Option<String>,
        payment_id: Option<String>,
        tracking_number: Option<String>,
    ) -> Self {
        SagaEvent::StepCompleted(StepCompletedData {
            step_name: step_name.into(),
            reservation_id,
            payment_id,
            tracking_number,
        })
    }

    /// Creates a StepFailed event.
    pub fn step_failed(step_name: impl Into<String>, error: impl Into<String>) -> Self {
        SagaEvent::StepFailed(StepFailedData {
            step_name: step_name.into(),
            error: error.into(),
        })
    }

    /// Creates a CompensationStarted event.
    pub fn compensation_started(from_step: impl Into<String>) -> Self {
        SagaEvent::CompensationStarted(CompensationData {
            from_step: from_step.into(),
        })
    }

    /// Creates a CompensationStepCompleted event.
    pub fn compensation_step_completed(step_name: impl Into<String>) -> Self {
        SagaEvent::CompensationStepCompleted(StepData {
            step_name: step_name.into(),
        })
    }

    /// Creates a CompensationStepFailed event.
    pub fn compensation_step_failed(
        step_name: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        SagaEvent::CompensationStepFailed(StepFailedData {
            step_name: step_name.into(),
            error: error.into(),
        })
    }

    /// Creates a SagaCompleted event.
    pub fn saga_completed() -> Self {
        SagaEvent::SagaCompleted(SagaCompletedData {
            completed_at: Utc::now(),
        })
    }

    /// Creates a SagaFailed event.
    pub fn saga_failed(reason: impl Into<String>) -> Self {
        SagaEvent::SagaFailed(SagaFailedData {
            reason: reason.into(),
            failed_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        let saga_id = AggregateId::new();
        let order_id = AggregateId::new();

        assert_eq!(
            SagaEvent::saga_started(saga_id, order_id, "OrderFulfillment").event_type(),
            "SagaStarted"
        );
        assert_eq!(
            SagaEvent::step_started("reserve_inventory").event_type(),
            "StepStarted"
        );
        assert_eq!(
            SagaEvent::step_completed("reserve_inventory", Some("RES-1".into()), None, None)
                .event_type(),
            "StepCompleted"
        );
        assert_eq!(
            SagaEvent::step_failed("reserve_inventory", "out of stock").event_type(),
            "StepFailed"
        );
        assert_eq!(
            SagaEvent::compensation_started("reserve_inventory").event_type(),
            "CompensationStarted"
        );
        assert_eq!(
            SagaEvent::compensation_step_completed("reserve_inventory").event_type(),
            "CompensationStepCompleted"
        );
        assert_eq!(
            SagaEvent::compensation_step_failed("reserve_inventory", "service down").event_type(),
            "CompensationStepFailed"
        );
        assert_eq!(SagaEvent::saga_completed().event_type(), "SagaCompleted");
        assert_eq!(
            SagaEvent::saga_failed("step failed").event_type(),
            "SagaFailed"
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        let saga_id = AggregateId::new();
        let order_id = AggregateId::new();

        let events = vec![
            SagaEvent::saga_started(saga_id, order_id, "OrderFulfillment"),
            SagaEvent::step_started("reserve_inventory"),
            SagaEvent::step_completed("reserve_inventory", Some("RES-1".into()), None, None),
            SagaEvent::step_failed("process_payment", "insufficient funds"),
            SagaEvent::compensation_started("process_payment"),
            SagaEvent::compensation_step_completed("reserve_inventory"),
            SagaEvent::compensation_step_failed("reserve_inventory", "timeout"),
            SagaEvent::saga_completed(),
            SagaEvent::saga_failed("payment failed"),
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let deserialized: SagaEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event.event_type(), deserialized.event_type());
        }
    }

    #[test]
    fn test_saga_started_data() {
        let saga_id = AggregateId::new();
        let order_id = AggregateId::new();
        let event = SagaEvent::saga_started(saga_id, order_id, "OrderFulfillment");

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SagaEvent = serde_json::from_str(&json).unwrap();

        if let SagaEvent::SagaStarted(data) = deserialized {
            assert_eq!(data.saga_id, saga_id);
            assert_eq!(data.order_id, order_id);
            assert_eq!(data.saga_type, "OrderFulfillment");
        } else {
            panic!("Expected SagaStarted event");
        }
    }

    #[test]
    fn test_step_completed_data() {
        let event =
            SagaEvent::step_completed("process_payment", None, Some("PAY-123".to_string()), None);

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SagaEvent = serde_json::from_str(&json).unwrap();

        if let SagaEvent::StepCompleted(data) = deserialized {
            assert_eq!(data.step_name, "process_payment");
            assert_eq!(data.payment_id, Some("PAY-123".to_string()));
            assert!(data.reservation_id.is_none());
            assert!(data.tracking_number.is_none());
        } else {
            panic!("Expected StepCompleted event");
        }
    }
}
