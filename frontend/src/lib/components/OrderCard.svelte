<script lang="ts">
	import type { OrderResponse } from '$lib/api/types';
	import { formatCents } from '$lib/utils/money';
	import OrderStateBadge from './OrderStateBadge.svelte';

	interface Props {
		order: OrderResponse;
	}

	let { order }: Props = $props();

	function truncateId(id: string): string {
		return id.slice(0, 8);
	}
</script>

<a
	href="/orders/{order.id}"
	class="block rounded-lg border border-gray-200 bg-white p-5 shadow-sm transition-shadow hover:shadow-md"
>
	<div class="mb-3 flex items-center justify-between">
		<span class="font-mono text-sm text-gray-500" title={order.id}>{truncateId(order.id)}</span>
		<OrderStateBadge state={order.state} />
	</div>
	<div class="mb-2 text-sm text-gray-600">
		{order.items.length} item{order.items.length !== 1 ? 's' : ''}
	</div>
	<div class="text-lg font-semibold text-gray-900">{formatCents(order.total_cents)}</div>
</a>
