<script lang="ts">
	import { page } from '$app/state';

	const links = [
		{ href: '/', label: 'Dashboard' },
		{ href: '/orders', label: 'Orders' },
		{ href: '/demo', label: 'Demo' },
		{ href: '/about', label: 'About' }
	];

	function isActive(href: string): boolean {
		if (href === '/') return page.url.pathname === '/';
		return page.url.pathname.startsWith(href);
	}
</script>

<header class="border-b border-gray-200 bg-white">
	<div class="mx-auto flex max-w-6xl items-center justify-between px-6 py-4">
		<a href="/" class="flex items-center gap-2 text-lg font-bold text-gray-900">
			<span class="rounded bg-gray-900 px-2 py-0.5 font-mono text-sm text-white">ES</span>
			Event Sourcing
		</a>
		<nav class="flex gap-1">
			{#each links as { href, label }}
				<a
					{href}
					class="rounded-md px-3 py-2 text-sm font-medium transition-colors {isActive(href)
						? 'bg-gray-100 text-gray-900'
						: 'text-gray-500 hover:bg-gray-50 hover:text-gray-900'}"
				>
					{label}
				</a>
			{/each}
		</nav>
	</div>
</header>
