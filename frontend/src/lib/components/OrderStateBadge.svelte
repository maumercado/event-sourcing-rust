<script lang="ts">
	import type { OrderState } from '$lib/api/types';

	interface Props {
		state: OrderState;
	}

	let { state }: Props = $props();

	const colors: Record<OrderState, string> = {
		Draft: 'bg-gray-100 text-gray-700',
		Reserved: 'bg-blue-100 text-blue-700',
		Processing: 'bg-amber-100 text-amber-700',
		Completed: 'bg-emerald-100 text-emerald-700',
		Cancelled: 'bg-red-100 text-red-700'
	};
</script>

<span class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium {colors[state] ?? 'bg-gray-100 text-gray-700'}">
	{state}
</span>
