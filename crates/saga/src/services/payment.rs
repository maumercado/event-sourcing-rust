//! Payment service trait and in-memory implementation.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use common::AggregateId;
use domain::{CustomerId, Money};

use crate::error::SagaError;

/// Result of a successful payment charge.
#[derive(Debug, Clone)]
pub struct PaymentResult {
    /// The payment ID assigned by the payment service.
    pub payment_id: String,
}

/// Trait for payment processing operations.
#[async_trait]
pub trait PaymentService: Send + Sync {
    /// Charges a customer for an order.
    async fn charge(
        &self,
        order_id: AggregateId,
        customer_id: CustomerId,
        amount: Money,
    ) -> Result<PaymentResult, SagaError>;

    /// Refunds a previously made payment.
    async fn refund(&self, payment_id: &str) -> Result<(), SagaError>;
}

#[derive(Debug, Default)]
struct InMemoryPaymentState {
    payments: HashMap<String, (AggregateId, CustomerId, Money)>,
    next_id: u32,
    fail_on_charge: bool,
}

/// In-memory payment service for testing.
#[derive(Debug, Clone, Default)]
pub struct InMemoryPaymentService {
    state: Arc<RwLock<InMemoryPaymentState>>,
}

impl InMemoryPaymentService {
    /// Creates a new in-memory payment service.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures the service to fail on the next charge call.
    pub fn set_fail_on_charge(&self, fail: bool) {
        self.state.write().unwrap().fail_on_charge = fail;
    }

    /// Returns the number of active payments.
    pub fn payment_count(&self) -> usize {
        self.state.read().unwrap().payments.len()
    }

    /// Returns true if a payment exists with the given ID.
    pub fn has_payment(&self, payment_id: &str) -> bool {
        self.state.read().unwrap().payments.contains_key(payment_id)
    }
}

#[async_trait]
impl PaymentService for InMemoryPaymentService {
    async fn charge(
        &self,
        order_id: AggregateId,
        customer_id: CustomerId,
        amount: Money,
    ) -> Result<PaymentResult, SagaError> {
        let mut state = self.state.write().unwrap();

        if state.fail_on_charge {
            return Err(SagaError::PaymentService("Payment declined".to_string()));
        }

        state.next_id += 1;
        let payment_id = format!("PAY-{:04}", state.next_id);
        state
            .payments
            .insert(payment_id.clone(), (order_id, customer_id, amount));

        Ok(PaymentResult { payment_id })
    }

    async fn refund(&self, payment_id: &str) -> Result<(), SagaError> {
        let mut state = self.state.write().unwrap();
        state.payments.remove(payment_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_charge_and_refund() {
        let service = InMemoryPaymentService::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();
        let amount = Money::from_cents(5000);

        let result = service.charge(order_id, customer_id, amount).await.unwrap();
        assert!(result.payment_id.starts_with("PAY-"));
        assert_eq!(service.payment_count(), 1);
        assert!(service.has_payment(&result.payment_id));

        service.refund(&result.payment_id).await.unwrap();
        assert_eq!(service.payment_count(), 0);
    }

    #[tokio::test]
    async fn test_fail_on_charge() {
        let service = InMemoryPaymentService::new();
        service.set_fail_on_charge(true);

        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();
        let amount = Money::from_cents(5000);

        let result = service.charge(order_id, customer_id, amount).await;
        assert!(result.is_err());
        assert_eq!(service.payment_count(), 0);
    }

    #[tokio::test]
    async fn test_sequential_payment_ids() {
        let service = InMemoryPaymentService::new();
        let order_id = AggregateId::new();
        let customer_id = CustomerId::new();
        let amount = Money::from_cents(1000);

        let r1 = service.charge(order_id, customer_id, amount).await.unwrap();
        let r2 = service.charge(order_id, customer_id, amount).await.unwrap();

        assert_eq!(r1.payment_id, "PAY-0001");
        assert_eq!(r2.payment_id, "PAY-0002");
    }
}
