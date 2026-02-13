import type { ApiErrorResponse } from './types';

const API_BASE = import.meta.env.VITE_API_URL ?? 'http://localhost:3001';

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
		this.name = 'ApiError';
	}
}

async function handleResponse<T>(response: Response): Promise<T> {
	if (!response.ok) {
		let message = `HTTP ${response.status}`;
		try {
			const body: ApiErrorResponse = await response.json();
			message = body.error;
		} catch {
			// use default message
		}
		throw new ApiError(response.status, message);
	}
	return response.json() as Promise<T>;
}

export async function get<T>(path: string): Promise<T> {
	const response = await fetch(`${API_BASE}${path}`);
	return handleResponse<T>(response);
}

export async function post<T>(path: string, body?: unknown): Promise<T> {
	const response = await fetch(`${API_BASE}${path}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: body !== undefined ? JSON.stringify(body) : undefined
	});
	return handleResponse<T>(response);
}
