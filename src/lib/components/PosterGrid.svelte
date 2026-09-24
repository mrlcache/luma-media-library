<script lang="ts">
	import PosterCard from '$lib/components/PosterCard.svelte';
	import type { MediaItem } from '$lib/types';

	type Props = { items: MediaItem[] };
	let { items }: Props = $props();
</script>

<div class="poster-grid" role="list" aria-label="Media results">
	{#each items as item, index (item.id)}
		<div role="listitem" aria-setsize={items.length} aria-posinset={index + 1}>
			<PosterCard media={item} variant="catalog" />
		</div>
	{/each}
</div>

<style>
	.poster-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(100%, 146px), 1fr));
		gap: 18px;
		margin: -14px;
		padding: 14px 22px 36px 15px;
	}

	@media (max-width: 760px) {
		.poster-grid {
			gap: 15px 14px;
			padding-right: 17px;
		}
	}
</style>
