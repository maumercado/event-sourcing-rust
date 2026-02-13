<script lang="ts">
	import { checkHealth, listOrders } from '$lib/api/orders';
	import { seedDemoData } from '$lib/api/seed';

	let orderCount = $state<number | null>(null);
	let healthOk = $state<boolean | null>(null);
	let seeding = $state(false);
	let seedProgress = $state('');

	async function loadData() {
		try {
			const orders = await listOrders();
			orderCount = orders.length;
		} catch {
			orderCount = null;
		}
		try {
			const health = await checkHealth();
			healthOk = health.status === 'ok';
		} catch {
			healthOk = false;
		}
	}

	async function handleSeed() {
		seeding = true;
		try {
			await seedDemoData((step, total, label) => {
				seedProgress = `${label} (${step}/${total})`;
			});
			seedProgress = 'Done!';
			await loadData();
			setTimeout(() => {
				seedProgress = '';
			}, 2000);
		} catch (e) {
			seedProgress = `Error: ${e instanceof Error ? e.message : 'Unknown error'}`;
		} finally {
			seeding = false;
		}
	}

	$effect(() => {
		loadData();
	});

	const concepts = [
		{
			title: 'Event Sourcing',
			description: 'All state changes are stored as immutable events. The current state is derived by replaying events from the beginning.',
			color: 'border-emerald-200 bg-emerald-50'
		},
		{
			title: 'CQRS',
			description: 'Command side handles writes and emits events. Query side maintains denormalized read models updated from events.',
			color: 'border-blue-200 bg-blue-50'
		},
		{
			title: 'Saga Pattern',
			description: 'Multi-step distributed transactions with compensating actions on failure. Orchestrates inventory, payment, and shipping.',
			color: 'border-amber-200 bg-amber-50'
		}
	];
</script>

<div class="space-y-8">
	<!-- Hero -->
	<div class="text-center">
		<h1 class="text-3xl font-bold text-gray-900">Event Sourcing in Rust</h1>
		<p class="mt-2 text-gray-600">
			Interactive demo of Event Sourcing, CQRS, and the Saga Pattern
		</p>
		<p class="mt-1 text-xs text-gray-400">
			This demo runs with an in-memory event store. Data resets when the server restarts.
		</p>
		<div class="mt-6 flex items-center justify-center gap-4">
			<a
				href="/demo"
				class="rounded-md bg-gray-900 px-6 py-2.5 text-sm font-medium text-white hover:bg-gray-800"
			>
				Try Guided Demo
			</a>
			<button
				onclick={handleSeed}
				disabled={seeding}
				class="rounded-md border border-gray-300 bg-white px-6 py-2.5 text-sm font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50"
			>
				{seeding ? 'Seeding...' : 'Seed Demo Data'}
			</button>
		</div>
		{#if seedProgress}
			<p class="mt-2 text-sm text-gray-500">{seedProgress}</p>
		{/if}
	</div>

	<!-- Stats -->
	<div class="flex justify-center gap-6">
		<div class="rounded-lg border border-gray-200 bg-white px-6 py-4 text-center">
			<p class="text-2xl font-bold text-gray-900">{orderCount ?? '--'}</p>
			<p class="text-sm text-gray-500">Active Orders</p>
		</div>
		<div class="rounded-lg border border-gray-200 bg-white px-6 py-4 text-center">
			<div class="flex items-center justify-center gap-2">
				{#if healthOk === true}
					<span class="h-2.5 w-2.5 rounded-full bg-emerald-500"></span>
					<span class="text-sm font-medium text-emerald-700">Healthy</span>
				{:else if healthOk === false}
					<span class="h-2.5 w-2.5 rounded-full bg-red-500"></span>
					<span class="text-sm font-medium text-red-700">Offline</span>
				{:else}
					<span class="h-2.5 w-2.5 rounded-full bg-gray-300"></span>
					<span class="text-sm font-medium text-gray-500">Checking...</span>
				{/if}
			</div>
			<p class="mt-1 text-sm text-gray-500">API Status</p>
		</div>
	</div>

	<!-- Concepts -->
	<div class="grid gap-6 md:grid-cols-3">
		{#each concepts as concept}
			<div class="rounded-lg border p-6 {concept.color}">
				<h3 class="text-lg font-semibold text-gray-900">{concept.title}</h3>
				<p class="mt-2 text-sm text-gray-700">{concept.description}</p>
			</div>
		{/each}
	</div>
</div>
