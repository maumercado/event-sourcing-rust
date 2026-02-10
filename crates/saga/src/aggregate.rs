//! Saga instance aggregate.

use common::AggregateId;
use domain::Aggregate;
use event_store::Version;
use serde::{Deserialize, Serialize};

use crate::error::SagaError;
use crate::events::SagaEvent;
use crate::state::SagaState;

/// An event-sourced saga instance.
///
/// Tracks the state of a saga execution including completed steps
/// and context accumulated during execution (reservation IDs, payment IDs, etc.).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SagaInstance {
    id: Option<AggregateId>,
    version: Version,
    saga_type: String,
    order_id: Option<AggregateId>,
    state: SagaState,
    current_step: usize,
    completed_steps: Vec<String>,
    /// Reservation ID from inventory service.
    reservation_id: Option<String>,
    /// Payment ID from payment service.
    payment_id: Option<String>,
    /// Tracking number from shipping service.
    tracking_number: Option<String>,
    /// Reason for failure, if any.
    failure_reason: Option<String>,
}

impl Aggregate for SagaInstance {
    type Event = SagaEvent;
    type Error = SagaError;

    fn aggregate_type() -> &'static str {
        "OrderFulfillmentSaga"
    }

    fn id(&self) -> Option<AggregateId> {
        self.id
    }

    fn version(&self) -> Version {
        self.version
    }

    fn set_version(&mut self, version: Version) {
        self.version = version;
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            SagaEvent::SagaStarted(data) => {
                self.id = Some(data.saga_id);
                self.order_id = Some(data.order_id);
                self.saga_type = data.saga_type;
                self.state = SagaState::Running;
            }
            SagaEvent::StepStarted(_) => {
                self.current_step += 1;
            }
            SagaEvent::StepCompleted(data) => {
                self.completed_steps.push(data.step_name);
                if let Some(rid) = data.reservation_id {
                    self.reservation_id = Some(rid);
                }
                if let Some(pid) = data.payment_id {
                    self.payment_id = Some(pid);
                }
                if let Some(tn) = data.tracking_number {
                    self.tracking_number = Some(tn);
                }
            }
            SagaEvent::StepFailed(data) => {
                self.failure_reason = Some(data.error);
            }
            SagaEvent::CompensationStarted(_) => {
                self.state = SagaState::Compensating;
            }
            SagaEvent::CompensationStepCompleted(_) => {
                // Compensation step tracked but no state change needed
            }
            SagaEvent::CompensationStepFailed(_) => {
                // Compensation failures are logged but don't stop the chain
            }
            SagaEvent::SagaCompleted(_) => {
                self.state = SagaState::Completed;
            }
            SagaEvent::SagaFailed(data) => {
                self.state = SagaState::Failed;
                self.failure_reason = Some(data.reason);
            }
        }
    }
}

// Query methods
impl SagaInstance {
    /// Returns the saga state.
    pub fn state(&self) -> SagaState {
        self.state
    }

    /// Returns the order ID this saga is fulfilling.
    pub fn order_id(&self) -> Option<AggregateId> {
        self.order_id
    }

    /// Returns the saga type.
    pub fn saga_type(&self) -> &str {
        &self.saga_type
    }

    /// Returns the list of completed step names.
    pub fn completed_steps(&self) -> &[String] {
        &self.completed_steps
    }

    /// Returns the reservation ID, if set.
    pub fn reservation_id(&self) -> Option<&str> {
        self.reservation_id.as_deref()
    }

    /// Returns the payment ID, if set.
    pub fn payment_id(&self) -> Option<&str> {
        self.payment_id.as_deref()
    }

    /// Returns the tracking number, if set.
    pub fn tracking_number(&self) -> Option<&str> {
        self.tracking_number.as_deref()
    }

    /// Returns the failure reason, if any.
    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order_fulfillment;

    fn make_saga_id() -> AggregateId {
        AggregateId::new()
    }

    fn make_order_id() -> AggregateId {
        AggregateId::new()
    }

    #[test]
    fn test_default_saga_instance() {
        let saga = SagaInstance::default();
        assert!(saga.id().is_none());
        assert_eq!(saga.state(), SagaState::NotStarted);
        assert!(saga.completed_steps().is_empty());
    }

    #[test]
    fn test_apply_saga_started() {
        let mut saga = SagaInstance::default();
        let saga_id = make_saga_id();
        let order_id = make_order_id();

        saga.apply(SagaEvent::saga_started(
            saga_id,
            order_id,
            order_fulfillment::SAGA_TYPE,
        ));

        assert_eq!(saga.id(), Some(saga_id));
        assert_eq!(saga.order_id(), Some(order_id));
        assert_eq!(saga.saga_type(), order_fulfillment::SAGA_TYPE);
        assert_eq!(saga.state(), SagaState::Running);
    }

