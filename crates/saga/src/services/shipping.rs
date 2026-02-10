//! Shipping service trait and in-memory implementation.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use common::AggregateId;

use crate::error::SagaError;

/// Result of a successful shipment creation.
#[derive(Debug, Clone)]
pub struct ShipmentResult {
    /// The tracking number assigned by the shipping service.
    pub tracking_number: String,
}

/// Trait for shipping operations.
#[async_trait]
pub trait ShippingService: Send + Sync {
    /// Creates a shipment for an order.
    async fn create_shipment(&self, order_id: AggregateId) -> Result<ShipmentResult, SagaError>;

    /// Cancels a previously created shipment.
    async fn cancel_shipment(&self, tracking_number: &str) -> Result<(), SagaError>;
}

#[derive(Debug, Default)]
struct InMemoryShippingState {
    shipments: HashMap<String, AggregateId>,
    next_id: u32,
    fail_on_create: bool,
}

/// In-memory shipping service for testing.
#[derive(Debug, Clone, Default)]
pub struct InMemoryShippingService {
    state: Arc<RwLock<InMemoryShippingState>>,
}

impl InMemoryShippingService {
    /// Creates a new in-memory shipping service.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures the service to fail on the next create_shipment call.
    pub fn set_fail_on_create(&self, fail: bool) {
        self.state.write().unwrap().fail_on_create = fail;
    }

    /// Returns the number of active shipments.
    pub fn shipment_count(&self) -> usize {
        self.state.read().unwrap().shipments.len()
    }

    /// Returns true if a shipment exists with the given tracking number.
    pub fn has_shipment(&self, tracking_number: &str) -> bool {
        self.state
            .read()
            .unwrap()
            .shipments
            .contains_key(tracking_number)
    }
}

#[async_trait]
impl ShippingService for InMemoryShippingService {
    async fn create_shipment(&self, order_id: AggregateId) -> Result<ShipmentResult, SagaError> {
        let mut state = self.state.write().unwrap();

        if state.fail_on_create {
            return Err(SagaError::ShippingService(
                "Shipping unavailable".to_string(),
            ));
        }

        state.next_id += 1;
        let tracking_number = format!("TRACK-{:04}", state.next_id);
        state.shipments.insert(tracking_number.clone(), order_id);

        Ok(ShipmentResult { tracking_number })
    }

    async fn cancel_shipment(&self, tracking_number: &str) -> Result<(), SagaError> {
        let mut state = self.state.write().unwrap();
        state.shipments.remove(tracking_number);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_cancel_shipment() {
        let service = InMemoryShippingService::new();
        let order_id = AggregateId::new();

        let result = service.create_shipment(order_id).await.unwrap();
        assert!(result.tracking_number.starts_with("TRACK-"));
        assert_eq!(service.shipment_count(), 1);
        assert!(service.has_shipment(&result.tracking_number));

        service
            .cancel_shipment(&result.tracking_number)
            .await
            .unwrap();
        assert_eq!(service.shipment_count(), 0);
    }

    #[tokio::test]
    async fn test_fail_on_create() {
        let service = InMemoryShippingService::new();
        service.set_fail_on_create(true);

        let order_id = AggregateId::new();
        let result = service.create_shipment(order_id).await;
        assert!(result.is_err());
        assert_eq!(service.shipment_count(), 0);
    }

    #[tokio::test]
    async fn test_sequential_tracking_numbers() {
        let service = InMemoryShippingService::new();
        let order_id = AggregateId::new();

        let r1 = service.create_shipment(order_id).await.unwrap();
        let r2 = service.create_shipment(order_id).await.unwrap();

        assert_eq!(r1.tracking_number, "TRACK-0001");
        assert_eq!(r2.tracking_number, "TRACK-0002");
    }
}
