<script lang="ts">
	import { page } from '$app/state';
	import { getOrder, getOrderEvents, submitOrder, fulfillOrder, getSagaStatus } from '$lib/api/orders';
	import type { OrderResponse, EventEnvelopeResponse, SagaStatusResponse } from '$lib/api/types';
	import { formatCents } from '$lib/utils/money';
	import OrderStateBadge from '$lib/components/OrderStateBadge.svelte';
	import EventTimeline from '$lib/components/EventTimeline.svelte';
	import StateMachine from '$lib/components/StateMachine.svelte';
	import SagaProgress from '$lib/components/SagaProgress.svelte';

	let order = $state<OrderResponse | null>(null);
	let events = $state<EventEnvelopeResponse[]>([]);
	let saga = $state<SagaStatusResponse | null>(null);
	let loading = $state(true);
	let error = $state('');
	let actionLoading = $state('');

	const orderId = $derived(page.params.id!);

	async function loadAll() {
		loading = true;
		error = '';
		try {
			const [o, e] = await Promise.all([getOrder(orderId), getOrderEvents(orderId)]);
			order = o;
			events = e;

			// Try to find saga events to auto-detect saga ID
			const sagaStartEvent = e.find(
				(ev) => ev.event_type === 'OrderCompleted' || ev.event_type === 'OrderCancelled'
			);
			if (sagaStartEvent || o.state === 'Completed' || o.state === 'Processing') {
				// Look for saga by searching fulfill response from events
				// We'll try each saga approach
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load order';
		} finally {
			loading = false;
		}
	}

	async function handleSubmit() {
		actionLoading = 'submit';
		try {
			await submitOrder(orderId);
			await loadAll();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to submit order';
		} finally {
			actionLoading = '';
		}
	}

	async function handleFulfill() {
		actionLoading = 'fulfill';
		try {
			const result = await fulfillOrder(orderId);
			// Load saga status
			saga = await getSagaStatus(result.saga_id);
			await loadAll();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to fulfill order';
		} finally {
			actionLoading = '';
		}
	}

	function truncateId(id: string): string {
		return id.slice(0, 8);
	}

	$effect(() => {
		// Re-run when orderId changes
		orderId;
		loadAll();
	});
</script>

{#if loading}
	<p class="text-gray-500">Loading order...</p>
{:else if error && !order}
	<div class="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700">{error}</div>
{:else if order}
	<div class="space-y-6">
		<!-- Back link -->
		<a href="/orders" class="text-sm text-gray-500 hover:text-gray-900">&larr; Back to orders</a>

		{#if error}
			<div class="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700">{error}</div>
		{/if}

		<!-- Order info -->
		<div class="rounded-lg border border-gray-200 bg-white p-6">
			<div class="mb-4 flex items-center justify-between">
				<div>
					<h1 class="text-xl font-bold text-gray-900">
						Order <span class="font-mono" title={order.id}>{truncateId(order.id)}</span>
					</h1>
					<p class="mt-1 text-sm text-gray-500">
						Customer: <span class="font-mono" title={order.customer_id}>{truncateId(order.customer_id)}</span>
					</p>
				</div>
				<OrderStateBadge state={order.state} />
			</div>

			<!-- Items table -->
			{#if order.items.length > 0}
				<table class="mt-4 w-full text-sm">
					<thead>
						<tr class="border-b border-gray-100 text-left text-gray-500">
							<th class="pb-2 font-medium">Product</th>
							<th class="pb-2 font-medium">SKU</th>
							<th class="pb-2 text-right font-medium">Qty</th>
							<th class="pb-2 text-right font-medium">Unit Price</th>
							<th class="pb-2 text-right font-medium">Subtotal</th>
						</tr>
					</thead>
					<tbody>
						{#each order.items as item}
							<tr class="border-b border-gray-50">
								<td class="py-2 text-gray-900">{item.product_name}</td>
								<td class="py-2 font-mono text-gray-500">{item.product_id}</td>
								<td class="py-2 text-right text-gray-700">{item.quantity}</td>
								<td class="py-2 text-right text-gray-700">{formatCents(item.unit_price_cents)}</td>
								<td class="py-2 text-right font-medium text-gray-900">
									{formatCents(item.unit_price_cents * item.quantity)}
								</td>
							</tr>
						{/each}
					</tbody>
					<tfoot>
						<tr>
							<td colspan="4" class="pt-3 text-right font-medium text-gray-700">Total</td>
							<td class="pt-3 text-right text-lg font-bold text-gray-900">
								{formatCents(order.total_cents)}
							</td>
						</tr>
					</tfoot>
				</table>
			{/if}

			<!-- Action buttons -->
			<div class="mt-6 flex gap-3">
				{#if order.state === 'Draft'}
					<button
						onclick={handleSubmit}
						disabled={actionLoading !== ''}
						class="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
					>
						{actionLoading === 'submit' ? 'Submitting...' : 'Submit Order'}
					</button>
				{/if}
				{#if order.state === 'Reserved'}
					<button
						onclick={handleFulfill}
						disabled={actionLoading !== ''}
						class="rounded-md bg-amber-600 px-4 py-2 text-sm font-medium text-white hover:bg-amber-700 disabled:opacity-50"
					>
						{actionLoading === 'fulfill' ? 'Fulfilling...' : 'Fulfill (Saga)'}
					</button>
				{/if}
			</div>
		</div>

		<!-- State Machine -->
		<StateMachine currentState={order.state} />

		<!-- Saga Progress (if available) -->
		{#if saga}
			<SagaProgress {saga} />
		{/if}

		<!-- Event Timeline -->
		<EventTimeline {events} />
	</div>
{/if}
