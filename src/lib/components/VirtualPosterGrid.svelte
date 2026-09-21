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
	const rowHeight = 365;
	const columnGap = 24;
	const minCardWidth = 142;

	let totalRows = $derived(Math.ceil(items.length / columns));
	let startRow = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - 2));
	let endRow = $derived(Math.min(totalRows, Math.ceil((scrollTop + viewportHeight) / rowHeight) + 2));
	let visibleItems = $derived(items.slice(startRow * columns, endRow * columns));

	function updateMetrics() {
		if (!viewport) return;
		const width = viewport.clientWidth;
		columns = Math.max(2, Math.floor((width + columnGap) / (minCardWidth + columnGap)));
		viewportHeight = viewport.clientHeight;
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
					<PosterCard media={item} />
				</div>
			{/each}
		</div>
	</div>
</div>

<style>
	.virtual-grid { height: min(68vh, 700px); min-height: 410px; overflow: auto; overscroll-behavior: contain; scrollbar-gutter: stable; }
	.virtual-grid__track { position: relative; width: 100%; }
	.virtual-grid__window { position: absolute; top: 0; right: 0; left: 0; display: grid; gap: 28px 24px; padding: 0 10px 36px 2px; }
	@media (max-width: 760px) { .virtual-grid { height: min(70vh, 620px); } .virtual-grid__window { gap: 22px 14px; padding-right: 6px; } }
</style>