    #[test]
    fn test_apply_step_lifecycle() {
        let mut saga = SagaInstance::default();
        let saga_id = make_saga_id();
        let order_id = make_order_id();

        saga.apply(SagaEvent::saga_started(
            saga_id,
            order_id,
            order_fulfillment::SAGA_TYPE,
        ));

        // Step 1: Reserve inventory
        saga.apply(SagaEvent::step_started(
            order_fulfillment::STEP_RESERVE_INVENTORY,
        ));
        assert_eq!(saga.current_step, 1);

        saga.apply(SagaEvent::step_completed(
            order_fulfillment::STEP_RESERVE_INVENTORY,
            Some("RES-123".to_string()),
            None,
            None,
        ));
        assert_eq!(saga.completed_steps(), &["reserve_inventory"]);
        assert_eq!(saga.reservation_id(), Some("RES-123"));

        // Step 2: Process payment
        saga.apply(SagaEvent::step_started(
            order_fulfillment::STEP_PROCESS_PAYMENT,
        ));
        assert_eq!(saga.current_step, 2);

        saga.apply(SagaEvent::step_completed(
            order_fulfillment::STEP_PROCESS_PAYMENT,
            None,
            Some("PAY-456".to_string()),
            None,
        ));
        assert_eq!(saga.completed_steps().len(), 2);
        assert_eq!(saga.payment_id(), Some("PAY-456"));

        // Step 3: Create shipment
        saga.apply(SagaEvent::step_started(
            order_fulfillment::STEP_CREATE_SHIPMENT,
        ));
        assert_eq!(saga.current_step, 3);

        saga.apply(SagaEvent::step_completed(
            order_fulfillment::STEP_CREATE_SHIPMENT,
            None,
            None,
            Some("TRACK-789".to_string()),
        ));
        assert_eq!(saga.completed_steps().len(), 3);
        assert_eq!(saga.tracking_number(), Some("TRACK-789"));

        // Saga completed
        saga.apply(SagaEvent::saga_completed());
        assert_eq!(saga.state(), SagaState::Completed);
        assert!(saga.state().is_terminal());
    }

    #[test]
    fn test_apply_step_failure_and_compensation() {
        let mut saga = SagaInstance::default();
        let saga_id = make_saga_id();
        let order_id = make_order_id();

        saga.apply(SagaEvent::saga_started(
            saga_id,
            order_id,
            order_fulfillment::SAGA_TYPE,
        ));

        // Step 1 succeeds
        saga.apply(SagaEvent::step_started(
            order_fulfillment::STEP_RESERVE_INVENTORY,
        ));
        saga.apply(SagaEvent::step_completed(
            order_fulfillment::STEP_RESERVE_INVENTORY,
            Some("RES-123".to_string()),
            None,
            None,
        ));

        // Step 2 fails
        saga.apply(SagaEvent::step_started(
            order_fulfillment::STEP_PROCESS_PAYMENT,
        ));
        saga.apply(SagaEvent::step_failed(
            order_fulfillment::STEP_PROCESS_PAYMENT,
            "insufficient funds",
        ));
        assert_eq!(saga.failure_reason(), Some("insufficient funds"));

        // Compensation
        saga.apply(SagaEvent::compensation_started(
            order_fulfillment::STEP_PROCESS_PAYMENT,
        ));
        assert_eq!(saga.state(), SagaState::Compensating);

        saga.apply(SagaEvent::compensation_step_completed(
            order_fulfillment::STEP_RESERVE_INVENTORY,
        ));

        // Saga failed
        saga.apply(SagaEvent::saga_failed("Payment failed: insufficient funds"));
        assert_eq!(saga.state(), SagaState::Failed);
        assert!(saga.state().is_terminal());
        assert_eq!(
            saga.failure_reason(),
            Some("Payment failed: insufficient funds")
        );
    }

    #[test]
    fn test_compensation_step_failure_does_not_change_state() {
        let mut saga = SagaInstance::default();
        let saga_id = make_saga_id();
        let order_id = make_order_id();

        saga.apply(SagaEvent::saga_started(
            saga_id,
            order_id,
            order_fulfillment::SAGA_TYPE,
        ));
        saga.apply(SagaEvent::step_started(
            order_fulfillment::STEP_RESERVE_INVENTORY,
        ));
        saga.apply(SagaEvent::step_failed(
            order_fulfillment::STEP_RESERVE_INVENTORY,
            "error",
        ));
        saga.apply(SagaEvent::compensation_started(
            order_fulfillment::STEP_RESERVE_INVENTORY,
        ));

        assert_eq!(saga.state(), SagaState::Compensating);

        saga.apply(SagaEvent::compensation_step_failed(
            order_fulfillment::STEP_RESERVE_INVENTORY,
            "service unavailable",
        ));

        // Still compensating — compensation failures don't stop the chain
        assert_eq!(saga.state(), SagaState::Compensating);
    }

    #[test]
    fn test_aggregate_type() {
        assert_eq!(SagaInstance::aggregate_type(), "OrderFulfillmentSaga");
    }

    #[test]
    fn test_serialization() {
        let mut saga = SagaInstance::default();
        let saga_id = make_saga_id();
        let order_id = make_order_id();

        saga.apply(SagaEvent::saga_started(
            saga_id,
            order_id,
            order_fulfillment::SAGA_TYPE,
        ));
        saga.apply(SagaEvent::step_started(
            order_fulfillment::STEP_RESERVE_INVENTORY,
        ));
        saga.apply(SagaEvent::step_completed(
            order_fulfillment::STEP_RESERVE_INVENTORY,
            Some("RES-1".into()),
            None,
            None,
        ));

        let json = serde_json::to_string(&saga).unwrap();
        let deserialized: SagaInstance = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id(), Some(saga_id));
        assert_eq!(deserialized.state(), SagaState::Running);
        assert_eq!(deserialized.reservation_id(), Some("RES-1"));
    }
}
