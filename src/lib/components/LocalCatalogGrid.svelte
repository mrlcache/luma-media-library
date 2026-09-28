<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { recoverRemoteArtwork, tmdbImageSize } from '$lib/media/artwork';
	import { usePlayer } from '$lib/player-context';
	import type { CatalogMedia } from '$lib/types';

	type Props = { items: CatalogMedia[] };
	let { items }: Props = $props();
	const player = usePlayer();

	function play(item: CatalogMedia) {
		player.open({
			id: String(item.id),
			title: item.title,
			kind: item.kind === 'series' ? 'series' : 'movie',
			year: item.year ?? 0,
			genres: [],
			rating: item.voteAverage?.toFixed(1) ?? '',
			runtime: '',
			poster: tmdbImageSize(item.posterUrl, 'w780'),
			backdrop: tmdbImageSize(item.backdropUrl, 'w1280'),
			synopsis: item.overview ?? '',
			match: ''
		});
	}
</script>

<div class="catalog-grid" role="list" aria-label="Local media results">
	{#each items as item, index (item.id)}
		<article class="catalog-card" role="listitem" aria-setsize={items.length} aria-posinset={index + 1}>
			<div class="catalog-card__poster">
				<a class="catalog-card__poster-link" href={`/title/${item.id}`} aria-label={`Details for ${item.title}`}>
					{#if item.posterUrl}
						<img use:recoverRemoteArtwork src={tmdbImageSize(item.posterUrl, 'w780')} alt={`Poster for ${item.title}`} width="780" height="1170" loading={index < 8 ? 'eager' : 'lazy'} decoding="async" />
					{:else}
						<span class="catalog-card__placeholder" aria-hidden="true"><Icon name={item.kind === 'series' ? 'tv' : 'film'} size={25} /></span>
					{/if}
				</a>
				<button class="catalog-card__play" type="button" aria-label={`Play ${item.title}`} title={`Play ${item.title}`} onclick={() => play(item)}>
					<Icon name="play" size={16} weight="fill" />
				</button>
			</div>
			<div class="catalog-card__copy">
				<a class="catalog-card__title" href={`/title/${item.id}`} title={item.title}>{item.title}</a>
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
	.catalog-card { min-width: 0; transform-origin: center bottom; transition: transform 190ms cubic-bezier(0.2, 0.72, 0.2, 1); }
	.catalog-card__poster { position: relative; overflow: hidden; aspect-ratio: 2 / 3; border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; background: var(--surface-2); box-shadow: 0 12px 28px rgba(0,0,0,0.22); transition: border-color 180ms ease, box-shadow 180ms ease; }
	.catalog-card__poster-link { position: absolute; inset: 0; display: block; overflow: hidden; border-radius: inherit; color: inherit; text-decoration: none; }
	.catalog-card__poster img { display: block; width: 100%; height: 100%; object-fit: cover; transition: transform 260ms ease, filter 260ms ease; }
	.catalog-card__placeholder { display: grid; place-items: center; width: 100%; height: 100%; color: var(--text-dim); background: linear-gradient(145deg, var(--surface-2), var(--surface-1)); }
	.catalog-card__play { position: absolute; z-index: 2; right: 11px; bottom: 11px; display: grid; width: 36px; height: 36px; place-items: center; border: 1px solid rgba(255,255,255,0.22); border-radius: 50%; color: #101317; background: rgba(245,247,249,0.94); box-shadow: 0 8px 24px rgba(0,0,0,0.3); cursor: pointer; opacity: 0; transform: translateY(4px); transition: opacity 160ms ease, transform 160ms ease; }
	.catalog-card:hover .catalog-card__play, .catalog-card:focus-within .catalog-card__play { opacity: 1; transform: translateY(0); }
	.catalog-card:hover, .catalog-card:focus-within { transform: translateY(-3px) scale(1.018); }
	.catalog-card:hover .catalog-card__poster, .catalog-card:focus-within .catalog-card__poster { border-color: rgba(255,255,255,0.34); box-shadow: 0 0 0 2px rgba(255,255,255,0.34), 0 18px 36px rgba(0,0,0,0.34); }
	.catalog-card:hover .catalog-card__poster img, .catalog-card:focus-within .catalog-card__poster img { filter: brightness(1.05); transform: scale(1.012); }
	.catalog-card__play:hover { background: #fff; }
	.catalog-card__copy { display: grid; gap: 3px; padding: 10px 2px 0; }
	.catalog-card__title { overflow: hidden; color: var(--text-soft); font-size: 0.81rem; font-weight: 610; text-decoration: none; text-overflow: ellipsis; white-space: nowrap; }
	.catalog-card__title:hover { color: var(--text-strong); }
	.catalog-card__copy span { overflow: hidden; color: var(--text-muted); font-size: 0.69rem; text-overflow: ellipsis; white-space: nowrap; }
	.catalog-card__copy i { padding: 0 3px; color: var(--text-dim); font-style: normal; }
	@media (max-width: 760px) { .catalog-grid { gap: 15px 14px; padding-right: 17px; } }
	@media (hover: none) { .catalog-card__play { opacity: 1; transform: none; } }
	@media (prefers-reduced-motion: reduce) { .catalog-card, .catalog-card__poster, .catalog-card__poster img, .catalog-card__play { transition: none; } .catalog-card:hover, .catalog-card:focus-within { transform: none; } }
</style>
