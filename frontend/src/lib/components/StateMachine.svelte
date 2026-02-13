<script lang="ts">
	import type { OrderState } from '$lib/api/types';

	interface Props {
		currentState: OrderState;
	}

	let { currentState }: Props = $props();

	const states: OrderState[] = ['Draft', 'Reserved', 'Processing', 'Completed'];
	const stateOrder: Record<OrderState, number> = {
		Draft: 0,
		Reserved: 1,
		Processing: 2,
		Completed: 3,
		Cancelled: -1
	};

	const isCancelled = $derived(currentState === 'Cancelled');
	const currentIndex = $derived(stateOrder[currentState]);

	function stateColor(state: OrderState): string {
		const index = stateOrder[state];
		if (isCancelled) return 'text-gray-400 border-gray-200 bg-gray-50';
		if (index < currentIndex) return 'text-emerald-700 border-emerald-300 bg-emerald-50';
		if (index === currentIndex) return 'text-white border-transparent bg-gray-900';
		return 'text-gray-400 border-gray-200 bg-white';
	}

	function arrowColor(fromIndex: number): string {
		if (isCancelled) return 'text-gray-200';
		if (fromIndex < currentIndex) return 'text-emerald-400';
		return 'text-gray-200';
	}
</script>

<div class="rounded-lg border border-gray-200 bg-white p-6">
	<h3 class="mb-4 text-lg font-semibold text-gray-900">State Machine</h3>

	<div class="flex items-center justify-center gap-2">
		{#each states as state, i}
			{#if i > 0}
				<!-- Arrow -->
				<svg class="h-4 w-6 flex-shrink-0 {arrowColor(i - 1)}" fill="none" stroke="currentColor" viewBox="0 0 24 16">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2 8h16m-4-4l4 4-4 4" />
				</svg>
			{/if}
			<div
				class="flex-shrink-0 rounded-lg border-2 px-4 py-2 text-center text-sm font-medium {stateColor(state)}"
			>
				{state}
			</div>
		{/each}
	</div>

	{#if isCancelled}
		<div class="mt-4 flex items-center justify-center">
			<div class="rounded-lg border-2 border-red-300 bg-red-50 px-4 py-2 text-sm font-medium text-red-700">
				Cancelled
			</div>
		</div>
		<p class="mt-2 text-center text-xs text-gray-400">
			Order was cancelled from a non-terminal state
		</p>
	{/if}
</div>
