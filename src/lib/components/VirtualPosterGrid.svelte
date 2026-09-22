<script lang="ts">
	import { onMount } from 'svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import type { MediaItem } from '$lib/types';

	type Props = { items: MediaItem[] };
	let { items }: Props = $props();

	let viewport: HTMLDivElement;
	let scrollTop = $state(0);
	let viewportHeight = $state(620);
	let columns = $state(5);
	const rowHeight = 342;
	const columnGap = 18;
	const minCardWidth = 146;
	const hoverClearance = 14;

	let totalRows = $derived(Math.ceil(items.length / columns));
	let startRow = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - 2));
	let endRow = $derived(Math.min(totalRows, Math.ceil((scrollTop + viewportHeight) / rowHeight) + 2));
	let visibleItems = $derived(items.slice(startRow * columns, endRow * columns));

	function updateMetrics() {
		if (!viewport) return;
		const width = viewport.clientWidth - hoverClearance * 2;
		columns = Math.max(2, Math.floor((width + columnGap) / (minCardWidth + columnGap)));
		viewportHeight = viewport.clientHeight - hoverClearance * 2;
	}

	function handleScroll(event: Event) {
		scrollTop = (event.currentTarget as HTMLDivElement).scrollTop;
	}

	onMount(() => {
		updateMetrics();
		const observer = new ResizeObserver(updateMetrics);
		observer.observe(viewport);
		return () => observer.disconnect();
	});
</script>

<div class="virtual-grid" bind:this={viewport} onscroll={handleScroll} aria-label="Media results">
	<div class="virtual-grid__track" style={`height: ${Math.max(totalRows * rowHeight, viewportHeight)}px`}>
		<div
			class="virtual-grid__window"
			style={`transform: translateY(${startRow * rowHeight}px); grid-template-columns: repeat(${columns}, minmax(0, 1fr));`}
			role="list"
			aria-label="Media results"
		>
			{#each visibleItems as item, index (item.id)}
				<div role="listitem" aria-setsize={items.length} aria-posinset={startRow * columns + index + 1}>
				<PosterCard media={item} variant="catalog" />
				</div>
			{/each}
		</div>
	</div>
</div>

<style>
	.virtual-grid { --hover-clearance: 14px; box-sizing: border-box; height: calc(min(69vh, 720px) + var(--hover-clearance) + var(--hover-clearance)); min-height: calc(410px + var(--hover-clearance) + var(--hover-clearance)); margin: calc(-1 * var(--hover-clearance)); padding: var(--hover-clearance); overflow: auto; overscroll-behavior: contain; scrollbar-gutter: stable; }
	.virtual-grid__track { position: relative; width: 100%; }
	.virtual-grid__window { position: absolute; top: 0; right: 0; left: 0; display: grid; gap: 18px; padding: 0 8px 36px 1px; }
	@media (max-width: 760px) { .virtual-grid { height: calc(min(70vh, 620px) + var(--hover-clearance) + var(--hover-clearance)); } .virtual-grid__window { gap: 15px 14px; padding-right: 3px; } }
</style>
