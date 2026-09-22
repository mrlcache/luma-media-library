<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import type { MediaItem } from '$lib/types';

	type Props = { media: MediaItem; onClose: () => void };
	let { media, onClose }: Props = $props();

	let overlay: HTMLDivElement;
	let playerStage: HTMLDivElement;
	let isPlaying = $state(false);
	let controlsVisible = $state(true);
	let progress = $state(12);
	let volume = $state(72);
	let storedVolume = 72;
	let timeoutId: ReturnType<typeof setTimeout> | undefined;

	let durationMinutes = $derived(runtimeInMinutes(media.runtime));
	let elapsedLabel = $derived(formatClock(durationMinutes * (progress / 100)));
	let remainingLabel = $derived(`-${formatClock(durationMinutes * (1 - progress / 100))}`);

	$effect.pre(() => {
		progress = Math.round((media.progress ?? 0.12) * 100);
	});

	function runtimeInMinutes(runtime: string) {
		const hours = Number(runtime.match(/(\d+)\s*h/)?.[1] ?? 0);
		const minutes = Number(runtime.match(/(\d+)\s*m/)?.[1] ?? 0);
		return hours * 60 + minutes || 1;
	}

	function formatClock(minutes: number) {
		const seconds = Math.max(0, Math.round(minutes * 60));
		const hours = Math.floor(seconds / 3600);
		const remainingMinutes = Math.floor((seconds % 3600) / 60);
		const remainingSeconds = seconds % 60;
		return hours > 0
			? `${hours}:${remainingMinutes.toString().padStart(2, '0')}:${remainingSeconds.toString().padStart(2, '0')}`
			: `${remainingMinutes}:${remainingSeconds.toString().padStart(2, '0')}`;
	}

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

	function seekBy(amount: number) {
		progress = Math.min(100, Math.max(0, progress + amount));
		revealControls();
	}

	function toggleMuted() {
		if (volume > 0) {
			storedVolume = volume;
			volume = 0;
		} else {
			volume = storedVolume || 72;
		}
		revealControls();
	}

	async function toggleFullscreen() {
		if (document.fullscreenElement) await document.exitFullscreen();
		else await playerStage?.requestFullscreen();
	}

	function handleKeydown(event: KeyboardEvent) {
		revealControls();
		const target = event.target;
		const isRange = target instanceof HTMLInputElement && target.type === 'range';

		if (event.key === 'Escape' && !document.fullscreenElement) onClose();
		if (event.code === 'Space' && !isRange) {
			event.preventDefault();
			togglePlayback();
		}
		if (event.key === 'ArrowLeft' && !isRange) seekBy(-5);
		if (event.key === 'ArrowRight' && !isRange) seekBy(5);
		if (event.key.toLowerCase() === 'm' && !isRange) toggleMuted();
	}

	onMount(() => {
		overlay.focus();
		revealControls();
		return () => {
			if (timeoutId) clearTimeout(timeoutId);
		};
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<div
	class="player-overlay"
	bind:this={overlay}
	role="dialog"
	aria-modal="true"
	aria-label={`Player for ${media.title}`}
	tabindex="-1"
	onpointermove={revealControls}
>
	<div class="player-stage" bind:this={playerStage} style={`--backdrop: url("${media.backdrop}")`}>
		<div class="player-stage__image" aria-label={`Preview frame for ${media.title}`} role="img"></div>
		<div class="player-stage__ambient"></div>
		<div class="player-stage__vignette"></div>

		<div class:player-ui--hidden={!controlsVisible && isPlaying} class="player-ui">
			<div class="player-topbar">
				<button class="player-glass-button" type="button" style="corner-shape: squircle" aria-label="Close player" title="Close player" onclick={onClose}>
					<Icon name="close" size={20} />
				</button>

				<div class="player-now-playing">
					<span>{media.kind === 'series' ? 'Series' : 'Movie'} · {media.year}</span>
					<strong>{media.title}</strong>
				</div>

				<button class="player-glass-button" type="button" style="corner-shape: squircle" aria-label="More playback options" title="More playback options">
					<Icon name="more" size={20} />
				</button>
			</div>

			<div class="player-control-deck" style="corner-shape: squircle">
				<div class="player-timeline">
					<div class="player-timeline__meta"><span>{elapsedLabel}</span><span>{remainingLabel}</span></div>
					<input
						class="player-range"
						type="range"
						min="0"
						max="100"
						step="0.1"
						bind:value={progress}
						style={`--player-progress: ${progress}%`}
						aria-label="Playback position"
						aria-valuetext={`${elapsedLabel} elapsed, ${remainingLabel} remaining`}
						oninput={revealControls}
					/>
				</div>

				<div class="player-transport">
					<div class="player-transport__group">
						<button class="player-primary-button" type="button" style="corner-shape: squircle" aria-label={isPlaying ? 'Pause' : 'Play'} title={isPlaying ? 'Pause' : 'Play'} onclick={togglePlayback}>
							<Icon name={isPlaying ? 'pause' : 'play'} size={19} weight="fill" />
						</button>
						<button class="player-control-button" type="button" style="corner-shape: squircle" aria-label="Go back five percent" title="Go back" onclick={() => seekBy(-5)}>
							<Icon name="arrow-left" size={18} />
						</button>
						<div class="player-volume" style="corner-shape: squircle">
							<button class="player-volume__button" type="button" aria-label={volume === 0 ? 'Unmute' : 'Mute'} title={volume === 0 ? 'Unmute' : 'Mute'} onclick={toggleMuted}>
								<Icon name="volume" size={18} />
							</button>
							<input class="player-volume__range" type="range" min="0" max="100" bind:value={volume} style={`--volume: ${volume}%`} aria-label="Volume" oninput={revealControls} />
						</div>
					</div>

					<div class="player-transport__group player-transport__group--end">
						<button class="player-chip" type="button" style="corner-shape: squircle" aria-label="Audio and subtitles" title="Audio and subtitles"><Icon name="captions" size={17} /><span>Subtitles</span></button>
						<button class="player-chip" type="button" style="corner-shape: squircle" aria-label="Audio track" title="Audio track"><Icon name="audio" size={17} /><span>English</span></button>
						<button class="player-chip" type="button" style="corner-shape: squircle" aria-label="Quality" title="Quality"><span>Auto</span><Icon name="chevron-down" size={14} /></button>
						<button class="player-control-button" type="button" style="corner-shape: squircle" aria-label="Toggle fullscreen" title="Fullscreen" onclick={toggleFullscreen}><Icon name="fullscreen" size={18} /></button>
					</div>
				</div>
			</div>
		</div>

		{#if !isPlaying}
			<button class="player-center-play" type="button" style="corner-shape: squircle" aria-label="Play" onclick={togglePlayback}>
				<Icon name="play" size={28} weight="fill" />
			</button>
		{/if}
	</div>
</div>

<style>
	.player-overlay { position: fixed; inset: 0; z-index: 80; color: #f4f6f7; background: #020304; outline: none; }
	.player-stage { position: relative; width: 100%; height: 100%; overflow: hidden; isolation: isolate; background: #05070a; }
	.player-stage__image { position: absolute; inset: -2.5%; background-image: var(--backdrop); background-position: center; background-size: cover; filter: saturate(0.8) contrast(1.06) brightness(0.74); transform: scale(1.025); z-index: -3; }
	.player-stage__ambient { position: absolute; inset: 0; background: radial-gradient(circle at 70% 42%, rgba(151, 193, 211, 0.13), transparent 32%), radial-gradient(circle at 18% 78%, rgba(45, 65, 76, 0.24), transparent 34%); mix-blend-mode: screen; pointer-events: none; z-index: -2; }
	.player-stage__vignette { position: absolute; inset: 0; background: radial-gradient(ellipse 82% 72% at 50% 48%, transparent 24%, rgba(2, 4, 6, 0.34) 72%, rgba(2, 4, 6, 0.78) 100%), linear-gradient(180deg, rgba(2,4,6,0.58), transparent 25%, transparent 57%, rgba(2,4,6,0.74)); pointer-events: none; z-index: -1; }

	.player-ui { position: absolute; inset: 0; z-index: 3; opacity: 1; pointer-events: none; transition: opacity 220ms ease; }
	.player-ui--hidden { opacity: 0; }
	.player-ui--hidden * { pointer-events: none !important; }

	.player-topbar { position: absolute; top: 0; right: 0; left: 0; display: flex; align-items: flex-start; justify-content: space-between; padding: 22px 24px 86px; background: linear-gradient(180deg, rgba(2,4,6,0.68), transparent); }
	.player-topbar > * { pointer-events: auto; }
	.player-now-playing { position: absolute; top: 23px; left: 50%; display: grid; gap: 3px; width: min(48vw, 520px); transform: translateX(-50%); text-align: center; text-shadow: 0 2px 16px rgba(0,0,0,0.52); }
	.player-now-playing span { color: rgba(238,242,244,0.5); font-size: 0.61rem; font-weight: 620; letter-spacing: 0.055em; text-transform: uppercase; }
	.player-now-playing strong { overflow: hidden; color: rgba(250,251,252,0.94); font-size: 0.82rem; font-weight: 650; letter-spacing: -0.018em; text-overflow: ellipsis; white-space: nowrap; }

	.player-glass-button,
	.player-control-button,
	.player-primary-button,
	.player-center-play,
	.player-chip,
	.player-volume__button { display: inline-grid; place-items: center; padding: 0; border: 0; color: rgba(244,247,249,0.82); cursor: pointer; }
	.player-glass-button { width: 43px; height: 43px; border: 1px solid rgba(255,255,255,0.14); border-radius: 14px; background: rgba(23,28,34,0.46); box-shadow: inset 0 1px rgba(255,255,255,0.1), 0 10px 28px rgba(0,0,0,0.22); -webkit-backdrop-filter: blur(22px) saturate(145%); backdrop-filter: blur(22px) saturate(145%); transition: border-color 150ms ease, background-color 150ms ease, transform 150ms ease; }
	.player-glass-button:hover { border-color: rgba(255,255,255,0.24); color: #fff; background: rgba(40,47,55,0.62); transform: scale(1.035); }

	.player-center-play { position: absolute; top: 50%; left: 50%; z-index: 4; width: 74px; height: 74px; border: 1px solid rgba(255,255,255,0.72); border-radius: 23px; color: #0b0e11; background: rgba(249,250,251,0.9); box-shadow: inset 0 1px #fff, 0 20px 58px rgba(0,0,0,0.38); transform: translate(-50%, -50%); -webkit-backdrop-filter: blur(18px) saturate(130%); backdrop-filter: blur(18px) saturate(130%); transition: background-color 150ms ease, transform 150ms ease; }
	.player-center-play:hover { background: #fff; transform: translate(-50%, -50%) scale(1.045); }

	.player-control-deck { position: absolute; right: 20px; bottom: max(20px, env(safe-area-inset-bottom)); left: 20px; max-width: 1120px; margin: 0 auto; padding: 15px 17px 14px; border: 1px solid rgba(255,255,255,0.13); border-radius: 22px; background: linear-gradient(145deg, rgba(37,43,50,0.72), rgba(15,19,24,0.66)); box-shadow: inset 0 1px rgba(255,255,255,0.095), 0 24px 74px rgba(0,0,0,0.42); -webkit-backdrop-filter: blur(30px) saturate(145%); backdrop-filter: blur(30px) saturate(145%); pointer-events: auto; }
	.player-timeline { display: grid; gap: 5px; }
	.player-timeline__meta { display: flex; justify-content: space-between; padding: 0 2px; color: rgba(241,244,246,0.55); font-size: 0.62rem; font-variant-numeric: tabular-nums; font-weight: 580; }

	.player-range,
	.player-volume__range { appearance: none; min-width: 0; margin: 0; cursor: pointer; background: transparent; }
	.player-range { width: 100%; height: 16px; }
	.player-range::-webkit-slider-runnable-track { height: 4px; border-radius: 999px; background: linear-gradient(90deg, rgba(194,225,237,0.96) 0 var(--player-progress), rgba(255,255,255,0.2) var(--player-progress) 100%); box-shadow: inset 0 1px rgba(0,0,0,0.18); }
	.player-range::-webkit-slider-thumb { appearance: none; width: 14px; height: 14px; margin-top: -5px; border: 0; border-radius: 50%; background: #eef7fa; box-shadow: 0 2px 10px rgba(0,0,0,0.48); transition: transform 140ms ease; }
	.player-range:hover::-webkit-slider-thumb { transform: scale(1.16); }
	.player-range::-moz-range-track { height: 4px; border: 0; border-radius: 999px; background: rgba(255,255,255,0.2); }
	.player-range::-moz-range-progress { height: 4px; border-radius: 999px; background: rgba(194,225,237,0.96); }
	.player-range::-moz-range-thumb { width: 14px; height: 14px; border: 0; border-radius: 50%; background: #eef7fa; box-shadow: 0 2px 10px rgba(0,0,0,0.48); }

	.player-transport { display: flex; align-items: center; justify-content: space-between; gap: 20px; margin-top: 7px; }
	.player-transport__group { display: flex; align-items: center; gap: 7px; min-width: 0; }
	.player-transport__group--end { justify-content: flex-end; }
	.player-primary-button { width: 46px; height: 46px; flex: 0 0 auto; border-radius: 15px; color: #0a0d10; background: rgba(248,250,251,0.94); box-shadow: inset 0 1px #fff, 0 9px 26px rgba(0,0,0,0.28); transition: background-color 150ms ease, transform 150ms ease; }
	.player-primary-button:hover { background: #fff; transform: scale(1.035); }
	.player-control-button { width: 40px; height: 40px; flex: 0 0 auto; border: 1px solid transparent; border-radius: 13px; background: transparent; transition: border-color 150ms ease, color 150ms ease, background-color 150ms ease; }
	.player-control-button:hover { border-color: rgba(255,255,255,0.1); color: #fff; background: rgba(255,255,255,0.08); }

	.player-volume { display: flex; align-items: center; gap: 2px; min-height: 40px; padding: 0 11px 0 4px; border: 1px solid rgba(255,255,255,0.08); border-radius: 13px; background: rgba(255,255,255,0.045); }
	.player-volume__button { width: 34px; height: 34px; color: rgba(244,247,249,0.7); background: transparent; }
	.player-volume__button:hover { color: #fff; }
	.player-volume__range { width: 76px; height: 14px; }
	.player-volume__range::-webkit-slider-runnable-track { height: 3px; border-radius: 999px; background: linear-gradient(90deg, rgba(223,236,241,0.88) 0 var(--volume), rgba(255,255,255,0.18) var(--volume) 100%); }
	.player-volume__range::-webkit-slider-thumb { appearance: none; width: 11px; height: 11px; margin-top: -4px; border: 0; border-radius: 50%; background: #f4f7f8; box-shadow: 0 1px 6px rgba(0,0,0,0.4); }
	.player-volume__range::-moz-range-track { height: 3px; border: 0; border-radius: 999px; background: rgba(255,255,255,0.18); }
	.player-volume__range::-moz-range-progress { height: 3px; border-radius: 999px; background: rgba(223,236,241,0.88); }
	.player-volume__range::-moz-range-thumb { width: 11px; height: 11px; border: 0; border-radius: 50%; background: #f4f7f8; }

	.player-chip { display: inline-flex; align-items: center; gap: 7px; min-height: 40px; padding: 0 10px; border: 1px solid transparent; border-radius: 13px; color: rgba(244,247,249,0.64); font-size: 0.67rem; font-weight: 620; background: transparent; transition: border-color 150ms ease, color 150ms ease, background-color 150ms ease; }
	.player-chip:hover { border-color: rgba(255,255,255,0.1); color: #fff; background: rgba(255,255,255,0.08); }

	@media (max-width: 760px) {
		.player-topbar { padding: 14px 12px 70px; }
		.player-now-playing { top: 16px; width: min(56vw, 300px); }
		.player-glass-button { width: 40px; height: 40px; border-radius: 13px; }
		.player-control-deck { right: 10px; bottom: max(10px, env(safe-area-inset-bottom)); left: 10px; padding: 12px; border-radius: 18px; }
		.player-chip { width: 38px; padding: 0; }
		.player-chip span { display: none; }
		.player-volume__range { width: 54px; }
		.player-center-play { width: 66px; height: 66px; border-radius: 20px; }
	}

	@media (max-width: 520px) {
		.player-volume { padding-right: 4px; }
		.player-volume__range { display: none; }
		.player-transport { gap: 8px; }
		.player-transport__group { gap: 2px; }
		.player-chip:nth-child(2),
		.player-chip:nth-child(3) { display: none; }
	}

	@media (prefers-reduced-transparency: reduce) {
		.player-control-deck,
		.player-glass-button { background: #161a1f; -webkit-backdrop-filter: none; backdrop-filter: none; }
	}

	@media (prefers-reduced-motion: reduce) {
		.player-ui,
		.player-glass-button,
		.player-primary-button,
		.player-center-play,
		.player-range::-webkit-slider-thumb { transition: none; }
	}
</style>
