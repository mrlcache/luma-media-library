<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { usePlayer } from '$lib/player-context';
	import type { MediaItem } from '$lib/types';

	type Props = { media: MediaItem; priority?: boolean; variant?: 'default' | 'home' };
	let { media, priority = false, variant = 'default' }: Props = $props();
	const player = usePlayer();
</script>

<article class:poster-card--home={variant === 'home'} class="poster-card">
	<a class="poster-card__link" href={`/title/${media.id}`} aria-label={`${media.title}, ${media.kind}, ${media.year}`}>
		<div class="poster-card__art">
			<img
				src={media.poster}
				alt={`Poster for ${media.title}`}
				width="520"
				height="780"
				loading={priority ? 'eager' : 'lazy'}
				decoding="async"
			/>
			<div class="poster-card__scrim"></div>
			{#if media.progress}
				<div class="poster-card__progress" aria-label={`${Math.round(media.progress * 100)} percent watched`}>
					<span style={`--progress: ${media.progress * 100}%`}></span>
				</div>
			{/if}
			<span class="poster-card__kind">{media.kind === 'series' ? 'Series' : 'Movie'}</span>
		</div>
		<div class="poster-card__copy">
			<strong>{media.title}</strong>
			<span>{media.year} <i aria-hidden="true">·</i> {media.genres[0]}</span>
		</div>
	</a>
	<button class="poster-card__play" aria-label={`Play ${media.title}`} title={`Play ${media.title}`} onclick={() => player.open(media)}>
		<Icon name="play" size={15} weight="fill" />
	</button>
</article>

<style>
	.poster-card { position: relative; min-width: 0; }
	.poster-card__link { display: block; color: inherit; text-decoration: none; }
	.poster-card__art { position: relative; overflow: hidden; aspect-ratio: 2 / 3; border-radius: var(--radius-md); background: var(--surface-2); box-shadow: 0 12px 24px rgba(0, 0, 0, 0.14); }
	.poster-card__art img { display: block; width: 100%; height: 100%; object-fit: cover; transition: transform 260ms ease, filter 260ms ease; }
	.poster-card__scrim { position: absolute; inset: 0; background: linear-gradient(180deg, transparent 50%, rgba(5, 7, 9, 0.74) 100%); opacity: 0.72; transition: opacity 220ms ease; }
	.poster-card__kind { position: absolute; top: 10px; left: 10px; padding: 4px 7px; border: 1px solid rgba(255, 255, 255, 0.14); border-radius: 5px; color: rgba(255, 255, 255, 0.82); font-size: 0.6rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; background: rgba(9, 11, 14, 0.58); backdrop-filter: blur(8px); }
	.poster-card__copy { display: grid; gap: 3px; padding: 11px 2px 0; }
	.poster-card__copy strong { overflow: hidden; color: var(--text-soft); font-size: 0.82rem; font-weight: 620; letter-spacing: -0.01em; text-overflow: ellipsis; white-space: nowrap; }
	.poster-card__copy span { overflow: hidden; color: var(--text-muted); font-size: 0.7rem; text-overflow: ellipsis; white-space: nowrap; }
	.poster-card__copy i { color: var(--text-dim); font-style: normal; }
	.poster-card__progress { position: absolute; right: 10px; bottom: 10px; left: 10px; height: 3px; overflow: hidden; border-radius: 2px; background: rgba(255, 255, 255, 0.24); }
	.poster-card__progress span { display: block; width: var(--progress); height: 100%; background: var(--accent); }
	.poster-card__play { position: absolute; right: 10px; bottom: 58px; display: grid; place-items: center; width: 34px; height: 34px; padding: 0; border: 1px solid rgba(255, 255, 255, 0.2); border-radius: 50%; color: var(--surface-0); background: var(--accent-soft); cursor: pointer; opacity: 0; transform: translateY(5px); transition: opacity 180ms ease, transform 180ms ease, background-color 180ms ease; }
	.poster-card:hover .poster-card__play, .poster-card:focus-within .poster-card__play { opacity: 1; transform: translateY(0); }
	.poster-card:hover .poster-card__art img, .poster-card:focus-within .poster-card__art img { filter: saturate(1.05); transform: scale(1.035); }
	.poster-card:hover .poster-card__scrim, .poster-card:focus-within .poster-card__scrim { opacity: 0.94; }
	.poster-card__play:hover { background: #ffffff; }
	.poster-card--home { transform-origin: center bottom; transition: transform 190ms cubic-bezier(0.2, 0.72, 0.2, 1); }
	.poster-card--home .poster-card__art { border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; box-shadow: 0 12px 28px rgba(0,0,0,0.22); transition: border-color 180ms ease, box-shadow 180ms ease; }
	.poster-card--home .poster-card__scrim, .poster-card--home .poster-card__kind { display: none; }
	.poster-card--home .poster-card__copy { padding: 10px 2px 0; }
	.poster-card--home .poster-card__copy strong { color: var(--text-soft); font-size: 0.81rem; font-weight: 610; }
	.poster-card--home .poster-card__copy span { font-size: 0.69rem; }
	.poster-card--home .poster-card__play { right: 10px; bottom: 50px; border: 0; border-radius: 50%; color: var(--surface-0); background: var(--text-strong); transition-duration: 150ms; }
	.poster-card--home:hover, .poster-card--home:focus-within { transform: translateY(-3px) scale(1.022); }
	.poster-card--home:hover .poster-card__art, .poster-card--home:focus-within .poster-card__art { border-color: rgba(255,255,255,0.2); box-shadow: 0 18px 36px rgba(0,0,0,0.34); }
	.poster-card--home:hover .poster-card__art img, .poster-card--home:focus-within .poster-card__art img { filter: brightness(1.05) saturate(1.03); transform: scale(1.012); }
	@media (max-width: 680px) { .poster-card--home .poster-card__play { display: none; } }
	@media (prefers-reduced-motion: reduce) { .poster-card__art img, .poster-card__scrim, .poster-card__play { transition: none; } }
</style>
