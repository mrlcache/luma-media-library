<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import FavoriteButton from './FavoriteButton.svelte';
	import { isMobilePreview } from '$lib/platform/mobile-preview';
	import { recoverRemoteArtwork, tmdbImageSize } from '$lib/media/artwork';
	import { usePlayer } from '$lib/player-context';
	import { smoothHorizontalScroll } from '$lib/scroll/lenis';
	import { rightEdgeHint } from '$lib/scroll/right-edge-hint';
	import type { MediaItem } from '$lib/types';

	type Props = { title: string; description?: string; items: MediaItem[]; showProgress?: boolean };
	let { title, description, items, showProgress = false }: Props = $props();
	const player = usePlayer();
	const mobilePreview = isMobilePreview();
</script>

<section class="media-row" aria-labelledby={`row-${title.toLowerCase().replaceAll(' ', '-')}`}>
	<div class="section-heading">
		<div>
			<h2 id={`row-${title.toLowerCase().replaceAll(' ', '-')}`}>{title}</h2>
			{#if description}<p>{description}</p>{/if}
		</div>
	</div>
	<div class="media-row__viewport" data-more-right="false">
		<div class="media-row__track" role="list" use:smoothHorizontalScroll use:rightEdgeHint>
			{#each items as media, index (media.id)}
				<article class="landscape-card" role="listitem">
					<div class="landscape-card__art" style="corner-shape: squircle">
						<div class="landscape-card__selection-halo" style="corner-shape: squircle"></div>
						<a class="landscape-card__media" href={`/title/${media.id}`} aria-label={`${media.title}, ${media.kind}`} style="corner-shape: squircle">
							{#if media.backdrop || media.poster}<img use:recoverRemoteArtwork src={mobilePreview ? tmdbImageSize(media.backdrop || media.poster, 'w500') : media.backdrop || media.poster} alt="" width="720" height="405" loading={index < 2 ? 'eager' : 'lazy'} decoding="async" />{/if}
							<div class="landscape-card__scrim"></div>
							{#if media.installed === false}<span class="landscape-card__availability" role="img" aria-label="Not downloaded" ><Icon name="download" size={13} /></span>{/if}
							{#if showProgress && media.progress}
								<div class="landscape-card__progress"><span style={`--progress: ${media.progress * 100}%`}></span></div>
							{/if}
						</a>
						{#if mobilePreview}<div class="landscape-card__favorite"><FavoriteButton {media} compact /></div>{/if}
						{#if media.installed !== false}<button class="landscape-card__play" aria-label={`Play ${media.title}`} onclick={() => player.open(media)}><Icon name="play" size={14} weight="fill" /></button>{/if}
					</div>
					<a class="landscape-card__link" href={`/title/${media.id}`} aria-label={`${media.title}, ${media.kind}`}>
						<div class="landscape-card__copy">
							<strong>{media.title}</strong>
							<span>{media.progressLabel ?? `${media.year} · ${media.genres[0]}`}</span>
						</div>
					</a>
				</article>
			{/each}
		</div>
	</div>
</section>

<style>
	.landscape-card__favorite { position:absolute; z-index:3; left:9px; top:9px; }
	.media-row { display: grid; grid-template-columns: minmax(0, 1fr); min-width: 0; gap: 18px; }
	.landscape-card__availability { position:absolute; right:9px; top:9px; display:grid; place-items:center; width:24px; height:24px; border:1px solid rgba(255,255,255,.14); border-radius:50%; color:rgba(255,255,255,.85); background:rgba(8,12,17,.65); backdrop-filter:blur(8px); }
	.section-heading { display: flex; align-items: end; justify-content: space-between; gap: 16px; }
	.section-heading h2 { margin: 0; color: var(--text-strong); font-family: var(--font-display); font-size: 1.08rem; font-weight: 690; letter-spacing: -0.034em; }
	.section-heading p { margin: 4px 0 0; color: var(--text-muted); font-size: 0.73rem; }
	.media-row__viewport { --hover-clearance: 14px; --hover-inline-clearance: 14px; min-width: 0; margin: calc(-1 * var(--hover-clearance)) calc(-1 * var(--hover-inline-clearance)); }
	.media-row__track { --hover-clearance: 14px; --hover-inline-clearance: 14px; display: grid; grid-auto-columns: clamp(250px, 24vw, 360px); grid-auto-flow: column; gap: 16px; overflow-x: auto; overflow-y: hidden; padding: calc(4px + var(--hover-clearance)) calc(3px + var(--hover-inline-clearance)) calc(16px + var(--hover-clearance)); scroll-padding-inline: calc(3px + var(--hover-inline-clearance)); scrollbar-width: none; overscroll-behavior-x: none; }
	:global(.media-row__viewport[data-more-right="true"] .media-row__track) { -webkit-mask-image: linear-gradient(90deg, #000 calc(100% - 26px), transparent calc(100% - 12px)); mask-image: linear-gradient(90deg, #000 calc(100% - 26px), transparent calc(100% - 12px)); }
	.media-row__track::-webkit-scrollbar { display: none; }
	.landscape-card { --art-radius: 12px; position: relative; min-width: 0; transform-origin: center bottom; transition: transform 190ms cubic-bezier(0.2, 0.72, 0.2, 1); }
	.landscape-card__link { display: block; color: inherit; text-decoration: none; }
	.landscape-card__art { position: relative; isolation: isolate; aspect-ratio: 16 / 9; border-radius: var(--art-radius); background: transparent; transition: box-shadow 180ms ease; }
	.landscape-card__selection-halo { position: absolute; z-index: 2; inset: -2px; box-sizing: border-box; border: 2px solid transparent; border-radius: calc(var(--art-radius) + 2px); pointer-events: none; opacity: 0; transition: border-color 180ms ease, opacity 180ms ease; }
	.landscape-card__media { position: absolute; z-index: 1; inset: 0; display: block; isolation: isolate; overflow: hidden; border-radius: var(--art-radius); color: inherit; text-decoration: none; }
	.landscape-card__art img,
	.landscape-card__scrim { display: block; width: 100%; height: 100%; border-radius: var(--art-radius); }
	.landscape-card__art img { object-fit: cover; transition: filter 180ms ease, transform 260ms ease; }
	.landscape-card__scrim { position: absolute; inset: 0; pointer-events: none; }
	.landscape-card__progress { position: absolute; right: 8px; bottom: 8px; left: 8px; height: 3px; overflow: hidden; border-radius: 2px; background: rgba(255, 255, 255, 0.24); }
	.landscape-card__progress span { display: block; width: var(--progress); height: 100%; background: var(--accent); }
	.landscape-card__copy { display: grid; gap: 3px; padding: 10px 2px 0; }
	.landscape-card__copy strong { overflow: hidden; color: var(--text-soft); font-size: 0.8rem; font-weight: 650; letter-spacing: -0.018em; text-overflow: ellipsis; white-space: nowrap; }
	.landscape-card__copy span { overflow: hidden; color: var(--text-muted); font-size: 0.7rem; text-overflow: ellipsis; white-space: nowrap; }
	.landscape-card__play { position: absolute; z-index: 3; right: 12px; bottom: 12px; display: grid; place-items: center; width: 35px; height: 35px; padding: 0; border: 1px solid rgba(255,255,255,0.8); border-radius: 50%; color: var(--surface-0); background: rgba(247,249,250,0.94); box-shadow: 0 8px 22px rgba(0,0,0,0.34); cursor: pointer; opacity: 0; transform: translateY(5px) scale(0.94); transition: opacity 160ms ease, transform 180ms ease; }
	.landscape-card:hover .landscape-card__play, .landscape-card:focus-within .landscape-card__play { opacity: 1; transform: translateY(0); }
	.landscape-card:hover, .landscape-card:focus-within { transform: translateY(-3px) scale(1.018); }
	.landscape-card:hover .landscape-card__art, .landscape-card:focus-within .landscape-card__art { box-shadow: 0 18px 38px rgba(0,0,0,0.34); }
	.landscape-card:hover .landscape-card__selection-halo, .landscape-card:focus-within .landscape-card__selection-halo { border-color: rgba(255,255,255,0.48); opacity: 1; }
	.landscape-card:hover .landscape-card__art img, .landscape-card:focus-within .landscape-card__art img { filter: brightness(1.06) saturate(1.03); transform: scale(1.012); }
	@media (max-width: 680px) { .media-row { gap: 14px; } .section-heading h2 { font-size: 1.02rem; } .media-row__viewport { margin-right: -18px; } .media-row__track { grid-auto-columns: 72vw; gap: 12px; padding-right: 18px; } .landscape-card__play { display: none; } }
	@media (prefers-reduced-motion: reduce) { .landscape-card, .landscape-card__art img, .landscape-card__play { transition: none; } .landscape-card:hover, .landscape-card:focus-within { transform: none; } }
</style>
