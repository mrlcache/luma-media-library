<script lang="ts">
	import { onMount, tick } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import {
		downloadOpenSubtitle,
		isDesktopRuntime,
		localMediaUrl,
		loginOpenSubtitles,
		openMediaInSystemPlayer,
		resolveMediaFile,
		setOpenSubtitlesApiKey,
		savePlaybackProgress,
		searchOpenSubtitles
	} from '$lib/platform/desktop';
import type { MediaItem } from '$lib/types';
import type { OpenSubtitleSearchResult } from '$lib/types';

	type Props = { media: MediaItem; onClose: () => void };
	let { media, onClose }: Props = $props();

	let overlay: HTMLDivElement;
	let playerStage: HTMLDivElement;
	let video: HTMLVideoElement;
	let isPlaying = $state(false);
	let controlsVisible = $state(true);
	let mediaReady = $state(false);
	let currentTime = $state(0);
	let duration = $state(0);
	let volume = $state(72);
	let isMuted = $state(false);
	let isLoading = $state(true);
	let playbackError = $state('');
	let resumePosition = 0;
	let lastSavedPosition = -1;
	let subtitleTracks = $state<{ label: string; language: string; url: string }[]>([]);
	let activeSubtitle = $state(-1);
	let subtitlePanelOpen = $state(false);
	let subtitleSize = $state(100);
	let subtitleOffset = $state(0);
	let subtitleQuery = $state('');
	let subtitleLanguage = $state('en');
	let subtitleApiKey = $state('');
	let subtitleUsername = $state('');
	let subtitlePassword = $state('');
	let subtitleResults = $state<OpenSubtitleSearchResult[]>([]);
	let subtitleSearchBusy = $state(false);
	let subtitleBusyFile = $state<number | null>(null);
	let subtitleStatus = $state('');
	let subtitleError = $state('');
	let playbackRate = $state(1);
	let audioTracks = $state<{ index: number; label: string; language: string }[]>([]);
	let activeAudio = $state(0);
	let timeoutId: ReturnType<typeof setTimeout> | undefined;

	let progressPercent = $derived(duration > 0 ? currentTime / duration * 100 : 0);
	let elapsedLabel = $derived(formatClock(currentTime));
	let remainingLabel = $derived(`-${formatClock(Math.max(0, duration - currentTime))}`);

	$effect.pre(() => { subtitleQuery = media.title; });

	function formatClock(value: number) {
		const seconds = Math.max(0, Math.round(value));
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
			if (isPlaying && !subtitlePanelOpen) controlsVisible = false;
		}, 2400);
	}

	async function togglePlayback() {
		if (!video || playbackError) return;
		if (video.paused) {
			try { await video.play(); }
			catch (error) { playbackError = error instanceof Error ? error.message : 'Playback could not start.'; }
		} else video.pause();
		revealControls();
	}

	function seekBy(amount: number) {
		if (video && Number.isFinite(video.duration)) video.currentTime = Math.min(video.duration, Math.max(0, video.currentTime + amount));
		revealControls();
	}

	function toggleMuted() {
		isMuted = !isMuted;
		if (video) video.muted = isMuted;
		revealControls();
	}

	function setVolume(event: Event) {
		volume = Number((event.currentTarget as HTMLInputElement).value);
		if (video) { video.volume = volume / 100; video.muted = volume === 0; isMuted = video.muted; }
		revealControls();
	}

	function setPlaybackRate(event: Event) {
		playbackRate = Number((event.currentTarget as HTMLSelectElement).value);
		if (video) video.playbackRate = playbackRate;
		revealControls();
	}

	function seekToPercent(event: Event) {
		if (!video || duration <= 0) return;
		video.currentTime = duration * Number((event.currentTarget as HTMLInputElement).value) / 100;
		revealControls();
	}

	function onTimeUpdate() {
		if (!video) return;
		currentTime = video.currentTime;
		duration = Number.isFinite(video.duration) ? video.duration : duration;
		if (Math.abs(currentTime - lastSavedPosition) >= 10) void persistProgress();
	}

	async function persistProgress() {
		const mediaId = Number(media.id);
		if (!Number.isSafeInteger(mediaId) || mediaId <= 0 || !video || !Number.isFinite(video.duration) || video.duration <= 0) return;
		lastSavedPosition = video.currentTime;
		try { await savePlaybackProgress(mediaId, video.currentTime, video.duration); }
		catch (error) { console.warn('Playback progress could not be saved', error); }
	}

	function onLoadedMetadata() {
		if (!video) return;
		duration = Number.isFinite(video.duration) ? video.duration : 0;
		if (resumePosition > 15 && resumePosition < duration - 10) video.currentTime = resumePosition;
		currentTime = video.currentTime;
		isLoading = false;
		readAudioTracks();
	}

	function readAudioTracks() {
		const list = (video as HTMLVideoElement & { audioTracks?: { length: number; [index: number]: { label?: string; language?: string; enabled: boolean } } }).audioTracks;
		if (!list) { audioTracks = []; return; }
		audioTracks = Array.from({ length: list.length }, (_, index) => ({
			index,
			label: list[index].label || list[index].language || `Track ${index + 1}`,
			language: list[index].language || ''
		}));
		activeAudio = Math.max(0, audioTracks.findIndex((track) => list[track.index].enabled));
	}

	function selectAudio(index: number) {
		const list = (video as HTMLVideoElement & { audioTracks?: { length: number; [index: number]: { enabled: boolean } } }).audioTracks;
		if (list && list[index]) {
			for (let trackIndex = 0; trackIndex < list.length; trackIndex += 1) list[trackIndex].enabled = trackIndex === index;
			activeAudio = index;
		}
	}

	function selectSubtitle(index: number) {
		activeSubtitle = index;
		for (let trackIndex = 0; trackIndex < video.textTracks.length; trackIndex += 1) {
			video.textTracks[trackIndex].mode = trackIndex === index ? 'showing' : 'disabled';
			setCueOffset(video.textTracks[trackIndex]);
		}
	}

	function onTrackLoad(event: Event, index: number) {
		const trackElement = event.currentTarget as HTMLTrackElement;
		trackElement.track.mode = activeSubtitle === index ? 'showing' : 'disabled';
		setCueOffset(trackElement.track);
	}

	function setCueOffset(track: TextTrack) {
		if (!track.cues) return;
		for (const cue of Array.from(track.cues)) {
			if (cue instanceof VTTCue) cue.line = subtitleOffset === 0 ? 'auto' : -subtitleOffset;
		}
	}

	function changeSubtitleOffset(event: Event) {
		subtitleOffset = Number((event.currentTarget as HTMLInputElement).value);
		for (const track of Array.from(video.textTracks)) setCueOffset(track);
	}

	async function signInToOpenSubtitles() {
		subtitleStatus = '';
		subtitleError = '';
		try {
			if (subtitleApiKey.trim()) await setOpenSubtitlesApiKey(subtitleApiKey);
			await loginOpenSubtitles(subtitleUsername, subtitlePassword);
			subtitleApiKey = '';
			subtitlePassword = '';
			subtitleStatus = 'Signed in for this app session. Your password was not saved.';
		} catch (error) { subtitleError = error instanceof Error ? error.message : 'OpenSubtitles sign-in failed.'; }
	}

	async function findOpenSubtitles() {
		subtitleSearchBusy = true;
		subtitleError = '';
		subtitleStatus = '';
		try {
			if (subtitleApiKey.trim()) {
				await setOpenSubtitlesApiKey(subtitleApiKey);
				subtitleApiKey = '';
			}
			subtitleResults = await searchOpenSubtitles(
				subtitleQuery,
				subtitleLanguage,
				media.year > 0 ? media.year : undefined,
				media.kind === 'series' ? 'series' : 'movie'
			);
			if (subtitleResults.length === 0) subtitleStatus = 'No matching subtitle files found.';
		} catch (error) { subtitleError = error instanceof Error ? error.message : 'OpenSubtitles search failed.'; }
		finally { subtitleSearchBusy = false; }
	}

	async function downloadSubtitle(result: OpenSubtitleSearchResult) {
		const mediaId = Number(media.id);
		if (!Number.isSafeInteger(mediaId) || mediaId <= 0) return;
		subtitleBusyFile = result.fileId;
		subtitleError = '';
		subtitleStatus = '';
		try {
			const path = await downloadOpenSubtitle(mediaId, result.fileId);
			const url = await localMediaUrl(path);
			const label = `${result.release} · ${result.language}`;
			subtitleTracks = [...subtitleTracks, { label, language: result.language || subtitleLanguage, url }];
			selectSubtitle(subtitleTracks.length - 1);
			subtitleStatus = 'Subtitle downloaded and enabled.';
		} catch (error) { subtitleError = error instanceof Error ? error.message : 'Subtitle download failed.'; }
		finally { subtitleBusyFile = null; }
	}

	async function loadSubtitleFile(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		const text = await file.text();
		const vtt = file.name.toLowerCase().endsWith('.vtt') ? text : `WEBVTT\n\n${text.replace(/(\d{2}:\d{2}:\d{2}),(\d{3})/g, '$1.$2')}`;
		const url = URL.createObjectURL(new Blob([vtt], { type: 'text/vtt' }));
		subtitleTracks = [...subtitleTracks, { label: file.name, language: 'und', url }];
		selectSubtitle(subtitleTracks.length - 1);
		input.value = '';
	}

	async function toggleFullscreen() {
		try {
			if (document.fullscreenElement) await document.exitFullscreen();
			else await playerStage?.requestFullscreen();
		} catch (error) {
			console.warn('Fullscreen could not be toggled', error);
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'F11') {
			event.preventDefault();
			void toggleFullscreen();
			return;
		}
		revealControls();
		const target = event.target;
		const isRange = target instanceof HTMLInputElement && target.type === 'range';

		if (event.key === 'Escape' && subtitlePanelOpen) { subtitlePanelOpen = false; return; }
		if (event.key === 'Escape' && !document.fullscreenElement) void closePlayer();
		if (event.code === 'Space' && !isRange) {
			event.preventDefault();
			togglePlayback();
		}
		if (event.key === 'ArrowLeft' && !isRange) seekBy(-10);
		if (event.key === 'ArrowRight' && !isRange) seekBy(10);
		if (event.key.toLowerCase() === 'm' && !isRange) toggleMuted();
	}

	function onPlaybackError() {
		if (!video?.error) return;
		playbackError = video.error.code === MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED
			? 'This file or its video codec is not supported by the built-in Windows player. Try an MP4 or WebM file, or open it in your configured desktop player.'
			: 'This media file could not be played. Check that it is still available in the library.';
		isLoading = false;
	}

	async function closePlayer() {
		if (document.fullscreenElement === playerStage) {
			try { await document.exitFullscreen(); } catch { /* The stage is removed immediately after closing. */ }
		}
		await persistProgress();
		for (const track of subtitleTracks) if (track.url.startsWith('blob:')) URL.revokeObjectURL(track.url);
		onClose();
	}

	async function openInSystemPlayer() {
		const mediaId = Number(media.id);
		if (!Number.isSafeInteger(mediaId) || mediaId <= 0) return;
		try { await openMediaInSystemPlayer(mediaId); }
		catch (error) { playbackError = error instanceof Error ? error.message : 'Could not open the desktop player.'; return; }
		await closePlayer();
	}

	onMount(() => {
		const handleFullscreenChange = () => {
			controlsVisible = true;
			revealControls();
		};
		document.addEventListener('fullscreenchange', handleFullscreenChange);
		overlay.focus();
		revealControls();
		if (!isDesktopRuntime()) {
			isLoading = false;
			playbackError = 'Local playback is available in the desktop app.';
			return () => document.removeEventListener('fullscreenchange', handleFullscreenChange);
		}
		const mediaId = Number(media.id);
		if (!Number.isSafeInteger(mediaId) || mediaId <= 0) {
			isLoading = false;
			playbackError = 'This item is not connected to a local media file.';
			return () => document.removeEventListener('fullscreenchange', handleFullscreenChange);
		}
		void resolveMediaFile(mediaId)
			.then(async (source) => {
				resumePosition = source.resumePositionSeconds;
				const episodeMarker = source.path.match(/(?:S\d{1,2}E\d{1,2}|\d{1,2}x\d{2})/i)?.[0];
				if (episodeMarker && media.kind === 'series') subtitleQuery = `${media.title} ${episodeMarker.toUpperCase()}`;
				const tracks = await Promise.all(source.subtitles.map(async (subtitle) => ({
					label: subtitle.label.replace(/\.[^.]+$/, ''),
					language: subtitle.label.match(/\.([a-z]{2,3}(?:-[A-Z]{2})?)\.(?:srt|vtt)$/i)?.[1] ?? 'und',
					url: await localMediaUrl(subtitle.path)
				})));
				subtitleTracks = tracks;
				await tick();
				video.src = await localMediaUrl(source.path);
				video.load();
			})
			.catch((error) => {
				isLoading = false;
				playbackError = error instanceof Error ? error.message : 'The local media file could not be opened.';
			});
		return () => {
			document.removeEventListener('fullscreenchange', handleFullscreenChange);
			if (timeoutId) clearTimeout(timeoutId);
			void persistProgress();
			for (const track of subtitleTracks) if (track.url.startsWith('blob:')) URL.revokeObjectURL(track.url);
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
	<div class="player-stage" class:player-stage--media-ready={mediaReady} bind:this={playerStage} style={`--backdrop: url("${media.backdrop}")`}>
		<div class="player-stage__image" aria-label={`Preview frame for ${media.title}`} role="img"></div>
		<div class="player-stage__ambient"></div>
		<div class="player-stage__vignette"></div>
		<video
			class="player-video"
			class:player-video--hidden={playbackError}
			bind:this={video}
			playsinline
			preload="metadata"
			aria-label={`${media.title} video`}
			style={`--caption-size: ${subtitleSize}%;`}
			onloadedmetadata={onLoadedMetadata}
			onloadeddata={() => (mediaReady = true)}
			ontimeupdate={onTimeUpdate}
			onplay={() => (isPlaying = true)}
			onpause={() => { isPlaying = false; void persistProgress(); }}
			onended={() => { isPlaying = false; currentTime = duration; void persistProgress(); }}
			onerror={onPlaybackError}
		>
			{#each subtitleTracks as track, index (track.url)}
				<track kind="subtitles" src={track.url} srclang={track.language} label={track.label} onload={(event) => onTrackLoad(event, index)} />
			{/each}
		</video>

		{#if isLoading}
			<div class="player-message" role="status"><span class="player-loading__spinner"></span><span>Opening media…</span></div>
		{:else if playbackError}
			<div class="player-message player-message--error" role="alert"><strong>Playback unavailable</strong><span>{playbackError}</span>{#if isDesktopRuntime()}<button type="button" onclick={openInSystemPlayer}>Open in desktop player</button>{/if}</div>
		{/if}

		<div class:player-ui--hidden={!controlsVisible && isPlaying} class="player-ui">
			<div class="player-topbar">
				<button class="player-glass-button" type="button" style="corner-shape: squircle" aria-label="Close player" title="Close player" onclick={closePlayer}>
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
						value={progressPercent}
						oninput={seekToPercent}
						style={`--player-progress: ${progressPercent}%`}
						aria-label="Playback position"
						aria-valuetext={`${elapsedLabel} elapsed, ${remainingLabel} remaining`}
					/>
				</div>

				<div class="player-transport">
					<div class="player-transport__group">
						<button class="player-primary-button" type="button" style="corner-shape: squircle" aria-label={isPlaying ? 'Pause' : 'Play'} title={isPlaying ? 'Pause' : 'Play'} onclick={togglePlayback}>
							<Icon name={isPlaying ? 'pause' : 'play'} size={19} weight="fill" />
						</button>
						<button class="player-control-button" type="button" style="corner-shape: squircle" aria-label="Go back ten seconds" title="Back 10 seconds" onclick={() => seekBy(-10)}>
							<Icon name="arrow-left" size={18} />
						</button>
						<button class="player-control-button" type="button" style="corner-shape: squircle" aria-label="Forward ten seconds" title="Forward 10 seconds" onclick={() => seekBy(10)}>
							<Icon name="arrow-left" size={18} mirrored />
						</button>
						<div class="player-volume" style="corner-shape: squircle">
							<button class="player-volume__button" type="button" aria-label={isMuted ? 'Unmute' : 'Mute'} title={isMuted ? 'Unmute' : 'Mute'} onclick={toggleMuted}>
								<Icon name="volume" size={18} />
							</button>
							<input class="player-volume__range" type="range" min="0" max="100" value={volume} style={`--volume: ${volume}%`} aria-label="Volume" oninput={setVolume} />
						</div>
					</div>

					<div class="player-transport__group player-transport__group--end">
						<button class="player-chip" type="button" style="corner-shape: squircle" aria-label="Subtitles" title="Subtitles" onclick={() => (subtitlePanelOpen = !subtitlePanelOpen)}><Icon name="captions" size={17} /><span>Subtitles</span></button>
						{#if audioTracks.length > 1}
							<select class="player-chip player-chip--select" aria-label="Audio track" value={activeAudio} onchange={(event) => selectAudio(Number(event.currentTarget.value))}>
								{#each audioTracks as track}<option value={track.index}>{track.label}</option>{/each}
							</select>
						{/if}
						<label class="player-chip player-rate" style="corner-shape: squircle"><span class="player-rate__caption">Speed</span><strong class="player-rate__value">{playbackRate}×</strong><Icon name="chevron-down" size={13} /><select aria-label="Playback speed" value={playbackRate} onchange={setPlaybackRate}><option value={0.75}>0.75×</option><option value={1}>1×</option><option value={1.25}>1.25×</option><option value={1.5}>1.5×</option><option value={2}>2×</option></select></label>
						<button class="player-control-button" type="button" style="corner-shape: squircle" aria-label="Toggle fullscreen" title="Fullscreen" onclick={toggleFullscreen}><Icon name="fullscreen" size={18} /></button>
					</div>
				</div>
			</div>
		</div>

		{#if !isPlaying}
			{#if !playbackError}<button class="player-center-play" type="button" style="corner-shape: squircle" aria-label="Play" onclick={togglePlayback}>
				<Icon name="play" size={28} weight="fill" />
			</button>{/if}
		{/if}

		{#if subtitlePanelOpen}
			<section class="subtitle-panel" aria-label="Subtitle settings" style="corner-shape: squircle">
				<header><strong>Subtitles</strong><button type="button" aria-label="Close subtitle settings" onclick={() => (subtitlePanelOpen = false)}><Icon name="close" size={16} /></button></header>
				<div class="subtitle-panel__tracks">
					<button type="button" class:active={activeSubtitle === -1} onclick={() => selectSubtitle(-1)}>Off</button>
					{#each subtitleTracks as track, index}<button type="button" class:active={activeSubtitle === index} onclick={() => selectSubtitle(index)}>{track.label}</button>{/each}
				</div>
				<label>Text size <input type="range" min="75" max="150" step="5" bind:value={subtitleSize} aria-label="Subtitle text size" /></label>
				<label>Vertical position <input type="range" min="0" max="12" step="1" value={subtitleOffset} oninput={changeSubtitleOffset} aria-label="Subtitle vertical position" /></label>
				<div class="subtitle-panel__actions">
					<button type="button" onclick={() => document.getElementById('player-subtitle-file-input')?.click()}><Icon name="captions" size={15} /> Load subtitle file</button>
					<input id="player-subtitle-file-input" class="sr-only" type="file" accept=".srt,.vtt,text/vtt,application/x-subrip" onchange={loadSubtitleFile} />
				</div>
				<details class="subtitle-online">
					<summary><Icon name="search" size={15} /> Find on OpenSubtitles</summary>
					<p class="subtitle-online__note">The API key and sign-in stay in memory for this session; your password is not saved.</p>
					<label>API key <input type="password" bind:value={subtitleApiKey} autocomplete="off" placeholder="OpenSubtitles API key" /></label>
					<div class="subtitle-online__row"><input bind:value={subtitleUsername} autocomplete="username" placeholder="Username" aria-label="OpenSubtitles username" /><input type="password" bind:value={subtitlePassword} autocomplete="current-password" placeholder="Password" aria-label="OpenSubtitles password" /></div>
					<button class="subtitle-online__button" type="button" onclick={signInToOpenSubtitles}>Sign in to download</button>
					<label>Title <input bind:value={subtitleQuery} maxlength="120" aria-label="Subtitle search title" /></label>
					<label>Language
						<select bind:value={subtitleLanguage} aria-label="Subtitle language">
							<option value="en">English</option><option value="pt-br">Portuguese (Brazil)</option><option value="es">Spanish</option><option value="fr">French</option><option value="ja">Japanese</option><option value="ko">Korean</option><option value="de">German</option>
						</select>
					</label>
					<button class="subtitle-online__button subtitle-online__button--primary" type="button" disabled={subtitleSearchBusy} onclick={findOpenSubtitles}>{subtitleSearchBusy ? 'Searching…' : 'Search subtitles'}</button>
					{#if subtitleStatus}<p class="subtitle-online__status" role="status">{subtitleStatus}</p>{/if}
					{#if subtitleError}<p class="subtitle-online__error" role="alert">{subtitleError}</p>{/if}
					{#if subtitleResults.length > 0}
						<div class="subtitle-online__results" aria-label="Subtitle results">
							{#each subtitleResults as result (result.fileId)}
								<div><span><strong>{result.release}</strong><small>{result.language}{result.hearingImpaired ? ' · SDH' : ''} · {result.downloads} downloads</small></span><button type="button" aria-label={`Download ${result.release}`} disabled={subtitleBusyFile === result.fileId} onclick={() => downloadSubtitle(result)}>{subtitleBusyFile === result.fileId ? '…' : 'Get'}</button></div>
							{/each}
						</div>
					{/if}
					<a class="subtitle-online__account" href="https://www.opensubtitles.com/" target="_blank" rel="noreferrer">OpenSubtitles account and API key ↗</a>
				</details>
			</section>
		{/if}
	</div>
</div>

<style>
	.player-overlay { position: fixed; inset: 0; z-index: 80; color: #f4f6f7; background: #020304; outline: none; }
	.player-stage { position: relative; width: 100%; height: 100%; overflow: hidden; isolation: isolate; background: #000; }
	.player-stage:fullscreen { width: 100vw; height: 100vh; background: #000; }
	.player-stage__image { position: absolute; inset: -2.5%; background-image: var(--backdrop); background-position: center; background-size: cover; filter: saturate(0.8) contrast(1.06) brightness(0.74); transform: scale(1.025); z-index: -3; transition: opacity 320ms ease, visibility 0s; }
	.player-stage__ambient { position: absolute; inset: 0; background: radial-gradient(circle at 70% 42%, rgba(151, 193, 211, 0.13), transparent 32%), radial-gradient(circle at 18% 78%, rgba(45, 65, 76, 0.24), transparent 34%); mix-blend-mode: screen; pointer-events: none; z-index: -2; transition: opacity 320ms ease, visibility 0s; }
	.player-stage__vignette { position: absolute; inset: 0; background: radial-gradient(ellipse 82% 72% at 50% 48%, transparent 24%, rgba(2, 4, 6, 0.34) 72%, rgba(2, 4, 6, 0.78) 100%), linear-gradient(180deg, rgba(2,4,6,0.58), transparent 25%, transparent 57%, rgba(2,4,6,0.74)); pointer-events: none; z-index: -1; transition: opacity 320ms ease, visibility 0s; }
	.player-stage--media-ready .player-stage__image,
	.player-stage--media-ready .player-stage__ambient,
	.player-stage--media-ready .player-stage__vignette { visibility: hidden; opacity: 0; transition: opacity 320ms ease, visibility 0s linear 320ms; }
	.player-video { position: absolute; inset: 0; z-index: 0; display: block; width: 100%; height: 100%; background: #000; object-fit: contain; outline: none; }
	.player-video--hidden { visibility: hidden; }
	.player-video::cue { color: #fff; font-size: var(--caption-size, 100%); background: rgba(0,0,0,0.68); text-shadow: 0 1px 2px rgba(0,0,0,0.8); }
	.player-message { position: absolute; top: 50%; left: 50%; z-index: 2; display: grid; justify-items: center; gap: 12px; width: min(440px, calc(100% - 36px)); color: rgba(244,247,249,0.72); font-size: 0.78rem; text-align: center; transform: translate(-50%,-50%); }
	.player-message--error { padding: 22px 24px; border: 1px solid rgba(255,255,255,0.15); border-radius: 18px; background: rgba(13,16,20,0.72); box-shadow: 0 24px 60px rgba(0,0,0,0.38); backdrop-filter: blur(24px) saturate(135%); }
	.player-message--error strong { color: #fff; font-size: 0.95rem; font-weight: 650; }
	.player-message--error button { min-height: 36px; margin-top: 4px; padding: 0 13px; border: 1px solid rgba(255,255,255,0.16); border-radius: 10px; color: #12161a; background: rgba(240,246,248,0.94); cursor: pointer; font: inherit; font-weight: 650; }
	.player-loading__spinner { width: 20px; height: 20px; border: 2px solid rgba(255,255,255,0.2); border-top-color: #e8f3f5; border-radius: 50%; animation: player-spin 700ms linear infinite; }
	@keyframes player-spin { to { transform: rotate(360deg); } }

	.player-ui { position: absolute; inset: 0; z-index: 3; visibility: visible; opacity: 1; pointer-events: none; transition: opacity 220ms ease, visibility 0s; }
	.player-ui--hidden { visibility: hidden; opacity: 0; transition: opacity 220ms ease, visibility 0s linear 220ms; }
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
	.player-chip--select, .player-rate select { max-width: 150px; color: inherit; font: inherit; }
	.player-chip--select option, .player-rate option { color: #eaf0f3; background: #161b21; }
	.player-rate { position: relative; min-width: 108px; justify-content: center; cursor: pointer; }
	.player-rate__value { color: rgba(244,247,249,0.88); font-size: 0.68rem; font-variant-numeric: tabular-nums; font-weight: 700; }
	.player-rate select { position: absolute; inset: 0; z-index: 1; width: 100%; max-width: none; height: 100%; padding: 0; border: 0; outline: 0; opacity: 0; cursor: pointer; }
	.player-rate:focus-within { outline: 2px solid rgba(158,198,214,0.8); outline-offset: 2px; }
	.subtitle-panel { position: absolute; right: 20px; bottom: 124px; z-index: 8; display: grid; gap: 14px; width: min(340px, calc(100vw - 32px)); max-height: min(70vh, 620px); overflow: auto; padding: 17px; border: 1px solid rgba(255,255,255,0.16); border-radius: 19px; color: rgba(244,247,249,0.76); background: rgba(20,24,29,0.91); box-shadow: 0 22px 66px rgba(0,0,0,0.42), inset 0 1px rgba(255,255,255,0.07); backdrop-filter: blur(34px) saturate(145%); pointer-events: auto; }
	.subtitle-panel header { display: flex; align-items: center; justify-content: space-between; color: #f8fafb; font-size: 0.86rem; font-weight: 650; }
	.subtitle-panel header button { display: grid; width: 28px; height: 28px; place-items: center; border: 0; border-radius: 9px; color: inherit; background: transparent; cursor: pointer; }
	.subtitle-panel header button:hover { background: rgba(255,255,255,0.09); }
	.subtitle-panel__tracks { display: grid; gap: 4px; max-height: 145px; overflow: auto; }
	.subtitle-panel__tracks button { overflow: hidden; min-height: 34px; padding: 0 10px; border: 1px solid transparent; border-radius: 9px; color: rgba(244,247,249,0.68); background: transparent; cursor: pointer; text-align: left; text-overflow: ellipsis; white-space: nowrap; }
	.subtitle-panel__tracks button:hover { background: rgba(255,255,255,0.06); }
	.subtitle-panel__tracks button.active { border-color: rgba(255,255,255,0.12); color: #fff; background: rgba(255,255,255,0.1); }
	.subtitle-panel > label { display: grid; gap: 7px; font-size: 0.68rem; }
	.subtitle-panel input[type='range'] { width: 100%; accent-color: #e8f3f5; }
	.subtitle-panel__actions { display: grid; gap: 7px; padding-top: 5px; border-top: 1px solid rgba(255,255,255,0.1); }
	.subtitle-panel__actions button { display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 38px; padding: 0 11px; border: 1px solid rgba(255,255,255,0.12); border-radius: 10px; color: #f1f5f6; background: rgba(255,255,255,0.06); cursor: pointer; font-size: 0.69rem; font-weight: 600; }
	.subtitle-panel__actions button:hover:not(:disabled) { background: rgba(255,255,255,0.12); }
	.subtitle-panel__actions button:disabled { opacity: 0.45; cursor: not-allowed; }
	.subtitle-online { display: grid; gap: 10px; padding-top: 12px; border-top: 1px solid rgba(255,255,255,0.1); }
	.subtitle-online summary { display: flex; align-items: center; gap: 8px; color: #f1f5f6; cursor: pointer; font-size: 0.72rem; font-weight: 640; list-style: none; }
	.subtitle-online summary::-webkit-details-marker { display: none; }
	.subtitle-online[open] summary { margin-bottom: 2px; }
	.subtitle-online__note { margin: -4px 0 0; color: rgba(235,240,244,0.5); font-size: 0.63rem; line-height: 1.45; }
	.subtitle-online > label { display: grid; gap: 5px; font-size: 0.63rem; }
	.subtitle-online input, .subtitle-online select { min-width: 0; height: 34px; padding: 0 9px; border: 1px solid rgba(255,255,255,0.12); border-radius: 9px; outline: none; color: #f2f5f7; background: rgba(5,8,11,0.38); font: inherit; font-size: 0.67rem; }
	.subtitle-online input:focus, .subtitle-online select:focus { border-color: rgba(225,242,248,0.38); }
	.subtitle-online input::placeholder { color: rgba(235,240,244,0.4); }
	.subtitle-online option { color: #edf2f4; background: #171c22; }
	.subtitle-online__row { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; }
	.subtitle-online__button { min-height: 34px; padding: 0 9px; border: 1px solid rgba(255,255,255,0.13); border-radius: 9px; color: rgba(244,247,249,0.8); background: rgba(255,255,255,0.055); cursor: pointer; font-size: 0.66rem; font-weight: 620; }
	.subtitle-online__button:hover:not(:disabled) { background: rgba(255,255,255,0.11); }
	.subtitle-online__button:disabled { opacity: 0.48; cursor: wait; }
	.subtitle-online__button--primary { color: #101418; background: rgba(232,243,245,0.94); }
	.subtitle-online__button--primary:hover:not(:disabled) { background: #fff; }
	.subtitle-online__status, .subtitle-online__error { margin: 0; font-size: 0.64rem; line-height: 1.45; }
	.subtitle-online__status { color: #b8d9cf; }
	.subtitle-online__error { color: #f3b6b7; }
	.subtitle-online__results { display: grid; gap: 6px; max-height: 190px; overflow: auto; }
	.subtitle-online__results > div { display: grid; grid-template-columns: minmax(0,1fr) auto; align-items: center; gap: 8px; padding: 8px; border: 1px solid rgba(255,255,255,0.08); border-radius: 9px; background: rgba(255,255,255,0.035); }
	.subtitle-online__results span { display: grid; gap: 3px; min-width: 0; }
	.subtitle-online__results strong { overflow: hidden; color: #f0f3f5; font-size: 0.64rem; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
	.subtitle-online__results small { color: rgba(235,240,244,0.48); font-size: 0.58rem; }
	.subtitle-online__results button { min-width: 42px; height: 28px; border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: #f3f6f7; background: rgba(255,255,255,0.08); cursor: pointer; font-size: 0.63rem; }
	.subtitle-online__results button:disabled { opacity: 0.5; cursor: wait; }
	.subtitle-online__account { color: rgba(235,240,244,0.54); font-size: 0.61rem; text-decoration: underline; text-underline-offset: 2px; }
	.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; clip-path: inset(50%); }

	@media (max-width: 760px) {
		.player-topbar { padding: 14px 12px 70px; }
		.player-now-playing { top: 16px; width: min(56vw, 300px); }
		.player-glass-button { width: 40px; height: 40px; border-radius: 13px; }
		.player-control-deck { right: 10px; bottom: max(10px, env(safe-area-inset-bottom)); left: 10px; padding: 12px; border-radius: 18px; }
		.player-chip { width: 38px; padding: 0; }
		.player-chip span { display: none; }
		.player-volume__range { width: 54px; }
		.player-center-play { width: 66px; height: 66px; border-radius: 20px; }
		.subtitle-panel { right: 12px; bottom: 118px; }
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
		.player-loading__spinner { animation: none; }
		.player-video { scroll-behavior: auto; }
	}
</style>
