import { get, post } from './client';
import type {
	CreateOrderRequest,
	EventEnvelopeResponse,
	FulfillResponse,
	HealthResponse,
	OrderCreatedResponse,
	OrderResponse,
	SagaStatusResponse
} from './types';

export async function listOrders(): Promise<OrderResponse[]> {
	return get<OrderResponse[]>('/orders');
}

export async function getOrder(id: string): Promise<OrderResponse> {
	return get<OrderResponse>(`/orders/${id}`);
}

export async function createOrder(req: CreateOrderRequest): Promise<OrderCreatedResponse> {
	return post<OrderCreatedResponse>('/orders', req);
}

export async function submitOrder(id: string): Promise<OrderResponse> {
	return post<OrderResponse>(`/orders/${id}/submit`);
}

export async function fulfillOrder(id: string): Promise<FulfillResponse> {
	return post<FulfillResponse>(`/orders/${id}/fulfill`);
}

export async function getOrderEvents(id: string): Promise<EventEnvelopeResponse[]> {
	return get<EventEnvelopeResponse[]>(`/orders/${id}/events`);
}

export async function getSagaStatus(sagaId: string): Promise<SagaStatusResponse> {
	return get<SagaStatusResponse>(`/orders/${sagaId}/saga`);
}

export async function checkHealth(): Promise<HealthResponse> {
	return get<HealthResponse>('/health');
}
