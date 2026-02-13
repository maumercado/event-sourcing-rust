<script lang="ts">
	import type { SagaStatusResponse } from '$lib/api/types';

	interface Props {
		saga: SagaStatusResponse;
	}

	let { saga }: Props = $props();

	const steps = [
		{ key: 'reserve_inventory', label: 'Reserve Inventory', serviceKey: 'reservation_id' as const },
		{ key: 'process_payment', label: 'Process Payment', serviceKey: 'payment_id' as const },
		{ key: 'create_shipment', label: 'Create Shipment', serviceKey: 'tracking_number' as const }
	];

	function stepStatus(stepKey: string): 'completed' | 'failed' | 'pending' {
		if (saga.completed_steps.includes(stepKey)) return 'completed';
		if (saga.failure_reason && !saga.completed_steps.includes(stepKey)) {
			const failedIndex = steps.findIndex((s) => !saga.completed_steps.includes(s.key));
			const thisIndex = steps.findIndex((s) => s.key === stepKey);
			if (thisIndex === failedIndex) return 'failed';
		}
		return 'pending';
	}

	function serviceValue(key: 'reservation_id' | 'payment_id' | 'tracking_number'): string | null {
		return saga[key];
	}
</script>

<div class="rounded-lg border border-gray-200 bg-white p-6">
	<div class="mb-4 flex items-center justify-between">
		<h3 class="text-lg font-semibold text-gray-900">Saga Progress</h3>
		<span
			class="rounded-full px-2.5 py-0.5 text-xs font-medium
			{saga.state === 'Completed' ? 'bg-emerald-100 text-emerald-700' :
			 saga.state === 'Failed' || saga.state === 'Compensated' ? 'bg-red-100 text-red-700' :
			 'bg-amber-100 text-amber-700'}"
		>
			{saga.state}
		</span>
	</div>

	<div class="flex items-start justify-between">
		{#each steps as step, i}
			{@const status = stepStatus(step.key)}
			{@const svcValue = serviceValue(step.serviceKey)}

			{#if i > 0}
				<!-- Connector line -->
				<div class="mt-4 flex-1 border-t-2 {status === 'completed' || stepStatus(steps[i-1].key) === 'completed' ? 'border-emerald-300' : 'border-gray-200'}"></div>
			{/if}

			<div class="flex flex-col items-center" style="min-width: 120px;">
				<!-- Circle -->
				<div
					class="flex h-8 w-8 items-center justify-center rounded-full text-sm
					{status === 'completed' ? 'bg-emerald-100 text-emerald-600' :
					 status === 'failed' ? 'bg-red-100 text-red-600' :
					 'bg-gray-100 text-gray-400'}"
				>
					{#if status === 'completed'}
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
						</svg>
					{:else if status === 'failed'}
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
						</svg>
					{:else}
						{i + 1}
					{/if}
				</div>
				<!-- Label -->
				<p class="mt-2 text-center text-xs font-medium text-gray-700">{step.label}</p>
				<!-- Service value -->
				{#if svcValue}
					<p class="mt-1 font-mono text-xs text-gray-400" title={svcValue}>
						{svcValue.slice(0, 8)}...
					</p>
				{/if}
			</div>
		{/each}
	</div>

	{#if saga.failure_reason}
		<div class="mt-4 rounded bg-red-50 p-3 text-sm text-red-700">
			{saga.failure_reason}
		</div>
	{/if}
</div>
