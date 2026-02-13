<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import Header from '$lib/components/Header.svelte';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Event Sourcing Demo</title>
</svelte:head>

<div class="min-h-screen bg-gray-50">
	<Header />
	<main class="mx-auto max-w-6xl px-6 py-8">
		{@render children()}
	</main>
</div>
