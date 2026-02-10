//! Order fulfillment saga constants.

/// The saga type identifier for order fulfillment.
pub const SAGA_TYPE: &str = "OrderFulfillment";

/// Step name: Reserve inventory for the order.
pub const STEP_RESERVE_INVENTORY: &str = "reserve_inventory";

/// Step name: Process payment for the order.
pub const STEP_PROCESS_PAYMENT: &str = "process_payment";

/// Step name: Create shipment for the order.
pub const STEP_CREATE_SHIPMENT: &str = "create_shipment";
