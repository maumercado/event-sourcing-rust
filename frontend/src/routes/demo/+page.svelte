<script lang="ts">
	import { createOrder, submitOrder, fulfillOrder, getOrder, getOrderEvents, getSagaStatus } from '$lib/api/orders';
	import type { OrderResponse, EventEnvelopeResponse, SagaStatusResponse } from '$lib/api/types';
	import { formatCents } from '$lib/utils/money';
	import OrderStateBadge from '$lib/components/OrderStateBadge.svelte';
	import EventTimeline from '$lib/components/EventTimeline.svelte';
	import StateMachine from '$lib/components/StateMachine.svelte';
	import SagaProgress from '$lib/components/SagaProgress.svelte';

	let step = $state(0);
	let orderId = $state('');
	let order = $state<OrderResponse | null>(null);
	let events = $state<EventEnvelopeResponse[]>([]);
	let saga = $state<SagaStatusResponse | null>(null);
	let loading = $state(false);
	let error = $state('');

	async function refresh() {
		if (!orderId) return;
		const [o, e] = await Promise.all([getOrder(orderId), getOrderEvents(orderId)]);
		order = o;
		events = e;
	}

	async function step1CreateOrder() {
		loading = true;
		error = '';
		try {
			const result = await createOrder({
				items: [
					{ product_id: 'SKU-DEMO-A', product_name: 'Demo Widget', quantity: 3, unit_price_cents: 1999 },
					{ product_id: 'SKU-DEMO-B', product_name: 'Demo Gadget', quantity: 1, unit_price_cents: 4999 }
				]
			});
			orderId = result.order_id;
			await refresh();
			step = 1;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to create order';
		} finally {
			loading = false;
		}
	}

	async function step2SubmitOrder() {
		loading = true;
		error = '';
		try {
			await submitOrder(orderId);
			await refresh();
			step = 2;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to submit order';
		} finally {
			loading = false;
		}
	}

	async function step3FulfillOrder() {
		loading = true;
		error = '';
		try {
			const result = await fulfillOrder(orderId);
			saga = await getSagaStatus(result.saga_id);
			await refresh();
			step = 3;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to fulfill order';
		} finally {
			loading = false;
		}
	}

	function reset() {
		step = 0;
		orderId = '';
		order = null;
		events = [];
		saga = null;
		error = '';
	}

	const stepInfo = [
		{
			title: '1. Create an Order',
			description: 'Creates a new order aggregate with two items. This emits OrderCreated and ItemAdded events to the event store.'
		},
		{
			title: '2. Observe Events',
			description: 'The event timeline below shows the immutable events that represent the order\'s history. Each event has a version number and timestamp.'
		},
		{
			title: '3. Submit the Order',
			description: 'Submitting transitions the order from Draft to Reserved. This emits an OrderSubmitted event.'
		},
		{
			title: '4. Fulfill via Saga',
			description: 'Fulfillment triggers a 3-step saga: Reserve Inventory, Process Payment, Create Shipment. Each step calls an external service.'
		}
	];
</script>

<div class="space-y-6">
	<div>
		<h1 class="text-2xl font-bold text-gray-900">Guided Demo</h1>
		<p class="mt-1 text-gray-600">Walk through the event-sourcing lifecycle step by step.</p>
	</div>

	{#if error}
		<div class="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700">{error}</div>
	{/if}

	<!-- Step indicators -->
	<div class="flex gap-2">
		{#each stepInfo as info, i}
			<div
				class="flex-1 rounded-md px-3 py-2 text-center text-xs font-medium
				{i < step ? 'bg-emerald-100 text-emerald-700' :
				 i === step ? 'bg-gray-900 text-white' :
				 'bg-gray-100 text-gray-400'}"
			>
				{info.title}
			</div>
		{/each}
	</div>

	<!-- Current step content -->
	<div class="rounded-lg border border-gray-200 bg-white p-6">
		<h2 class="text-lg font-semibold text-gray-900">{stepInfo[Math.min(step, 3)].title}</h2>
		<p class="mt-1 text-sm text-gray-600">{stepInfo[Math.min(step, 3)].description}</p>

		<div class="mt-4">
			{#if step === 0}
				<button
					onclick={step1CreateOrder}
					disabled={loading}
					class="rounded-md bg-gray-900 px-4 py-2 text-sm font-medium text-white hover:bg-gray-800 disabled:opacity-50"
				>
					{loading ? 'Creating...' : 'Create Order'}
				</button>
			{:else if step === 1}
				<div class="mb-4 rounded bg-blue-50 p-3 text-sm text-blue-700">
					Order created! Notice the <strong>OrderCreated</strong> and <strong>ItemAdded</strong> events in the timeline below.
					Each event is immutable and versioned.
				</div>
				<button
					onclick={step2SubmitOrder}
					disabled={loading}
					class="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
				>
					{loading ? 'Submitting...' : 'Submit Order'}
				</button>
			{:else if step === 2}
				<div class="mb-4 rounded bg-amber-50 p-3 text-sm text-amber-700">
					Order submitted! The state machine shows the transition from <strong>Draft</strong> to <strong>Reserved</strong>.
					Now trigger the saga to fulfill it.
				</div>
				<button
					onclick={step3FulfillOrder}
					disabled={loading}
					class="rounded-md bg-amber-600 px-4 py-2 text-sm font-medium text-white hover:bg-amber-700 disabled:opacity-50"
				>
					{loading ? 'Fulfilling...' : 'Fulfill via Saga'}
				</button>
			{:else}
				<div class="mb-4 rounded bg-emerald-50 p-3 text-sm text-emerald-700">
					The saga completed all 3 steps. The order is now <strong>Completed</strong>.
					Check the event timeline to see the full history of state changes.
				</div>
				<div class="flex gap-3">
					<a
						href="/orders/{orderId}"
						class="rounded-md bg-gray-900 px-4 py-2 text-sm font-medium text-white hover:bg-gray-800"
					>
						View Order Detail
					</a>
					<button
						onclick={reset}
						class="rounded-md border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50"
					>
						Start Over
					</button>
				</div>
			{/if}
		</div>
	</div>

	<!-- Live visualizations -->
	{#if order}
		<div class="grid gap-6 lg:grid-cols-2">
			<div>
				<div class="mb-4 rounded-lg border border-gray-200 bg-white p-6">
					<div class="flex items-center justify-between">
						<h3 class="font-semibold text-gray-900">Order Status</h3>
						<OrderStateBadge state={order.state} />
					</div>
					<p class="mt-2 text-sm text-gray-600">
						{order.items.length} items &middot; {formatCents(order.total_cents)}
					</p>
				</div>
				<StateMachine currentState={order.state} />
			</div>
			<div>
				<EventTimeline {events} />
			</div>
		</div>

		{#if saga}
			<SagaProgress {saga} />
		{/if}
	{/if}
</div>
