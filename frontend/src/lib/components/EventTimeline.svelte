<script lang="ts">
	import type { EventEnvelopeResponse } from '$lib/api/types';

	interface Props {
		events: EventEnvelopeResponse[];
	}

	let { events }: Props = $props();

	let expandedId: string | null = $state(null);

	function toggleExpand(eventId: string) {
		expandedId = expandedId === eventId ? null : eventId;
	}

	function eventColor(eventType: string): string {
		if (eventType.includes('Created')) return 'bg-emerald-500';
		if (eventType.includes('Completed') || eventType.includes('Shipped')) return 'bg-emerald-500';
		if (eventType.includes('Cancelled') || eventType.includes('Failed')) return 'bg-red-500';
		if (eventType.includes('Submitted') || eventType.includes('Reserved') || eventType.includes('Processing'))
			return 'bg-amber-500';
		return 'bg-blue-500';
	}

	function relativeTime(timestamp: string): string {
		const diff = Date.now() - new Date(timestamp).getTime();
		const seconds = Math.floor(diff / 1000);
		if (seconds < 60) return `${seconds}s ago`;
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes}m ago`;
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return `${hours}h ago`;
		return new Date(timestamp).toLocaleDateString();
	}

	function formatPayload(payload: Record<string, unknown>): string {
		return JSON.stringify(payload, null, 2);
	}
</script>

<div class="rounded-lg border border-gray-200 bg-white p-6">
	<h3 class="mb-4 text-lg font-semibold text-gray-900">Event Timeline</h3>

	{#if events.length === 0}
		<p class="text-sm text-gray-500">No events yet.</p>
	{:else}
		<div class="relative ml-4">
			<!-- Vertical line -->
			<div class="absolute top-0 bottom-0 left-0 w-0.5 bg-gray-200"></div>

			{#each events as event}
				<div class="relative mb-6 pl-8 last:mb-0">
					<!-- Dot -->
					<div class="absolute top-1 left-0 -translate-x-1/2">
						<div class="h-3 w-3 rounded-full ring-4 ring-white {eventColor(event.event_type)}"></div>
					</div>

					<!-- Content -->
					<div>
						<div class="flex items-center gap-2">
							<button
								onclick={() => toggleExpand(event.event_id)}
								class="text-sm font-semibold text-gray-900 hover:text-blue-600"
							>
								{event.event_type}
							</button>
							<span class="rounded bg-gray-100 px-1.5 py-0.5 font-mono text-xs text-gray-500">
								v{event.version}
							</span>
						</div>
						<p class="mt-0.5 text-xs text-gray-400" title={event.timestamp}>
							{relativeTime(event.timestamp)}
						</p>

						{#if expandedId === event.event_id}
							<pre class="mt-2 overflow-x-auto rounded bg-gray-50 p-3 font-mono text-xs text-gray-700">{formatPayload(event.payload)}</pre>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
