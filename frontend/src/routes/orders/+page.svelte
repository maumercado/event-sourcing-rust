<script lang="ts">
	import { goto } from '$app/navigation';
	import { listOrders } from '$lib/api/orders';
	import type { OrderResponse } from '$lib/api/types';
	import CreateOrderForm from '$lib/components/CreateOrderForm.svelte';
	import OrderCard from '$lib/components/OrderCard.svelte';

	let orders = $state<OrderResponse[]>([]);
	let loading = $state(true);
	let error = $state('');
	let showForm = $state(false);

	async function loadOrders() {
		loading = true;
		try {
			orders = await listOrders();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load orders';
		} finally {
			loading = false;
		}
	}

	function handleCreated(orderId: string) {
		showForm = false;
		goto(`/orders/${orderId}`);
	}

	$effect(() => {
		loadOrders();
	});
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<h1 class="text-2xl font-bold text-gray-900">Orders</h1>
		<button
			onclick={() => (showForm = !showForm)}
			class="rounded-md bg-gray-900 px-4 py-2 text-sm font-medium text-white hover:bg-gray-800"
		>
			{showForm ? 'Cancel' : 'Create New Order'}
		</button>
	</div>

	{#if showForm}
		<CreateOrderForm onCreated={handleCreated} />
	{/if}

	{#if error}
		<div class="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700">{error}</div>
	{/if}

	{#if loading}
		<p class="text-gray-500">Loading orders...</p>
	{:else if orders.length === 0}
		<div class="rounded-lg border border-gray-200 bg-white p-8 text-center">
			<p class="text-gray-500">No orders yet. Create one to get started!</p>
		</div>
	{:else}
		<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
			{#each orders as order (order.id)}
				<OrderCard {order} />
			{/each}
		</div>
	{/if}
</div>
