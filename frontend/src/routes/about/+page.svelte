<div class="mx-auto max-w-3xl space-y-8">
	<div>
		<h1 class="text-2xl font-bold text-gray-900">Architecture</h1>
		<p class="mt-2 text-gray-600">
			This project demonstrates three key distributed systems patterns implemented in Rust.
		</p>
	</div>

	<!-- Event Sourcing -->
	<section class="rounded-lg border border-gray-200 bg-white p-6">
		<h2 class="text-xl font-semibold text-gray-900">Event Sourcing</h2>
		<p class="mt-2 text-sm text-gray-700">
			Instead of storing the current state of an entity, every state change is captured as an
			<strong>immutable event</strong>. The current state is derived by replaying all events from
			the beginning.
		</p>
		<div class="mt-4 rounded bg-gray-50 p-4">
			<div class="flex items-center gap-3 text-sm">
				<span class="rounded bg-emerald-100 px-2 py-1 font-mono text-emerald-700">OrderCreated</span>
				<span class="text-gray-400">&rarr;</span>
				<span class="rounded bg-blue-100 px-2 py-1 font-mono text-blue-700">ItemAdded</span>
				<span class="text-gray-400">&rarr;</span>
				<span class="rounded bg-amber-100 px-2 py-1 font-mono text-amber-700">OrderSubmitted</span>
				<span class="text-gray-400">&rarr;</span>
				<span class="rounded bg-emerald-100 px-2 py-1 font-mono text-emerald-700">OrderCompleted</span>
			</div>
		</div>
		<ul class="mt-4 space-y-1 text-sm text-gray-600">
			<li><strong>Append-only:</strong> Events are never updated or deleted</li>
			<li><strong>Full audit trail:</strong> Complete history of every change</li>
			<li><strong>Temporal queries:</strong> Rebuild state at any point in time</li>
			<li><strong>Optimistic concurrency:</strong> Version numbers prevent conflicting writes</li>
		</ul>
	</section>

	<!-- CQRS -->
	<section class="rounded-lg border border-gray-200 bg-white p-6">
		<h2 class="text-xl font-semibold text-gray-900">CQRS (Command Query Responsibility Segregation)</h2>
		<p class="mt-2 text-sm text-gray-700">
			The system separates the <strong>write side</strong> (commands that emit events) from the
			<strong>read side</strong> (projections that build query-optimized views from events).
		</p>
		<div class="mt-4 grid gap-4 sm:grid-cols-2">
			<div class="rounded border border-blue-200 bg-blue-50 p-4">
				<h3 class="text-sm font-semibold text-blue-900">Command Side (Write)</h3>
				<ul class="mt-2 space-y-1 text-xs text-blue-800">
					<li>Order Aggregate with business rules</li>
					<li>Command Handler (load, execute, persist)</li>
					<li>Emits domain events</li>
				</ul>
			</div>
			<div class="rounded border border-emerald-200 bg-emerald-50 p-4">
				<h3 class="text-sm font-semibold text-emerald-900">Query Side (Read)</h3>
				<ul class="mt-2 space-y-1 text-xs text-emerald-800">
					<li>Denormalized read models</li>
					<li>Projection Processor subscribes to events</li>
					<li>Eventually consistent views</li>
				</ul>
			</div>
		</div>
	</section>

	<!-- Saga Pattern -->
	<section class="rounded-lg border border-gray-200 bg-white p-6">
		<h2 class="text-xl font-semibold text-gray-900">Saga Pattern</h2>
		<p class="mt-2 text-sm text-gray-700">
			Coordinates multi-step distributed transactions. Each step calls an external service,
			and if any step fails, <strong>compensating transactions</strong> undo previous steps
			in reverse order.
		</p>
		<div class="mt-4 rounded bg-gray-50 p-4">
			<div class="flex flex-col gap-2 text-sm">
				<div class="flex items-center gap-3">
					<span class="w-6 text-center font-bold text-gray-400">1</span>
					<span class="text-gray-700">Reserve Inventory</span>
					<span class="text-xs text-gray-400">&rarr; InventoryService</span>
				</div>
				<div class="flex items-center gap-3">
					<span class="w-6 text-center font-bold text-gray-400">2</span>
					<span class="text-gray-700">Process Payment</span>
					<span class="text-xs text-gray-400">&rarr; PaymentService</span>
				</div>
				<div class="flex items-center gap-3">
					<span class="w-6 text-center font-bold text-gray-400">3</span>
					<span class="text-gray-700">Create Shipment</span>
					<span class="text-xs text-gray-400">&rarr; ShippingService</span>
				</div>
			</div>
		</div>
		<p class="mt-3 text-sm text-gray-600">
			If step 3 fails, the system automatically refunds payment (step 2) and releases inventory (step 1).
		</p>
	</section>

	<!-- Storage -->
	<section class="rounded-lg border border-gray-200 bg-white p-6">
		<h2 class="text-xl font-semibold text-gray-900">Event Store Backends</h2>
		<p class="mt-2 text-sm text-gray-700">
			The API layer is generic over the <code class="rounded bg-gray-100 px-1 font-mono text-xs">EventStore</code> trait.
			The backend selects the implementation at startup based on configuration.
		</p>
		<div class="mt-4 grid gap-4 sm:grid-cols-2">
			<div class="rounded border border-amber-200 bg-amber-50 p-4">
				<h3 class="text-sm font-semibold text-amber-900">In-Memory (this demo)</h3>
				<ul class="mt-2 space-y-1 text-xs text-amber-800">
					<li>Default when no <code class="font-mono">DATABASE_URL</code> is set</li>
					<li>All data lives in memory &mdash; resets on server restart</li>
					<li>Great for development and demos</li>
				</ul>
			</div>
			<div class="rounded border border-blue-200 bg-blue-50 p-4">
				<h3 class="text-sm font-semibold text-blue-900">PostgreSQL (production)</h3>
				<ul class="mt-2 space-y-1 text-xs text-blue-800">
					<li>Activated by setting <code class="font-mono">DATABASE_URL</code> env var</li>
					<li>Events persisted in Postgres with migrations</li>
					<li>Connection pooling via SQLx</li>
				</ul>
			</div>
		</div>
	</section>

	<!-- Tech Stack -->
	<section class="rounded-lg border border-gray-200 bg-white p-6">
		<h2 class="text-xl font-semibold text-gray-900">Tech Stack</h2>
		<div class="mt-4 grid gap-3 sm:grid-cols-2">
			<div>
				<h3 class="text-sm font-semibold text-gray-700">Backend</h3>
				<ul class="mt-1 space-y-1 text-sm text-gray-600">
					<li>Rust with Tokio async runtime</li>
					<li>Axum HTTP framework</li>
					<li>PostgreSQL or in-memory event store</li>
					<li>Tracing + Prometheus metrics</li>
				</ul>
			</div>
			<div>
				<h3 class="text-sm font-semibold text-gray-700">Frontend</h3>
				<ul class="mt-1 space-y-1 text-sm text-gray-600">
					<li>SvelteKit with TypeScript</li>
					<li>Tailwind CSS v4</li>
					<li>Static SPA deployment</li>
				</ul>
			</div>
		</div>
	</section>
</div>
