<script lang="ts">
	import { createOrder } from '$lib/api/orders';
	import type { OrderItemRequest } from '$lib/api/types';

	interface Props {
		onCreated?: (orderId: string) => void;
	}

	let { onCreated }: Props = $props();

	let items: { product_id: string; product_name: string; quantity: number; price_dollars: string }[] = $state([
		{ product_id: '', product_name: '', quantity: 1, price_dollars: '' }
	]);
	let submitting = $state(false);
	let error = $state('');

	function addRow() {
		items = [...items, { product_id: '', product_name: '', quantity: 1, price_dollars: '' }];
	}

	function removeRow(index: number) {
		items = items.filter((_, i) => i !== index);
	}

	function addSampleItems() {
		items = [
			{ product_id: 'SKU-WIDGET', product_name: 'Premium Widget', quantity: 2, price_dollars: '24.99' },
			{ product_id: 'SKU-GADGET', product_name: 'Super Gadget', quantity: 1, price_dollars: '49.99' }
		];
	}

	async function handleSubmit() {
		error = '';
		const orderItems: OrderItemRequest[] = items
			.filter((item) => item.product_id && item.product_name)
			.map((item) => ({
				product_id: item.product_id,
				product_name: item.product_name,
				quantity: item.quantity,
				unit_price_cents: Math.round(parseFloat(item.price_dollars || '0') * 100)
			}));

		if (orderItems.length === 0) {
			error = 'Add at least one item with a product ID and name.';
			return;
		}

		submitting = true;
		try {
			const result = await createOrder({ items: orderItems });
			onCreated?.(result.order_id);
			items = [{ product_id: '', product_name: '', quantity: 1, price_dollars: '' }];
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to create order';
		} finally {
			submitting = false;
		}
	}
</script>

<form onsubmit={handleSubmit} class="rounded-lg border border-gray-200 bg-white p-6">
	<div class="mb-4 flex items-center justify-between">
		<h3 class="text-lg font-semibold text-gray-900">Create New Order</h3>
		<button
			type="button"
			onclick={addSampleItems}
			class="text-sm text-blue-600 hover:text-blue-800"
		>
			Add Sample Items
		</button>
	</div>

	<div class="space-y-3">
		{#each items as item, i}
			<div class="flex items-center gap-2">
				<input
					bind:value={item.product_id}
					placeholder="SKU"
					class="w-28 rounded border border-gray-300 px-2 py-1.5 text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500 focus:outline-none"
				/>
				<input
					bind:value={item.product_name}
					placeholder="Product name"
					class="flex-1 rounded border border-gray-300 px-2 py-1.5 text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500 focus:outline-none"
				/>
				<input
					bind:value={item.quantity}
					type="number"
					min="1"
					class="w-16 rounded border border-gray-300 px-2 py-1.5 text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500 focus:outline-none"
				/>
				<div class="relative">
					<span class="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-sm text-gray-400">$</span>
					<input
						bind:value={item.price_dollars}
						placeholder="0.00"
						class="w-24 rounded border border-gray-300 py-1.5 pr-2 pl-5 text-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500 focus:outline-none"
					/>
				</div>
				{#if items.length > 1}
					<button
						type="button"
						onclick={() => removeRow(i)}
						class="text-red-400 hover:text-red-600"
						title="Remove item"
					>
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
						</svg>
					</button>
				{/if}
			</div>
		{/each}
	</div>

	<div class="mt-4 flex items-center justify-between">
		<button
			type="button"
			onclick={addRow}
			class="text-sm text-gray-600 hover:text-gray-900"
		>
			+ Add item
		</button>
		<button
			type="submit"
			disabled={submitting}
			class="rounded-md bg-gray-900 px-4 py-2 text-sm font-medium text-white hover:bg-gray-800 disabled:opacity-50"
		>
			{submitting ? 'Creating...' : 'Create Order'}
		</button>
	</div>

	{#if error}
		<p class="mt-3 text-sm text-red-600">{error}</p>
	{/if}
</form>
