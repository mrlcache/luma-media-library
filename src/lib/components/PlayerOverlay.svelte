<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import type { MediaItem } from '$lib/types';

	type Props = { media: MediaItem; onClose: () => void };
	let { media, onClose }: Props = $props();

	let isPlaying = $state(false);
	let controlsVisible = $state(true);
	let progress = $state(0.12);
	let volume = $state(72);
	let timeoutId: ReturnType<typeof setTimeout> | undefined;

	$effect.pre(() => {
		progress = media.progress ?? 0.12;
	});

	function revealControls() {
		controlsVisible = true;
		if (timeoutId) clearTimeout(timeoutId);
		timeoutId = setTimeout(() => {
			if (isPlaying) controlsVisible = false;
		}, 2800);
	}

	function togglePlayback() {
		isPlaying = !isPlaying;
		revealControls();
	}

	function handleKeydown(event: KeyboardEvent) {
		revealControls();
		if (event.key === 'Escape') onClose();
		if (event.key === ' ' && event.target === event.currentTarget) {
			event.preventDefault();
			togglePlayback();
		}
	}

	onMount(() => {
		revealControls();
		return () => {
			if (timeoutId) clearTimeout(timeoutId);
		};
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="player-overlay" role="dialog" aria-modal="true" aria-label={`Player for ${media.title}`} tabindex="-1" onmousemove={revealControls}>
	<div class="player-stage" style={`--backdrop: url("${media.backdrop}")`}>
		<div class="player-stage__image" aria-label={`Preview frame for ${media.title}`} role="img"></div>
		<div class="player-stage__vignette"></div>
		<div class="player-stage__message">
			<strong>{media.title}</strong>
			<span>{media.kind === 'series' ? 'Series' : 'Movie'} · {media.runtime}</span>
		</div>

		{#if controlsVisible}
			<div class="player-topbar">
				<button class="player-icon-button" type="button" aria-label="Close player" title="Close player" onclick={onClose}><Icon name="close" size={20} /></button>
				<div class="player-topbar__title"><span>Playing</span><strong>{media.title}</strong></div>
				<button class="player-icon-button" type="button" aria-label="More playback options" title="More playback options"><Icon name="more" size={20} /></button>
			</div>

			<div class="player-controls">
				<div class="player-scrubber">
					<input class="range-input" type="range" min="0" max="100" step="1" bind:value={progress} aria-label="Playback position" />
					<div class="player-time"><span>{progress < 50 ? '00:42' : '01:08'}</span><span>{media.runtime}</span></div>
				</div>
				<div class="player-controls__row">
					<div class="player-controls__left">
						<button class="player-icon-button" type="button" aria-label={isPlaying ? 'Pause' : 'Play'} title={isPlaying ? 'Pause' : 'Play'} onclick={togglePlayback}><Icon name={isPlaying ? 'pause' : 'play'} size={19} /></button>
						<button class="player-icon-button" type="button" aria-label="Previous episode" title="Previous episode"><Icon name="arrow-left" size={18} /></button>
						<div class="volume-control">
							<Icon name="volume" size={18} />
							<input class="volume-input" type="range" min="0" max="100" bind:value={volume} aria-label="Volume" />
						</div>
					</div>
					<div class="player-controls__right">
						<button class="player-text-button" type="button" aria-label="Audio and subtitles" title="Audio and subtitles"><Icon name="captions" size={17} /><span>Subtitles</span></button>
						<button class="player-text-button" type="button" aria-label="Audio track" title="Audio track"><Icon name="audio" size={17} /><span>English</span></button>
						<button class="player-text-button" type="button" aria-label="Quality" title="Quality"><span>Auto</span><Icon name="chevron-down" size={14} /></button>
						<button class="player-icon-button" type="button" aria-label="Fullscreen" title="Fullscreen"><Icon name="fullscreen" size={18} /></button>
					</div>
				</div>
			</div>
		{/if}
	</div>
</div>

<style>
	.player-overlay { position: fixed; inset: 0; z-index: 80; display: block; padding: 0; background: #030507; }
	.player-stage { position: relative; display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; overflow: hidden; background: #060709; }
	.player-stage__image { position: absolute; inset: -2%; background-image: var(--backdrop); background-position: center; background-size: cover; filter: saturate(0.7) contrast(1.08); opacity: 0.62; }
	.player-stage__vignette { position: absolute; inset: 0; background: radial-gradient(circle at center, transparent 19%, rgba(3,5,7,0.48) 100%), linear-gradient(180deg, rgba(3,5,7,0.72), transparent 32%, transparent 55%, rgba(3,5,7,0.9)); }
	.player-stage__message { position: relative; z-index: 1; display: grid; gap: 6px; justify-items: center; color: var(--text-soft); text-align: center; text-shadow: 0 2px 18px rgba(0,0,0,0.55); pointer-events: none; }
	.player-stage__message strong { color: var(--text-strong); font-size: clamp(1.25rem, 2.8vw, 2.25rem); font-weight: 560; letter-spacing: -0.04em; }
	.player-stage__message span { color: rgba(240,240,236,0.62); font-size: 0.75rem; }
	.player-topbar { position: absolute; top: 0; right: 0; left: 0; z-index: 2; display: flex; align-items: center; justify-content: space-between; padding: 24px 28px 66px; background: linear-gradient(180deg, rgba(5,7,9,0.7), transparent); }
	.player-topbar__title { position: absolute; top: 25px; left: 50%; display: grid; gap: 1px; min-width: min(42vw, 420px); transform: translateX(-50%); text-align: center; }
	.player-topbar__title span { color: rgba(255,255,255,0.52); font-size: 0.62rem; }
	.player-topbar__title strong { overflow: hidden; color: rgba(255,255,255,0.92); font-size: 0.78rem; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
	.player-icon-button { display: inline-grid; place-items: center; width: 40px; height: 40px; padding: 0; border: 1px solid rgba(255,255,255,0.12); border-radius: var(--radius-sm); color: rgba(255,255,255,0.8); background: rgba(10,12,15,0.42); cursor: pointer; transition: border-color 160ms ease, color 160ms ease, background-color 160ms ease; }
	.player-icon-button:hover { border-color: rgba(255,255,255,0.24); color: #fff; background: rgba(10,12,15,0.72); }
	.player-controls { position: absolute; right: 0; bottom: 0; left: 0; z-index: 2; padding: 84px 36px 28px; background: linear-gradient(0deg, rgba(5,7,9,0.92), transparent); }
	.player-scrubber { display: grid; gap: 8px; }
	.range-input, .volume-input { width: 100%; accent-color: var(--accent); cursor: pointer; }
	.range-input { height: 4px; }
	.volume-input { width: 74px; height: 3px; }
	.player-time { display: flex; justify-content: space-between; color: rgba(255,255,255,0.58); font-size: 0.66rem; }
	.player-controls__row { display: flex; align-items: center; justify-content: space-between; gap: 18px; margin-top: 10px; }
	.player-controls__left, .player-controls__right, .volume-control { display: flex; align-items: center; gap: 7px; }
	.volume-control { gap: 9px; margin-left: 8px; color: rgba(255,255,255,0.7); }
	.player-text-button { display: inline-flex; align-items: center; gap: 6px; min-height: 36px; padding: 0 9px; border: 1px solid transparent; border-radius: var(--radius-sm); color: rgba(255,255,255,0.68); font-size: 0.68rem; background: transparent; cursor: pointer; transition: border-color 160ms ease, color 160ms ease, background-color 160ms ease; }
	.player-text-button:hover { border-color: rgba(255,255,255,0.12); color: #fff; background: rgba(255,255,255,0.08); }
	@media (max-width: 620px) {
		.player-topbar { padding: 16px 14px 58px; }
		.player-topbar__title { top: 17px; min-width: min(54vw, 240px); }
		.player-controls { padding: 70px 14px 18px; }
		.player-text-button span { display: none; }
		.volume-input { width: 54px; }
		.player-controls__right { gap: 2px; }
	}
	@media (prefers-reduced-transparency: reduce) {
		.player-overlay { background: var(--surface-0); }
		.player-stage__vignette { background: linear-gradient(180deg, rgba(3,5,7,0.78), transparent 34%, transparent 52%, rgba(3,5,7,0.94)); }
		.player-icon-button, .player-text-button { background: var(--surface-2); }
	}
	@media (prefers-reduced-motion: reduce) { .player-icon-button, .player-text-button { transition: none; } }
</style>
