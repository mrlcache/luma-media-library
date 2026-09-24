<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import type { CatalogMedia } from '$lib/types';

	type Props = { items: CatalogMedia[] };
	let { items }: Props = $props();
</script>

<div class="catalog-grid" role="list" aria-label="Local media results">
	{#each items as item, index (item.id)}
		<article class="catalog-card" role="listitem" aria-setsize={items.length} aria-posinset={index + 1}>
			<div class="catalog-card__poster">
				{#if item.posterUrl}
					<img src={item.posterUrl} alt={`Poster for ${item.title}`} width="342" height="513" loading={index < 8 ? 'eager' : 'lazy'} decoding="async" />
				{:else}
					<div class="catalog-card__placeholder" aria-label="Poster unavailable"><Icon name={item.kind === 'series' ? 'tv' : 'film'} size={25} /></div>
				{/if}
			</div>
			<div class="catalog-card__copy">
				<strong title={item.title}>{item.title}</strong>
				<span>
					{#if item.year}{item.year}{:else if item.kind}{item.kind === 'series' ? 'Series' : 'Movie'}{:else}{item.extension.toUpperCase()} file{/if}
					{#if item.year && item.kind}<i aria-hidden="true">·</i> {item.kind === 'series' ? 'Series' : 'Movie'}{/if}
					{#if item.voteAverage}<i aria-hidden="true">·</i> {item.voteAverage.toFixed(1)}{/if}
				</span>
			</div>
		</article>
	{/each}
</div>

<style>
	.catalog-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 146px), 1fr)); gap: 18px; margin: -14px; padding: 14px 22px 36px 15px; }
	.catalog-card { min-width: 0; }
	.catalog-card__poster { overflow: hidden; aspect-ratio: 2 / 3; border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; background: var(--surface-2); box-shadow: 0 12px 28px rgba(0,0,0,0.22); }
	.catalog-card__poster img { display: block; width: 100%; height: 100%; object-fit: cover; }
	.catalog-card__placeholder { display: grid; place-items: center; width: 100%; height: 100%; color: var(--text-dim); background: linear-gradient(145deg, var(--surface-2), var(--surface-1)); }
	.catalog-card__copy { display: grid; gap: 3px; padding: 10px 2px 0; }
	.catalog-card__copy strong { overflow: hidden; color: var(--text-soft); font-size: 0.81rem; font-weight: 610; text-overflow: ellipsis; white-space: nowrap; }
	.catalog-card__copy span { overflow: hidden; color: var(--text-muted); font-size: 0.69rem; text-overflow: ellipsis; white-space: nowrap; }
	.catalog-card__copy i { padding: 0 3px; color: var(--text-dim); font-style: normal; }
	@media (max-width: 760px) { .catalog-grid { gap: 15px 14px; padding-right: 17px; } }
</style>
