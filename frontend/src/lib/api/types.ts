export interface OrderItemRequest {
	product_id: string;
	product_name: string;
	quantity: number;
	unit_price_cents: number;
}

export interface CreateOrderRequest {
	customer_id?: string;
	items: OrderItemRequest[];
}

export interface OrderCreatedResponse {
	order_id: string;
	state: string;
}

export interface OrderItemResponse {
	product_id: string;
	product_name: string;
	quantity: number;
	unit_price_cents: number;
}

export interface OrderResponse {
	id: string;
	customer_id: string;
	state: OrderState;
	items: OrderItemResponse[];
	total_cents: number;
}

export type OrderState = 'Draft' | 'Reserved' | 'Processing' | 'Completed' | 'Cancelled';

export interface FulfillResponse {
	saga_id: string;
	saga_state: string;
}

export interface SagaStatusResponse {
	saga_id: string;
	order_id: string;
	state: string;
	completed_steps: string[];
	reservation_id: string | null;
	payment_id: string | null;
	tracking_number: string | null;
	failure_reason: string | null;
}

export interface EventEnvelopeResponse {
	event_id: string;
	event_type: string;
	aggregate_id: string;
	version: number;
	timestamp: string;
	payload: Record<string, unknown>;
}

export interface HealthResponse {
	status: string;
}

export interface ApiErrorResponse {
	error: string;
}
