//! Inventory service trait and in-memory implementation.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use common::AggregateId;
use domain::ProductId;

use crate::error::SagaError;

/// Result of a successful inventory reservation.
#[derive(Debug, Clone)]
pub struct ReservationResult {
    /// The reservation ID assigned by the inventory service.
    pub reservation_id: String,
}

/// An item to reserve in inventory.
#[derive(Debug, Clone)]
pub struct ReservationItem {
    /// The product to reserve.
    pub product_id: ProductId,
    /// Product name for display.
    pub product_name: String,
    /// Quantity to reserve.
    pub quantity: u32,
}

/// Trait for inventory management operations.
#[async_trait]
pub trait InventoryService: Send + Sync {
    /// Reserves inventory for the given order items.
    async fn reserve(
        &self,
        order_id: AggregateId,
        items: Vec<ReservationItem>,
    ) -> Result<ReservationResult, SagaError>;

    /// Releases a previously made reservation.
    async fn release(&self, reservation_id: &str) -> Result<(), SagaError>;
}

#[derive(Debug, Default)]
struct InMemoryInventoryState {
    reservations: HashMap<String, (AggregateId, Vec<ReservationItem>)>,
    next_id: u32,
    fail_on_reserve: bool,
}

/// In-memory inventory service for testing.
#[derive(Debug, Clone, Default)]
pub struct InMemoryInventoryService {
    state: Arc<RwLock<InMemoryInventoryState>>,
}

impl InMemoryInventoryService {
    /// Creates a new in-memory inventory service.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures the service to fail on the next reserve call.
    pub fn set_fail_on_reserve(&self, fail: bool) {
        self.state.write().unwrap().fail_on_reserve = fail;
    }

    /// Returns the number of active reservations.
    pub fn reservation_count(&self) -> usize {
        self.state.read().unwrap().reservations.len()
    }

    /// Returns true if a reservation exists with the given ID.
    pub fn has_reservation(&self, reservation_id: &str) -> bool {
        self.state
            .read()
            .unwrap()
            .reservations
            .contains_key(reservation_id)
    }
}

#[async_trait]
impl InventoryService for InMemoryInventoryService {
    async fn reserve(
        &self,
        order_id: AggregateId,
        items: Vec<ReservationItem>,
    ) -> Result<ReservationResult, SagaError> {
        let mut state = self.state.write().unwrap();

        if state.fail_on_reserve {
            return Err(SagaError::InventoryService(
                "Insufficient stock".to_string(),
            ));
        }

        state.next_id += 1;
        let reservation_id = format!("RES-{:04}", state.next_id);
        state
            .reservations
            .insert(reservation_id.clone(), (order_id, items));

        Ok(ReservationResult { reservation_id })
    }

    async fn release(&self, reservation_id: &str) -> Result<(), SagaError> {
        let mut state = self.state.write().unwrap();
        state.reservations.remove(reservation_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_reserve_and_release() {
        let service = InMemoryInventoryService::new();
        let order_id = AggregateId::new();
        let items = vec![ReservationItem {
            product_id: ProductId::new("SKU-001"),
            product_name: "Widget".to_string(),
            quantity: 2,
        }];

        let result = service.reserve(order_id, items).await.unwrap();
        assert!(result.reservation_id.starts_with("RES-"));
        assert_eq!(service.reservation_count(), 1);
        assert!(service.has_reservation(&result.reservation_id));

        service.release(&result.reservation_id).await.unwrap();
        assert_eq!(service.reservation_count(), 0);
    }

    #[tokio::test]
    async fn test_fail_on_reserve() {
        let service = InMemoryInventoryService::new();
        service.set_fail_on_reserve(true);

        let order_id = AggregateId::new();
        let items = vec![ReservationItem {
            product_id: ProductId::new("SKU-001"),
            product_name: "Widget".to_string(),
            quantity: 2,
        }];

        let result = service.reserve(order_id, items).await;
        assert!(result.is_err());
        assert_eq!(service.reservation_count(), 0);
    }

    #[tokio::test]
    async fn test_sequential_reservation_ids() {
        let service = InMemoryInventoryService::new();
        let order_id = AggregateId::new();

        let r1 = service.reserve(order_id, vec![]).await.unwrap();
        let r2 = service.reserve(order_id, vec![]).await.unwrap();

        assert_eq!(r1.reservation_id, "RES-0001");
        assert_eq!(r2.reservation_id, "RES-0002");
    }
}
