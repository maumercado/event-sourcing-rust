//! Value objects for the order domain.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a customer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CustomerId(Uuid);

impl CustomerId {
    /// Creates a new random customer ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a customer ID from an existing UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for CustomerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CustomerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for CustomerId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<CustomerId> for Uuid {
    fn from(id: CustomerId) -> Self {
        id.0
    }
}

/// Product identifier (SKU).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProductId(String);

impl ProductId {
    /// Creates a new product ID from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the product ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProductId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ProductId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ProductId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for ProductId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Money amount represented in cents to avoid floating point issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Money {
    /// Amount in cents (e.g., 1000 = $10.00)
    cents: i64,
}

impl Money {
    /// Creates a new Money amount from cents.
    pub fn from_cents(cents: i64) -> Self {
        Self { cents }
    }

    /// Creates a new Money amount from a dollar value.
    ///
    /// The cents portion is calculated as dollars * 100.
    pub fn from_dollars(dollars: i64) -> Self {
        Self {
            cents: dollars * 100,
        }
    }

    /// Returns zero money.
    pub fn zero() -> Self {
        Self { cents: 0 }
    }

    /// Returns the amount in cents.
    pub fn cents(&self) -> i64 {
        self.cents
    }

    /// Returns the dollar portion (whole number).
    pub fn dollars(&self) -> i64 {
        self.cents / 100
    }

    /// Returns the cents portion (remainder after dollars).
    pub fn cents_part(&self) -> i64 {
        self.cents.abs() % 100
    }

    /// Returns true if the amount is positive.
    pub fn is_positive(&self) -> bool {
        self.cents > 0
    }

    /// Returns true if the amount is zero.
    pub fn is_zero(&self) -> bool {
        self.cents == 0
    }

    /// Returns true if the amount is negative.
    pub fn is_negative(&self) -> bool {
        self.cents < 0
    }

    /// Adds another money amount.
    pub fn add(&self, other: Money) -> Money {
        Money {
            cents: self.cents + other.cents,
        }
    }

    /// Subtracts another money amount.
    pub fn subtract(&self, other: Money) -> Money {
        Money {
            cents: self.cents - other.cents,
        }
    }

    /// Multiplies by a quantity.
    pub fn multiply(&self, quantity: u32) -> Money {
        Money {
            cents: self.cents * quantity as i64,
        }
    }
}

impl Default for Money {
    fn default() -> Self {
        Self::zero()
    }
}

impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.cents < 0 {
            write!(f, "-${}.{:02}", self.dollars().abs(), self.cents_part())
        } else {
            write!(f, "${}.{:02}", self.dollars(), self.cents_part())
        }
    }
}

impl std::ops::Add for Money {
    type Output = Money;

    fn add(self, rhs: Self) -> Self::Output {
        Money {
            cents: self.cents + rhs.cents,
        }
    }
}

impl std::ops::Sub for Money {
    type Output = Money;

    fn sub(self, rhs: Self) -> Self::Output {
        Money {
            cents: self.cents - rhs.cents,
        }
    }
}

impl std::ops::AddAssign for Money {
    fn add_assign(&mut self, rhs: Self) {
        self.cents += rhs.cents;
    }
}

impl std::ops::SubAssign for Money {
    fn sub_assign(&mut self, rhs: Self) {
        self.cents -= rhs.cents;
    }
}

/// An item in an order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderItem {
    /// The product identifier.
    pub product_id: ProductId,

    /// Human-readable product name.
    pub product_name: String,

    /// Quantity ordered.
    pub quantity: u32,

    /// Price per unit in cents.
    pub unit_price: Money,
}

impl OrderItem {
    /// Creates a new order item.
    pub fn new(
        product_id: impl Into<ProductId>,
        product_name: impl Into<String>,
        quantity: u32,
        unit_price: Money,
    ) -> Self {
        Self {
            product_id: product_id.into(),
            product_name: product_name.into(),
            quantity,
            unit_price,
        }
    }

    /// Returns the total price for this item (quantity * unit_price).
    pub fn total_price(&self) -> Money {
        self.unit_price.multiply(self.quantity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_customer_id_new_creates_unique_ids() {
        let id1 = CustomerId::new();
        let id2 = CustomerId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_customer_id_from_uuid_preserves_value() {
        let uuid = Uuid::new_v4();
        let id = CustomerId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), uuid);
    }

    #[test]
    fn test_product_id_string_conversion() {
        let id = ProductId::new("SKU-001");
        assert_eq!(id.as_str(), "SKU-001");

        let id2: ProductId = "SKU-002".into();
        assert_eq!(id2.as_str(), "SKU-002");
    }

    #[test]
    fn test_money_from_cents() {
        let money = Money::from_cents(1234);
        assert_eq!(money.cents(), 1234);
        assert_eq!(money.dollars(), 12);
        assert_eq!(money.cents_part(), 34);
    }

    #[test]
    fn test_money_from_dollars() {
        let money = Money::from_dollars(50);
        assert_eq!(money.cents(), 5000);
        assert_eq!(money.dollars(), 50);
        assert_eq!(money.cents_part(), 0);
    }

    #[test]
    fn test_money_display() {
        assert_eq!(Money::from_cents(1234).to_string(), "$12.34");
        assert_eq!(Money::from_cents(100).to_string(), "$1.00");
        assert_eq!(Money::from_cents(5).to_string(), "$0.05");
        assert_eq!(Money::from_cents(-1234).to_string(), "-$12.34");
    }

    #[test]
    fn test_money_arithmetic() {
        let a = Money::from_cents(1000);
        let b = Money::from_cents(500);

        assert_eq!((a + b).cents(), 1500);
        assert_eq!((a - b).cents(), 500);
        assert_eq!(a.multiply(3).cents(), 3000);
    }

    #[test]
    fn test_money_comparison() {
        assert!(Money::from_cents(100).is_positive());
        assert!(Money::from_cents(0).is_zero());
        assert!(Money::from_cents(-100).is_negative());
    }

    #[test]
    fn test_order_item_total_price() {
        let item = OrderItem::new("SKU-001", "Widget", 3, Money::from_cents(1000));
        assert_eq!(item.total_price().cents(), 3000);
    }

    #[test]
    fn test_order_item_serialization() {
        let item = OrderItem::new("SKU-001", "Widget", 2, Money::from_cents(999));
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: OrderItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deserialized);
    }

    #[test]
    fn test_money_add_assign() {
        let mut money = Money::from_cents(100);
        money += Money::from_cents(50);
        assert_eq!(money.cents(), 150);
    }

    #[test]
    fn test_money_sub_assign() {
        let mut money = Money::from_cents(100);
        money -= Money::from_cents(30);
        assert_eq!(money.cents(), 70);
    }
}
