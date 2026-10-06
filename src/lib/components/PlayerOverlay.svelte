<script module lang="ts">
	let nativeOwner: symbol | null = null;
</script>

<script lang="ts">
	import AppSelect from '$lib/components/AppSelect.svelte';
	import { bitratePresets, readTranscodePreferences, saveTranscodePreferences } from '$lib/platform/transcode-preferences';
	import { playbackPosition, resumePlaybackPosition, reachedPlaybackEnd } from '$lib/platform/playback-timeline';
	import { fetchCompatibleMedia } from '$lib/platform/compatible-stream';
	import { openHlsStream } from '$lib/platform/hls-stream';
	import { onMount, tick } from 'svelte';
	import { dev } from '$app/environment';
	import { isMobilePreview } from '$lib/platform/mobile-preview';
	import {readPlayerLevels, setPlayerLevel, resetPlayerLevels} from '$lib/platform/player-device';
	import {nativeMobile} from '$lib/platform/mobile-connection';
	import Icon from '$lib/components/Icon.svelte';
	import SteppedRange from '$lib/components/SteppedRange.svelte';
	import { suspendNativeAcrylicForPlayback } from '$lib/platform/native-acrylic';
	import {
		downloadOpenSubtitle,
		isDesktopRuntime,
		localMediaUrl,
		chooseSubtitleFile,
		loginOpenSubtitles,
		nativePlayerAction,
		nativePlayerLoadSubtitle,
		nativePlayerStatus,
		readPreferredDesktopPlayer,
		recordPlaybackActivity,
		resolveMediaFile,
		resolveSubtitleFile,
		resizeNativePlayer,
		savePreferredDesktopPlayer,
		setOpenSubtitlesApiKey,
		savePlaybackProgress,
		searchOpenSubtitles,
		startNativePlayer,
		stopNativePlayer,
		type NativePlaybackSnapshot,
		type DesktopPlayer
	} from '$lib/platform/desktop';
	import type { MediaItem, OpenSubtitleSearchResult } from '$lib/types';
	import { readPlaybackPreferences, updatePlaybackPreference, type SubtitleFont } from '$lib/platform/playback-preferences';
	import { PLAYBACK_HISTORY_UPDATED_EVENT, usePlayer } from '$lib/player-context';

	type Props = { media: MediaItem; onClose: () => void; visualPreview?: boolean };
	let { media, onClose, visualPreview = false }: Props = $props();
	function isVisualPreview() { return dev && visualPreview; }
	const mobilePlayer = isMobilePreview() || isVisualPreview();
	let previewOnly = $derived(dev && visualPreview);
	const player = usePlayer();

	let overlay: HTMLDivElement;
	let playerStage: HTMLDivElement;
	let video: HTMLVideoElement;
	let isPlaying = $state(false);
	let controlsVisible = $state(true);
	let screenLocked = $state(false);
	let mediaReady = $state(false);
	let currentTime = $state(isVisualPreview() ? 854 : 0);
	let duration = $state(isVisualPreview() ? 3120 : 0);
	let volume = $state(72);
	let isMuted = $state(false);
	let desktopVolumeFeedback = $state(false);
	let desktopVolumeFeedbackTimer: ReturnType<typeof setTimeout> | undefined;
	let isLoading = $state(!isVisualPreview());
	let playbackError = $state('');
	$effect(() => {
		if (!playbackError) return;
		isLoading = false; isPlaying = false;
		subtitlePanelOpen = false; speedMenuOpen = false; gestureFeedback = null; gesture = null;
	});
	let playerMenuOpen = $state(false);
	let mobileSourceUrl = $state('');
	let transcoding = $state(false);
	let activeTranscoding = $state(false);
	let transcodeQuality = $state('auto');
	let transcodeBitrate = $state(0);
	const bitrateOptions = $derived((bitratePresets[transcodeQuality] || []).map(value=>({value,label:`${value / 1_000_000} Mbps`})));
	const mobileStreamSession = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2,14)}`;
	let mobileMetadataAbort: AbortController | undefined;
	let compatibleStream: ReturnType<typeof openHlsStream> | undefined;
	let mobileStreamPaused = false;
	function stopCompatibleStream() {
		compatibleStream?.stop();
		compatibleStream = undefined;
		mobileStreamPaused = false;
	}
	function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string) {
		let timer: ReturnType<typeof setTimeout>;
		return Promise.race([
			promise,
			new Promise<T>((_, reject) => {
				timer = setTimeout(() => reject(new Error(message)), timeoutMs);
			})
		]).finally(() => clearTimeout(timer));
	}
	let sourceBitrate = $state(0);
	const qualityOptions = [
		{ value: 'auto', label: 'Automatic' },
		{ value: '480p', label: '480p' },
		{ value: '720p', label: '720p' },
		{ value: '1080p', label: '1080p' },
	];
	let transcodeOffset = 0;
	let mobilePendingSeek:number|null = null;
	let mobilePendingResume = false;
	const originalCueTimes = new WeakMap<TextTrackCue,[number,number]>();
	let mobileLoadAttempt = 0;
	const canTranscode = $derived(previewOnly || /^https?:\/\/[^/]+\/api\/v1\/media\/\d+\?/.test(mobileSourceUrl));
	let speedMenuOpen = $state(false);
	let brightness = $state(100);
	let gestureFeedback = $state<'brightness' | 'volume' | 'back' | 'forward' | null>(null);
	let gestureFeedbackTimer: ReturnType<typeof setTimeout> | undefined;
	let gesture: { pointerId: number; side: 'brightness' | 'volume'; x: number; y: number; time: number; value: number; moved: boolean } | null = null;
	let lastTap: { side: 'brightness' | 'volume'; time: number; x: number; y: number } | null = null;
	let mobileTapTimer: ReturnType<typeof setTimeout> | undefined;
	let mobileBackListener: { unregister: () => Promise<void> } | undefined;
	$effect(() => () => {
		if (gestureFeedbackTimer) clearTimeout(gestureFeedbackTimer);
		if (mobileTapTimer) clearTimeout(mobileTapTimer);
	});
	let desktopPlayerBusy = $state(false);
	let selectedDesktopPlayer = $state<DesktopPlayer>('mpv');
	let activeEngine = $state<DesktopPlayer | null>(null);
	let nativePoll: ReturnType<typeof setInterval> | undefined;
	let nativeLoadingTimeout: number | undefined;
	let playerDisposed = false;
	const playerToken = Symbol('native-player-owner');
	let pendingNativeSubtitlePath: string | null = null;
	let pendingNativeResume = 0;
	let resumePosition = 0;
	let lastSavedPosition = -1;
	let playbackActivityPromise: Promise<void> | null = null;
	let playbackActivityStarted = false;
	let subtitleTracks = $state<{ label: string; language: string; url: string; path?: string; streamIndex?: number | null; supported?: boolean; lazyIndex?: number }[]>([]);
	let subtitleSelectionRequest = 0;
	let subtitleWarning = $state('');
	let nativeSubtitleTracks = $state<{ id: number; label: string; language: string; selected: boolean }[]>([]);
	let nativeAudioTracks = $state<{ id: number; label: string; language: string; selected: boolean }[]>([]);
	let activeSubtitle = $state(-1);
	let subtitlePanelOpen = $state(false);
	let subtitleFont = $state<SubtitleFont>('Manrope');
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
	let audioTracks = $state<{ index: number; nativeId?: number; label: string; language: string }[]>([]);
	let activeAudio = $state(0);
	let endedHandled = false;
	let autoplayTransitioning = false;
	let vlcSubtitleNote = $state(false);
	let timeoutId: ReturnType<typeof setTimeout> | undefined;

	let scrubPercent = $state<number | null>(null);
	let progressPercent = $derived(scrubPercent ?? (duration > 0 ? currentTime / duration * 100 : 0));
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

	function desktopPlayerLabel(player = selectedDesktopPlayer): string {
		return player === 'mpv' ? 'mpv' : 'VLC';
	}

	function setDesktopPlayer(value: DesktopPlayer) {
		selectedDesktopPlayer = value;
		savePreferredDesktopPlayer(selectedDesktopPlayer);
	}

	function applyNativeSnapshot(snapshot: NativePlaybackSnapshot) {
		const ended = snapshot.ended;
		activeEngine = snapshot.engine;
		isPlaying = snapshot.playing;
		currentTime = snapshot.positionSeconds;
		duration = snapshot.durationSeconds;
		volume = Math.round(snapshot.volume);
		isMuted = snapshot.muted;
		if (snapshot.rate > 0) playbackRate = snapshot.rate;
		nativeAudioTracks = snapshot.audioTracks ?? [];
		nativeSubtitleTracks = snapshot.subtitleTracks ?? [];
		if (activeEngine) {
			audioTracks = nativeAudioTracks.map((track, index) => ({ index, nativeId: track.id, label: track.label, language: track.language }));
			activeAudio = Math.max(0, nativeAudioTracks.findIndex((track) => track.selected));
			vlcSubtitleNote = snapshot.engine === 'vlc';
		}
		if (duration > 0) {
			mediaReady = true;
			isLoading = false;
		}
		if (Math.abs(currentTime - lastSavedPosition) >= 10) void persistProgress();
		if (ended && !endedHandled) { endedHandled = true; void handlePlaybackEnded(); }
	}

	async function startSelectedEngine(position = resumePosition) {
		if (playerDisposed) return;
		const mediaId = Number(media.id);
		if (!Number.isSafeInteger(mediaId) || mediaId <= 0) return;
		endedHandled = false;
		desktopPlayerBusy = true;
		isLoading = true;
		playbackError = '';
		if (nativePoll) clearInterval(nativePoll);
		try {
			const preferences = readPlaybackPreferences();
			const systemLanguage = navigator.language?.slice(0, 2).toLowerCase();
			if (preferences.audioLanguage === 'system' && ['en', 'pt', 'ja'].includes(systemLanguage ?? '')) {
				preferences.audioLanguage = systemLanguage as 'en' | 'pt' | 'ja';
			}
			if (preferences.subtitleLanguage === 'auto' && ['en', 'pt', 'ja'].includes(systemLanguage ?? '')) {
				preferences.subtitleLanguage = systemLanguage as 'en' | 'pt' | 'ja';
			}
			nativeOwner = playerToken;
			const snapshot = await startNativePlayer(mediaId, selectedDesktopPlayer, preferences);
			if (playerDisposed) {
				// A new overlay may already have started another native session.
				if (nativeOwner === playerToken) {
					nativeOwner = null;
					await stopNativePlayer().catch(() => undefined);
				}
				return;
			}
			applyNativeSnapshot(snapshot);
			pendingNativeSubtitlePath = activeSubtitle >= 0 ? subtitleTracks[activeSubtitle]?.path ?? null : null;
			recordPlaybackStarted();
			pendingNativeResume = position > 15 ? position : 0;
			document.documentElement.dataset.nativePlayer = 'true';
			if (video) { video.pause(); video.removeAttribute('src'); video.load(); }
			nativePoll = setInterval(() => {
				void nativePlayerStatus().then(async (state) => {
					if (playerDisposed) return;
					applyNativeSnapshot(state);
					if (pendingNativeSubtitlePath && state.durationSeconds > 0) {
						const path = pendingNativeSubtitlePath;
						pendingNativeSubtitlePath = null;
						if (!state.subtitleTracks.some((track) => track.selected)) {
							const updated = await nativePlayerLoadSubtitle(path);
							if (playerDisposed) return;
							applyNativeSnapshot(updated);
						}
					}
					if (pendingNativeResume > 0 && state.durationSeconds > pendingNativeResume + 10) {
						const seekTo = pendingNativeResume;
						pendingNativeResume = 0;
						const updated = await nativePlayerAction('seek', seekTo);
						if (playerDisposed) return;
						applyNativeSnapshot(updated);
					}
				}).catch((error) => { if (!playerDisposed) console.warn('Native player status unavailable', error); });
			}, 500);
			if (nativeLoadingTimeout) clearTimeout(nativeLoadingTimeout);
			nativeLoadingTimeout = window.setTimeout(() => {
				nativeLoadingTimeout = undefined;
				if (!playerDisposed && activeEngine && isLoading) isLoading = false;
			}, 5000);
		} catch (error) {
			if (playerDisposed) return;
			activeEngine = null;
			isPlaying = false;
			isLoading = false;
			delete document.documentElement.dataset.nativePlayer;
			playbackError = error instanceof Error ? error.message : typeof error === 'string' && error.trim() ? error : `Could not start ${desktopPlayerLabel()}.`;
		} finally {
			desktopPlayerBusy = false;
		}
	}

	function revealControls() {
		if (screenLocked) return;
		controlsVisible = true;
		if (timeoutId) clearTimeout(timeoutId);
		timeoutId = setTimeout(() => {
			if (isPlaying && !subtitlePanelOpen && !playerMenuOpen && !speedMenuOpen) controlsVisible = false;
		}, 2400);
	}

	async function togglePlayback() {
		if (previewOnly) { isPlaying = !isPlaying; return; }
		if (activeEngine) {
			try { applyNativeSnapshot(await nativePlayerAction(isPlaying ? 'pause' : 'play')); }
			catch (error) { playbackError = error instanceof Error ? error.message : 'Playback could not start.'; }
			revealControls();
			return;
		}
		if (!video || playbackError) return;
		if (video.paused) {
			if (mobilePlayer && activeTranscoding && mobileStreamPaused) { await loadMobileStream(currentTime); revealControls(); return; }
			try { await video.play(); }
			catch (error) { playbackError = error instanceof Error ? error.message : 'Playback could not start.'; }
		} else video.pause();
		revealControls();
	}

	function seekBy(amount: number) {
		if (previewOnly) { currentTime = Math.min(duration, Math.max(0, currentTime + amount)); return; }
		if (mobilePlayer && activeTranscoding) {
			const target = Math.min(duration || 86400, Math.max(0, currentTime + amount));
			const streamTarget = target - transcodeOffset;
			const alreadyBuffered = video && streamTarget >= 0 && Array.from({ length: video.buffered.length }, (_, index) => ({ start: video.buffered.start(index), end: video.buffered.end(index) }))
				.some((range) => streamTarget >= range.start - 0.25 && streamTarget <= range.end + 0.25);
			if (alreadyBuffered) {
				video.currentTime = streamTarget;
				currentTime = target;
				revealControls();
			} else void loadMobileStream(target);
			return;
		}
		if (activeEngine) {
			void nativePlayerAction('seek', Math.min(duration || Number.MAX_SAFE_INTEGER, Math.max(0, currentTime + amount))).then(applyNativeSnapshot).catch(console.warn);
			revealControls();
			return;
		}
		if (video && Number.isFinite(video.duration)) video.currentTime = Math.min(video.duration, Math.max(0, video.currentTime + amount));
		revealControls();
	}

	function toggleMuted() {
		isMuted = !isMuted;
		if (activeEngine) void nativePlayerAction('mute', isMuted ? 1 : 0).then(applyNativeSnapshot).catch(console.warn);
		if (video) video.muted = isMuted;
		revealControls();
	}

	function setVolume(event: Event) {
		applyVolume(Number((event.currentTarget as HTMLInputElement).value));
	}

	function applyVolume(value: number, showControls = true) {
		volume = Math.round(Math.min(100, Math.max(0, value)));
		isMuted = volume === 0;
		if (!previewOnly) setPlayerLevel('volume', volume);
		if (activeEngine) void nativePlayerAction('volume', volume).then(applyNativeSnapshot).catch(console.warn);
		if (video) { video.volume = nativeMobile ? 1 : volume / 100; video.muted = volume === 0; isMuted = video.muted; }
		if (showControls) revealControls();
	}

	function showDesktopVolumeFeedback() {
		desktopVolumeFeedback = true;
		if (desktopVolumeFeedbackTimer) clearTimeout(desktopVolumeFeedbackTimer);
		desktopVolumeFeedbackTimer = setTimeout(() => { desktopVolumeFeedback = false; }, 900);
	}

	function setPlaybackRate(value: number) {
		playbackRate = value;
		if (activeEngine) void nativePlayerAction('rate', playbackRate).then(applyNativeSnapshot).catch(console.warn);
		if (video) video.playbackRate = playbackRate;
		revealControls();
	}

	function seekToPercent(event: Event) {
		if (duration <= 0) return;
		if (previewOnly) { currentTime = duration * Number((event.currentTarget as HTMLInputElement).value) / 100; return; }
		if (activeEngine) {
			void nativePlayerAction('seek', duration * Number((event.currentTarget as HTMLInputElement).value) / 100).then(applyNativeSnapshot).catch(console.warn);
			revealControls();
			return;
		}
		if (!video) return;
		if (mobilePlayer && activeTranscoding) { void loadMobileStream(duration * Number((event.currentTarget as HTMLInputElement).value) / 100); return; }
		video.currentTime = duration * Number((event.currentTarget as HTMLInputElement).value) / 100;
		revealControls();
	}
	function previewSeek(event: Event) {
		if (mobilePlayer && activeTranscoding && !previewOnly) {
			scrubPercent = Math.max(0, Math.min(100, Number((event.currentTarget as HTMLInputElement).value)));
		} else seekToPercent(event);
	}
	function commitSeek(event: Event) {
		if (mobilePlayer && activeTranscoding && !previewOnly) seekToPercent(event);
		scrubPercent = null;
	}

	function onTimeUpdate() {
		if (!video || isLoading || !mediaReady || playbackError) return;
		currentTime = playbackPosition(video.currentTime, activeTranscoding ? transcodeOffset : 0, activeTranscoding ? duration : video.duration);
		if (!activeTranscoding) duration = Number.isFinite(video.duration) ? video.duration : duration;
		if (Math.abs(currentTime - lastSavedPosition) >= 10) void persistProgress();
	}
	function onPlaybackPause() {
		if (!video?.paused) return;
		isPlaying = false;
		revealControls();
		if (mobilePlayer && activeTranscoding && compatibleStream && !isLoading && mediaReady && !playbackError) {
			currentTime = playbackPosition(video.currentTime, transcodeOffset, duration);
			compatibleStream.suspend();
			mobileStreamPaused = true;
		}
		void persistProgress();
	}

	async function persistProgress() {
		// Loading, failed opens and source-reset pause events are not watched progress.
		if (isLoading || !mediaReady || playbackError) return;
		const mediaId = Number(media.id);
		const position = activeEngine || activeTranscoding ? currentTime : video?.currentTime;
		const length = activeEngine || activeTranscoding ? duration : video?.duration;
		if (!Number.isSafeInteger(mediaId) || mediaId <= 0 || !Number.isFinite(position) || !Number.isFinite(length) || !length || length <= 0) return;
		lastSavedPosition = position!;
		try { await savePlaybackProgress(mediaId, position!, length); }
		catch (error) { console.warn('Playback progress could not be saved', error); }
	}

	function recordPlaybackStarted() {
		const mediaId = Number(media.id);
		if (!Number.isSafeInteger(mediaId) || mediaId <= 0 || playbackActivityStarted) return;
		// Pause/resume stays within the same player session, so it must not record a new start.
		playbackActivityStarted = true;
		playbackActivityPromise = recordPlaybackActivity(mediaId, readPlaybackPreferences().markPreviousEpisodesWatched).catch((error) => {
			console.warn('Playback history could not be saved', error);
		});
	}

	function onLoadedMetadata() {
		if (!video) return;
		for(const track of Array.from(video.textTracks))setCueOffset(track);
		if (activeTranscoding) { currentTime = transcodeOffset; video.playbackRate = playbackRate; readAudioTracks(); return; }
		duration = Number.isFinite(video.duration) ? video.duration : 0;
		if(mobilePendingSeek!==null){video.currentTime=mobilePendingResume ? resumePlaybackPosition(mobilePendingSeek,duration) : Math.min(Math.max(0,mobilePendingSeek),Math.max(0,duration-.1));mobilePendingSeek=null;}
		else if (resumePosition > 0) video.currentTime = resumePlaybackPosition(resumePosition, duration);
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
		const preference = readPlaybackPreferences().audioLanguage;
		const desiredLanguage = preference === 'system' ? (navigator.language?.slice(0, 2) ?? '') : preference;
		const preferredIndex = audioTracks.findIndex((track) => track.language.toLowerCase().startsWith(desiredLanguage.toLowerCase()));
		if (preferredIndex >= 0) selectAudio(preferredIndex);
	}

	function preferredSubtitleIndex(tracks: { language: string; supported?: boolean }[]): number {
		const preference = readPlaybackPreferences().subtitleLanguage;
		if (preference === 'off') return -1;
		const desiredLanguage = preference === 'auto' ? (navigator.language?.slice(0, 2) ?? '') : preference;
		if (!desiredLanguage) return -1;
		const normalize = (language: string) => ({ eng:'en', por:'pt', spa:'es', fra:'fr', fre:'fr', deu:'de', ger:'de', jpn:'ja', kor:'ko', ita:'it', rus:'ru', zho:'zh', chi:'zh' }[language.toLowerCase()] ?? language.toLowerCase());
		return tracks.findIndex((track) => track.supported !== false && normalize(track.language).startsWith(normalize(desiredLanguage)));
	}

	function selectAudio(index: number) {
		if (activeEngine) {
			const track = audioTracks[index];
			if (track?.nativeId !== undefined) void nativePlayerAction('audio-track', track.nativeId).then(applyNativeSnapshot).catch((error) => { playbackError = error instanceof Error ? error.message : 'Audio track could not be selected.'; });
			return;
		}
		const list = (video as HTMLVideoElement & { audioTracks?: { length: number; [index: number]: { enabled: boolean } } }).audioTracks;
		if (list && list[index]) {
			for (let trackIndex = 0; trackIndex < list.length; trackIndex += 1) list[trackIndex].enabled = trackIndex === index;
			activeAudio = index;
		}
	}

	function selectNativeSubtitle(id: number) {
		void nativePlayerAction('subtitle-track', id).then(applyNativeSnapshot).catch((error) => { subtitleError = error instanceof Error ? error.message : 'Subtitle track could not be selected.'; });
	}

	async function selectNativeExternalSubtitle(track: { label: string; path?: string }) {
		if (!track.path) return;
		subtitleError = '';
		try {
			applyNativeSnapshot(await nativePlayerLoadSubtitle(track.path));
			subtitleStatus = `${track.label} loaded and enabled.`;
		} catch (error) { subtitleError = error instanceof Error ? error.message : 'Subtitle file could not be loaded.'; }
	}

	async function selectSubtitle(index: number) {
		const request = ++subtitleSelectionRequest;
		subtitleError = '';
		const selected = subtitleTracks[index];
		if (selected?.supported === false) return;
		if (selected?.lazyIndex !== undefined) {
			try {
				const path = await resolveSubtitleFile(Number(media.id), selected.lazyIndex);
				if (playerDisposed || request !== subtitleSelectionRequest) return;
				const url = await localMediaUrl(path);
				if (playerDisposed || request !== subtitleSelectionRequest) return;
				selected.url = url;
				selected.lazyIndex = undefined;
				await tick();
			} catch (error) { subtitleError = error instanceof Error ? error.message : String(error); return; }
		}
		activeSubtitle = index;
		if (mobilePlayer) await tick();
		if (playerDisposed || request !== subtitleSelectionRequest) return;
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
			if(!originalCueTimes.has(cue))originalCueTimes.set(cue,[cue.startTime,cue.endTime]);
			const times=originalCueTimes.get(cue)!;
			cue.startTime=Math.max(0,times[0]-(activeTranscoding?transcodeOffset:0));
			cue.endTime=Math.max(0,times[1]-(activeTranscoding?transcodeOffset:0));
			if (cue instanceof VTTCue) cue.line = subtitleOffset === 0 ? 'auto' : -(subtitleOffset * 1.5);
		}
	}

	function changeSubtitleOffset(event: Event) {
		subtitleOffset = Number((event.currentTarget as HTMLInputElement).value);
		const position = subtitleOffset * 1.5;
		updatePlaybackPreference('subtitlePosition', position);
		if (activeEngine === 'mpv') void nativePlayerAction('subtitle-position', position).then(applyNativeSnapshot).catch(console.warn);
		for (const track of Array.from(video.textTracks)) setCueOffset(track);
	}

	function changeSubtitleSize(event: Event) {
		subtitleSize = Number((event.currentTarget as HTMLInputElement).value);
		updatePlaybackPreference('subtitleSize', subtitleSize);
		if (activeEngine === 'mpv') void nativePlayerAction('subtitle-size', subtitleSize).then(applyNativeSnapshot).catch(console.warn);
	}

	async function handlePlaybackEnded() {
		if (playerDisposed || autoplayTransitioning || isLoading || !mediaReady || playbackError) return;
		isPlaying = false;
		if (!activeEngine && video) currentTime = playbackPosition(video.currentTime, activeTranscoding ? transcodeOffset : 0, duration);
		if (!reachedPlaybackEnd(currentTime, duration)) {
			await persistProgress();
			playbackError = 'The stream stopped before the video ended. Your position is saved. Try again to continue.';
			return;
		}
		await persistProgress();
		await playbackActivityPromise;
		if (playerDisposed || isLoading) return;
		const preferences = readPlaybackPreferences();
		if (!preferences.autoplayNextEpisode || !media.nextEpisode) return;
		autoplayTransitioning = true;
		if (nativePoll) clearInterval(nativePoll);
		if (activeEngine) {
			await stopNativePlayer().catch((error) => console.warn('Finished episode could not be closed', error));
			activeEngine = null;
		}
		delete document.documentElement.dataset.nativePlayer;
		player.open(media.nextEpisode);
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
			if (activeEngine) {
				applyNativeSnapshot(await nativePlayerLoadSubtitle(path));
				subtitleStatus = 'Subtitle downloaded and enabled.';
				return;
			}
			const url = await localMediaUrl(path);
			const label = `${result.release} · ${result.language}`;
			subtitleTracks = [...subtitleTracks, { label, language: result.language || subtitleLanguage, url }];
			selectSubtitle(subtitleTracks.length - 1);
			subtitleStatus = 'Subtitle downloaded and enabled.';
		} catch (error) { subtitleError = error instanceof Error ? error.message : 'Subtitle download failed.'; }
		finally { subtitleBusyFile = null; }
	}

	async function chooseSubtitle() {
		if (activeEngine) {
			subtitleError = '';
			try {
				const path = await chooseSubtitleFile();
				if (path) { applyNativeSnapshot(await nativePlayerLoadSubtitle(path)); subtitleStatus = 'Subtitle loaded and enabled.'; }
			} catch (error) { subtitleError = error instanceof Error ? error.message : 'Subtitle file could not be loaded.'; }
			return;
		}
		document.getElementById('player-subtitle-file-input')?.click();
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
		const target = event.target;
		const isRange = target instanceof HTMLInputElement && target.type === 'range';
		const isEditable = target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName));
		if (!mobilePlayer && !isRange && !isEditable && ['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown'].includes(event.key)) {
			event.preventDefault();
			applyVolume(volume + (event.key === 'ArrowUp' || event.key === 'PageUp' ? 1 : -1) * (event.key.startsWith('Page') ? 10 : 5), false);
			showDesktopVolumeFeedback();
			return;
		}
		revealControls();

		if (event.key === 'Escape' && subtitlePanelOpen) { subtitlePanelOpen = false; return; }
		if (event.key === 'Escape' && speedMenuOpen) { speedMenuOpen = false; return; }
		if (event.key === 'Escape' && playerMenuOpen) { playerMenuOpen = false; return; }
		if (event.key === 'Escape' && !document.fullscreenElement) void closePlayer();
		if (event.code === 'Space' && !isRange) {
			event.preventDefault();
			togglePlayback();
		}
		if (event.key === 'ArrowLeft' && !isRange) seekBy(-10);
		if (event.key === 'ArrowRight' && !isRange) seekBy(10);
		if (event.key.toLowerCase() === 'm' && !isRange) toggleMuted();
	}

	function handlePlayerPointerDown(event: PointerEvent) {
		if (playerMenuOpen && !(event.target instanceof Element && event.target.closest('.player-options-anchor'))) {
			playerMenuOpen = false;
		}
		if (speedMenuOpen && !(event.target instanceof Element && event.target.closest('.mobile-player-speed'))) speedMenuOpen = false;
	}

	function showGestureFeedback(value: typeof gestureFeedback) {
		if (gestureFeedbackTimer) clearTimeout(gestureFeedbackTimer);
		gestureFeedback = value;
		gestureFeedbackTimer = setTimeout(() => { gestureFeedback = null; }, 850);
	}

	function startMobileGesture(event: PointerEvent, side: 'brightness' | 'volume') {
		if (!event.isPrimary || event.button !== 0 || screenLocked || subtitlePanelOpen || playerMenuOpen || speedMenuOpen) return;
		const target = event.currentTarget as HTMLElement;
		target.setPointerCapture(event.pointerId);
		gesture = {pointerId: event.pointerId, side, x: event.clientX, y: event.clientY, time: performance.now(), value: side === 'brightness' ? brightness : volume, moved: false};
	}

	function moveMobileGesture(event: PointerEvent) {
		if (!gesture || gesture.pointerId !== event.pointerId) return;
		const distance = gesture.y - event.clientY;
		if (!gesture.moved && Math.abs(distance) < 10) return;
		gesture.moved = true;
		lastTap = null;
		const value = gesture.value + distance / Math.max(120, playerStage.clientHeight * .45) * 100;
		if (gesture.side === 'brightness') {
			brightness = Math.round(Math.min(100, Math.max(0, value)));
			if (!previewOnly) setPlayerLevel('brightness', brightness);
		}
		else applyVolume(value);
		showGestureFeedback(gesture.side);
	}

	function endMobileGesture(event: PointerEvent) {
		if (!gesture || gesture.pointerId !== event.pointerId) return;
		const current = gesture;
		gesture = null;
		if (current.moved || performance.now() - current.time > 500 || Math.hypot(event.clientX - current.x, event.clientY - current.y) > 18) {
			lastTap = null;
			if (mobileTapTimer) clearTimeout(mobileTapTimer);
			mobileTapTimer = undefined;
			return;
		}
		const now = performance.now();
		if (lastTap && lastTap.side === current.side && now - lastTap.time < 320 && Math.hypot(event.clientX - lastTap.x, event.clientY - lastTap.y) < 70) {
			if (mobileTapTimer) clearTimeout(mobileTapTimer);
			mobileTapTimer = undefined;
			seekBy(current.side === 'brightness' ? -10 : 10);
			showGestureFeedback(current.side === 'brightness' ? 'back' : 'forward');
			lastTap = null;
		} else {
			if (mobileTapTimer) clearTimeout(mobileTapTimer);
			lastTap = {side: current.side, time: now, x: event.clientX, y: event.clientY};
			mobileTapTimer = setTimeout(() => {
				mobileTapTimer = undefined;
				lastTap = null;
				if (screenLocked) return;
				if (controlsVisible) {
					controlsVisible = false;
					if (timeoutId) clearTimeout(timeoutId);
					timeoutId = undefined;
				} else revealControls();
			}, 320);
		}
	}

	function lockScreen() {
		screenLocked = true;
		controlsVisible = false;
		playerMenuOpen = false;
		speedMenuOpen = false;
		subtitlePanelOpen = false;
		gesture = null;
		lastTap = null;
		if (timeoutId) clearTimeout(timeoutId);
		timeoutId = undefined;
	}

	function unlockScreen() {
		screenLocked = false;
		revealControls();
	}

	function attachMobileBackButton() {
		if (!mobilePlayer || !nativeMobile) return;
		void import('@tauri-apps/api/app').then(async ({ onBackButtonPress }) => {
			const listener = await onBackButtonPress(() => {
				if (playerDisposed) return;
				if (subtitlePanelOpen) { subtitlePanelOpen = false; return; }
				if (speedMenuOpen) { speedMenuOpen = false; return; }
				if (playerMenuOpen) { playerMenuOpen = false; return; }
				void closePlayer();
			});
			if (playerDisposed) await listener.unregister();
			else mobileBackListener = listener;
		}).catch((error) => console.warn('Android back button listener unavailable', error));
	}

	function detachMobileBackButton() {
		const listener = mobileBackListener;
		mobileBackListener = undefined;
		if (listener) void listener.unregister().catch((error) => console.warn('Android back button listener cleanup failed', error));
	}

	function tryCompatiblePlayback() {
		if (!mobilePlayer || previewOnly || !canTranscode || activeTranscoding || transcoding || playerDisposed) return false;
		// Retry unsupported computer-backed media once through the converter.
		// Starting a new attempt also invalidates the failed direct play promise.
		transcoding = true;
		rememberTranscoding();
		void loadMobileStream(currentTime || resumePosition, mobilePendingResume);
		return true;
	}

	function onPlaybackError() {
		if (activeEngine) return;
		// Let hls.js recover its media errors before showing the terminal error card.
		if (mobilePlayer && activeTranscoding && compatibleStream) return;
		if(mobilePlayer && isLoading && !video?.getAttribute('src'))return;
		if (!video?.error) return;
		const errorCode = video.error.code;
		if ((errorCode === MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED || errorCode === MediaError.MEDIA_ERR_DECODE) && tryCompatiblePlayback()) return;
		if (mobilePlayer) {
			mobileMetadataAbort?.abort();
			stopCompatibleStream();
			video.pause();
			video.removeAttribute('src');
			video.load();
		}
		if (playbackError) return;
		playbackError = errorCode === MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED
			? mobilePlayer
				? activeTranscoding
					? 'The converted stream could not be opened. Update Android System WebView and try again.'
					: 'This video format is not supported on your phone.'
				: 'This file or its video codec is not supported by the built-in Windows player. Try an MP4 or WebM file, or open it in your configured desktop player.'
			: 'This media file could not be played. Check that it is still available in the library.';
		isLoading = false;
	}

	async function closePlayer() {
		if (previewOnly) { onClose(); return; }
		if (document.fullscreenElement === playerStage) {
			try { await document.exitFullscreen(); } catch { /* The stage is removed immediately after closing. */ }
		}
		await playbackActivityPromise;
		await persistProgress();
		window.dispatchEvent(new CustomEvent(PLAYBACK_HISTORY_UPDATED_EVENT));
		if (nativePoll) clearInterval(nativePoll);
		if (activeEngine) await stopNativePlayer().catch((error) => console.warn('Native player could not stop', error));
		delete document.documentElement.dataset.nativePlayer;
		suspendNativeAcrylicForPlayback(false);
		for (const track of subtitleTracks) if (track.url.startsWith('blob:')) URL.revokeObjectURL(track.url);
		onClose();
	}

	async function switchEngine() {
		playerMenuOpen = false;
		await persistProgress();
		await startSelectedEngine(currentTime);
	}

	async function loadMobileStream(position=0, restoringResume=false) {
		const attempt=++mobileLoadAttempt;
		const requestedTranscoding = transcoding;
		const requestedQuality = transcodeQuality;
		const requestedBitrate = transcodeBitrate;
		mobileMetadataAbort?.abort();
		stopCompatibleStream();
		video.pause();video.removeAttribute('src');video.load();mediaReady=false;isPlaying=false;
		const controller=new AbortController();mobileMetadataAbort=controller;
		const metadataTimeout=setTimeout(()=>controller.abort(),20000);
		let streamForAttempt: ReturnType<typeof openHlsStream> | undefined;
		playbackError='';isLoading=true;
		try {
			const url=new URL(mobileSourceUrl);
			if(requestedTranscoding){
				url.pathname += '/hls/index.m3u8';url.searchParams.set('start',String(Math.max(0,position)));
				url.searchParams.set('quality',requestedQuality);
				url.searchParams.set('bitrate',String(requestedBitrate));
				url.searchParams.set('session',mobileStreamSession);
				const metadata=await fetchCompatibleMedia(url,{method:'HEAD',cache:'no-store',signal:controller.signal},'prepare');
				if(!metadata.ok)throw new Error('Could not start transcoding on your computer. Check that Luma is updated and FFmpeg is available.');
				const length=Number(metadata.headers.get('X-Luma-Duration'));
				if(Number.isFinite(length)&&length>0)duration=length;
				if (restoringResume) {
					position = resumePlaybackPosition(position, duration);
					url.searchParams.set('start', String(position));
				}
				sourceBitrate=Number(metadata.headers.get('X-Luma-Source-Bitrate')) || 0;
			}
			if(attempt!==mobileLoadAttempt||playerDisposed)return;
			activeTranscoding=requestedTranscoding;
			mobilePendingResume=restoringResume;
			transcodeOffset=activeTranscoding?position:0;resumePosition=activeTranscoding?0:position;mobilePendingSeek=activeTranscoding?null:position;
			currentTime=position;
			if (activeTranscoding) {
				streamForAttempt = openHlsStream(video, url.toString(), error => {
					if (attempt === mobileLoadAttempt && !playerDisposed) {
						if (compatibleStream === streamForAttempt) compatibleStream = undefined;
						video.pause();video.removeAttribute('src');video.load();mediaReady=false;
						isLoading = false; playbackError = error.message;
					}
				});
				compatibleStream = streamForAttempt;
				await streamForAttempt.ready;
				if (attempt !== mobileLoadAttempt || playerDisposed) return;
				mediaReady = true;
				isLoading = false;
			} else { video.src=url.toString();video.load(); }
			video.playbackRate=playbackRate;
			try {
				const play = video.play();
				if (streamForAttempt) await withTimeout(play, 20000, 'The converted stream did not start playing in time. Try again.');
				else await play;
			} catch(error){if(error instanceof Error && error.name==='NotAllowedError'){isLoading=false;controlsVisible=true;}else throw error;}
		}catch(error){
			if(attempt===mobileLoadAttempt&&!playerDisposed){
				if (error instanceof Error && error.name === 'NotSupportedError' && tryCompatiblePlayback()) return;
				if (streamForAttempt) {
					streamForAttempt.stop();
					if (compatibleStream === streamForAttempt) compatibleStream = undefined;
					video.pause();video.removeAttribute('src');video.load();mediaReady=false;
				}
				isLoading=false;
				if (!playbackError) playbackError=error instanceof Error && error.name==='AbortError'?'Your computer took too long to prepare the stream. Try again.':error instanceof Error?error.message:String(error);
			}
		}
		finally {clearTimeout(metadataTimeout);if(mobileMetadataAbort===controller)mobileMetadataAbort=undefined;}
	}
	async function retryMobilePlayback() {
		try {
			isLoading=true;playbackError='';
			const source=await resolveMediaFile(Number(media.id),media.title,media.episodeLabel,media.tmdbId,media.kind,media.playbackUuid);
			if(playerDisposed)return;
			if(source.mediaId)media={...media,id:String(source.mediaId),playbackUuid:source.playbackUuid ?? media.playbackUuid};
			mobileSourceUrl=await localMediaUrl(source.path);
			await loadMobileStream(currentTime || source.resumePositionSeconds);
		}catch(error){isLoading=false;playbackError=error instanceof Error?error.message:String(error);}
	}
	async function toggleTranscoding(){
		if(!canTranscode)return;
		transcoding=!(previewOnly ? transcoding : activeTranscoding);playerMenuOpen=false;
		rememberTranscoding();
		if(previewOnly)return;
		await loadMobileStream(currentTime || resumePosition);
	}
	async function changeTranscodeQuality(quality: string) {
		transcodeQuality = quality;
		transcodeBitrate=bitratePresets[quality]?.[1] || 0;
		if (!canTranscode) return;
		transcoding = true;
		rememberTranscoding();
		if (previewOnly) return;
		playerMenuOpen = false;
		await loadMobileStream(currentTime || resumePosition);
	}
	async function changeTranscodeBitrate(bitrate: number) {
		transcodeBitrate=bitrate;
		if(!canTranscode)return;
		transcoding=true;playerMenuOpen=false;
		rememberTranscoding();
		if(previewOnly)return;
		await loadMobileStream(currentTime || resumePosition);
	}
	function rememberTranscoding() {
		if (mobilePlayer && !previewOnly) saveTranscodePreferences({ enabled: transcoding, quality: transcodeQuality, bitrate: transcodeBitrate });
	}

	onMount(() => {
		if (previewOnly) { isLoading = false; currentTime = 854; duration = 3120; return; }
		if (mobilePlayer) {
			const saved = readTranscodePreferences();
			transcoding = saved.enabled; transcodeQuality = saved.quality; transcodeBitrate = saved.bitrate;
		}
		playerDisposed = false;
		attachMobileBackButton();
		suspendNativeAcrylicForPlayback(true);
		selectedDesktopPlayer = readPreferredDesktopPlayer();
		const playbackPreferences = readPlaybackPreferences();
		subtitleFont = playbackPreferences.subtitleFont;
		subtitleSize = playbackPreferences.subtitleSize;
		subtitleOffset = playbackPreferences.subtitlePosition / 1.5;
		const handleFullscreenChange = () => {
			controlsVisible = true;
			revealControls();
			if (activeEngine) void resizeNativePlayer().catch(console.warn);
		};
		const handleResize = () => { if (activeEngine) void resizeNativePlayer().catch(console.warn); };
		document.addEventListener('fullscreenchange', handleFullscreenChange);
		window.addEventListener('resize', handleResize);
		overlay.focus();
		revealControls();
		if (!isDesktopRuntime()) {
			isLoading = false;
			playbackError = 'Local playback is available in the desktop app.';
			return () => { playerDisposed = true; detachMobileBackButton(); document.removeEventListener('fullscreenchange', handleFullscreenChange); window.removeEventListener('resize', handleResize); suspendNativeAcrylicForPlayback(false); };
		}
		const mediaId = Number(media.id);
		if (!Number.isSafeInteger(mediaId) || mediaId <= 0) {
			isLoading = false;
			playbackError = 'This item is not connected to a local media file.';
			return () => { playerDisposed = true; detachMobileBackButton(); document.removeEventListener('fullscreenchange', handleFullscreenChange); window.removeEventListener('resize', handleResize); suspendNativeAcrylicForPlayback(false); };
		}
		void readPlayerLevels().then(levels=>{ if(levels && !playerDisposed){volume=Math.round(levels.volume); brightness=Math.round(levels.brightness);} }).catch(console.warn);
		void resolveMediaFile(mediaId, media.title, media.episodeLabel, media.tmdbId, media.kind, media.playbackUuid)
			.then(async (source) => {
				if (playerDisposed) return;
				if(source.mediaId)media={...media,id:String(source.mediaId),playbackUuid:source.playbackUuid ?? media.playbackUuid};
				resumePosition = source.resumePositionSeconds;
				const episodeMarker = source.path.match(/(?:S\d{1,2}E\d{1,2}|\d{1,2}x\d{2})/i)?.[0];
				if (episodeMarker && media.kind === 'series') subtitleQuery = `${media.title} ${episodeMarker.toUpperCase()}`;
				subtitleWarning = source.subtitleWarning ?? '';
				const tracks = await Promise.all(source.subtitles.map(async (subtitle, index) => ({
					label: subtitle.label.replace(/\.(?:srt|vtt|ass|ssa|sub|idx)$/i, ''),
					language: subtitle.language && subtitle.language !== 'und' ? subtitle.language : subtitle.label.match(/(?:^|[. _-])(en|eng|english|pt-br|por|portuguese|pt|es|spa|fr|fre|fra|de|ger|deu|ja|jpn|ko|kor)(?=[. _-]|$)/i)?.[1]?.toLowerCase().replace('english','en').replace('portuguese','pt') ?? 'und',
					url: subtitle.supported === false ? '' : /^https?:/i.test(subtitle.path) || /\.vtt$/i.test(subtitle.path) ? await localMediaUrl(subtitle.path) : '',
					path: subtitle.path,
					streamIndex: subtitle.streamIndex,
					supported: subtitle.supported,
					lazyIndex: subtitle.supported !== false && !/^https?:/i.test(subtitle.path) && !/\.vtt$/i.test(subtitle.path) ? index : undefined
				})));
				if (playerDisposed) {
					for (const track of tracks) if (track.url.startsWith('blob:')) URL.revokeObjectURL(track.url);
					return;
				}
				subtitleTracks = tracks;
				activeSubtitle = -1;
				const preferred = preferredSubtitleIndex(tracks);
				if (import.meta.env.VITE_LUMA_MOBILE === 'true') {
					if (preferred >= 0 && tracks[preferred].supported !== false) void selectSubtitle(preferred);
				} else if (preferred >= 0 && tracks[preferred].streamIndex == null) activeSubtitle = preferred;
				await tick();
				if (import.meta.env.VITE_LUMA_MOBILE === 'true') {
					mobileSourceUrl = await localMediaUrl(source.path);
					// Files stored on this phone have no computer transcoder.
					// Keep the saved preference for the next computer-backed video.
					if (!canTranscode) transcoding = false;
					await loadMobileStream(resumePosition, true);
				} else await startSelectedEngine();
			})
			.catch((error) => {
				if (playerDisposed) return;
				isLoading = false;
				playbackError = error instanceof Error ? error.message : typeof error === 'string' ? error : 'The media file could not be opened.';
			});
		return () => {
			playerDisposed = true;
			detachMobileBackButton();
			mobileMetadataAbort?.abort();
			compatibleStream?.stop();
			mobileLoadAttempt++;
			resetPlayerLevels();
			document.removeEventListener('fullscreenchange', handleFullscreenChange);
			window.removeEventListener('resize', handleResize);
			if (timeoutId) clearTimeout(timeoutId);
			if (mobileTapTimer) clearTimeout(mobileTapTimer);
			if (nativeLoadingTimeout) clearTimeout(nativeLoadingTimeout);
			if (desktopVolumeFeedbackTimer) clearTimeout(desktopVolumeFeedbackTimer);
			if (nativePoll) clearInterval(nativePoll);
			void persistProgress();
			if (activeEngine && nativeOwner === playerToken) {
				nativeOwner = null;
				void stopNativePlayer();
			}
			delete document.documentElement.dataset.nativePlayer;
			suspendNativeAcrylicForPlayback(false);
			for (const track of subtitleTracks) if (track.url.startsWith('blob:')) URL.revokeObjectURL(track.url);
		};
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<div
	class="player-overlay"
	class:player-overlay--mobile={mobilePlayer}
	class:player-overlay--preview={previewOnly}
	class:player-overlay--native={activeEngine !== null}
	bind:this={overlay}
	role="dialog"
	aria-modal="true"
	aria-label={`Player for ${media.title}`}
	tabindex="-1"
		onpointermove={(event) => { if (!mobilePlayer || event.pointerType === 'mouse') revealControls(); }}
	onpointerdown={handlePlayerPointerDown}
>
	<div class="player-stage" class:player-stage--media-ready={mediaReady} class:player-stage--native={activeEngine !== null} bind:this={playerStage} style={`--backdrop: url("${media.backdrop}")`}>
		<div class="player-stage__image" aria-label={`Preview frame for ${media.title}`} role="img"></div>
		<div class="player-stage__ambient"></div>
		<div class="player-stage__vignette"></div>
		<video
			class="player-video"
			class:player-video--hidden={!!playbackError || activeEngine !== null || (mobilePlayer && !mediaReady)}
			bind:this={video}
			playsinline
			crossorigin={mobilePlayer ? 'anonymous' : undefined}
			preload="metadata"
			aria-label={`${media.title} video`}
			style={`--caption-size: ${subtitleSize}%; --caption-font: '${subtitleFont}', sans-serif;`}
			onloadedmetadata={onLoadedMetadata}
			onloadeddata={() => (mediaReady = true)}
			ontimeupdate={onTimeUpdate}
			onplay={() => { isPlaying = true; revealControls(); recordPlaybackStarted(); }}
			onpause={onPlaybackPause}
			onended={() => { void handlePlaybackEnded(); }}
			onerror={onPlaybackError}
		>
			{#each subtitleTracks as track, index (index)}
				<track kind="subtitles" src={track.url || undefined} srclang={track.language} label={track.label} onload={(event) => onTrackLoad(event, index)} onerror={() => { if (activeSubtitle === index) subtitleError = 'This subtitle could not be loaded. Try another track.'; }} />
			{/each}
		</video>
		{#if mobilePlayer && !playbackError && !isLoading}
			<div class="mobile-player-dimmer" style={`opacity: ${nativeMobile && !previewOnly ? 0 : (100 - brightness) / 100 * .85}`}></div>
			<div class="mobile-player-gestures" aria-label="Player gestures">
				{#each ['brightness', 'volume'] as side}
					<button type="button" class="mobile-player-gesture" aria-label={side === 'brightness' ? 'Swipe up or down for brightness; double tap to go back ten seconds' : 'Swipe up or down for volume; double tap to go forward ten seconds'} onpointerdown={(event) => startMobileGesture(event, side as 'brightness' | 'volume')} onpointermove={moveMobileGesture} onpointerup={endMobileGesture} onpointercancel={() => { gesture = null; lastTap = null; if (mobileTapTimer) clearTimeout(mobileTapTimer); mobileTapTimer = undefined; }}>
						<span class="mobile-player-level" class:mobile-player-level--visible={controlsVisible || gestureFeedback === side} aria-hidden="true"><Icon name={side === 'brightness' ? 'sun' : 'volume'} size={17} /><span class="mobile-player-level__rail"><span style={`height: ${side === 'brightness' ? brightness : isMuted ? 0 : volume}%`}></span></span></span>
					</button>
				{/each}
			</div>
			{#if gestureFeedback}
				<div class="mobile-player-feedback" class:mobile-player-feedback--left={gestureFeedback === 'brightness' || gestureFeedback === 'back'} role="status">
					<Icon name={gestureFeedback === 'brightness' ? 'sun' : gestureFeedback === 'volume' ? 'volume' : gestureFeedback === 'back' ? 'rewind-ten' : 'forward-ten'} size={24} />
					<span>{gestureFeedback === 'brightness' ? `${brightness}%` : gestureFeedback === 'volume' ? `${volume}%` : '10 s'}</span>
				</div>
			{/if}
		{/if}

		{#if isLoading}
			<div class="player-message" role="status"><span class="player-loading__spinner"></span><span>Opening media…</span></div>
		{:else if playbackError}
			{#if mobilePlayer}
				<div class="player-message player-message--mobile-error" role="alert"><Icon name="info" size={26}/><strong>Couldn’t play this video</strong><span>{playbackError}</span><button type="button" onclick={retryMobilePlayback}>Try again</button><button type="button" onclick={closePlayer}><Icon name="arrow-left" size={16}/>Go back</button></div>
			{:else}
				<div class="player-message player-message--error" role="alert"><strong>Playback unavailable</strong><span>{playbackError}</span><span>Choose another engine in the menu at the top right.</span></div>
			{/if}
		{/if}
		{#if !mobilePlayer && desktopVolumeFeedback}
			<div class="desktop-volume-feedback" role="status" aria-live="polite"><Icon name="volume" size={20}/><strong>{volume}%</strong></div>
		{/if}

		<div class:player-ui--hidden={!controlsVisible} class="player-ui">
			<div class="player-topbar">
				<button class="player-glass-button" type="button" style="corner-shape: squircle" aria-label="Close player"  onclick={closePlayer}>
					<Icon name={mobilePlayer ? 'arrow-left' : 'close'} size={20} />
				</button>

				<div class="player-now-playing">
					<span>{media.kind === 'series' ? 'Series' : 'Movie'} · {media.year}</span>
					<strong>{media.title}{media.episodeLabel ? ` · ${media.episodeLabel}` : ''}</strong>
				</div>

				<div class="player-options-anchor">
					<button class="player-glass-button" type="button" style="corner-shape: squircle" aria-label="More playback options"  aria-expanded={playerMenuOpen} aria-haspopup="true" onclick={() => (playerMenuOpen = !playerMenuOpen)}>
						<Icon name="more" size={20} />
					</button>
					{#if playerMenuOpen}
						{#if mobilePlayer}
							<div class="player-options" role="group" aria-label="Playback options">
								<button type="button" role="switch" aria-checked={previewOnly ? transcoding : activeTranscoding} disabled={!canTranscode || isLoading} onclick={toggleTranscoding}>Transcoding <span>{(previewOnly ? transcoding : activeTranscoding)?'On':'Off'}</span></button>
								<p>{canTranscode?'Convert unsupported formats on your computer.':'Transcoding is available for media streamed from your computer.'}</p>
								{#if canTranscode}
									<span class="player-options__label">Quality</span>
									<AppSelect value={transcodeQuality} label="Streaming quality" options={qualityOptions} disabled={isLoading} onchange={changeTranscodeQuality}/>
									{#if bitrateOptions.length}<span class="player-options__label">Video bitrate limit</span><AppSelect value={transcodeBitrate} label="Video bitrate limit" options={bitrateOptions} disabled={isLoading} onchange={changeTranscodeBitrate}/>{/if}
									{#if sourceBitrate > 0}<p>Original bitrate: {(sourceBitrate / 1_000_000).toFixed(1)} Mbps</p>{/if}
								{/if}
								{#if audioTracks.length>1}<AppSelect value={activeAudio} label="Audio track" options={audioTracks.map(track=>({value:track.index,label:track.label}))} onchange={selectAudio}/>{/if}
							</div>
						{:else}
						<div class="player-options" role="group" aria-label="Playback engine selection">
							<span class="player-options__label">Playback engine</span>
							<AppSelect bind:value={selectedDesktopPlayer} label="Playback engine" options={[{value:"mpv",label:"libmpv"},{value:"vlc",label:"libVLC"}]} onchange={setDesktopPlayer} />
							<p>{selectedDesktopPlayer === 'mpv' ? 'Uses libmpv with its GPU video renderer.' : 'Uses libVLC inside this player.'}</p>
							<button type="button" disabled={desktopPlayerBusy || selectedDesktopPlayer === activeEngine} onclick={switchEngine}>{desktopPlayerBusy ? 'Switching…' : `Use ${desktopPlayerLabel()}`}</button>
						</div>
						{/if}
					{/if}
				</div>
			</div>

			{#if mobilePlayer && !playbackError && !isLoading}
				<div class="mobile-player-transport">
					<button class="mobile-player-skip" type="button" aria-label="Go back ten seconds" onclick={() => seekBy(-10)}><Icon name="rewind-ten" size={30} /><span>10</span></button>
					<button class="mobile-player-play" type="button" aria-label={isPlaying ? 'Pause' : 'Play'} onclick={togglePlayback}><Icon name={isPlaying ? 'pause' : 'play'} size={30} weight="fill" /></button>
					<button class="mobile-player-skip" type="button" aria-label="Forward ten seconds" onclick={() => seekBy(10)}><Icon name="forward-ten" size={30} /><span>10</span></button>
				</div>
			{/if}
			{#if !mobilePlayer || (!playbackError && !isLoading)}<div class={mobilePlayer ? 'mobile-player-control-deck' : 'player-control-deck'} style="corner-shape: squircle">
				<div class="player-timeline">
					<div class="player-timeline__meta"><span>{elapsedLabel}</span><span>{remainingLabel}</span></div>
					<input
						class="player-range"
						type="range"
						min="0"
						max="100"
						step="0.1"
						value={progressPercent}
						oninput={previewSeek}
						onchange={commitSeek}
						style={`--player-progress: ${progressPercent}%`}
						aria-label="Playback position"
						aria-valuetext={`${elapsedLabel} elapsed, ${remainingLabel} remaining`}
					/>
				</div>

				{#if mobilePlayer}
					<div class="mobile-player-tools">
						<button type="button" class:active={subtitlePanelOpen} onclick={() => { subtitlePanelOpen = !subtitlePanelOpen; playerMenuOpen = false; speedMenuOpen = false; }}><Icon name="captions" size={21} /><span>Subtitles</span></button>
						<div class="mobile-player-speed">
							<button type="button" class:active={speedMenuOpen} aria-expanded={speedMenuOpen} aria-label={`Speed: ${playbackRate}×`} onclick={() => { speedMenuOpen = !speedMenuOpen; playerMenuOpen = false; subtitlePanelOpen = false; }}><span class="mobile-player-speed__value">{playbackRate}×</span><span>Speed</span></button>
							{#if speedMenuOpen}<div class="mobile-player-speed__menu" role="group" aria-label="Playback speed">{#each [0.75,1,1.25,1.5,2] as rate}<button type="button" class:active={playbackRate === rate} aria-pressed={playbackRate === rate} onclick={() => { setPlaybackRate(rate); speedMenuOpen = false; }}>{rate}×</button>{/each}</div>{/if}
						</div>
						<button type="button" onclick={toggleFullscreen}><Icon name="fullscreen" size={21} /><span>Fullscreen</span></button>
						<button type="button" aria-label="Lock screen touch controls" onclick={lockScreen}><Icon name="lock" size={21} /><span>Lock screen</span></button>
					</div>
				{:else}
				<div class="player-transport">
					<div class="player-transport__group">
						<button class="player-primary-button" type="button" style="corner-shape: squircle" aria-label={isPlaying ? 'Pause' : 'Play'}  onclick={togglePlayback}>
							<Icon name={isPlaying ? 'pause' : 'play'} size={19} weight="fill" />
						</button>
						<button class="player-control-button" type="button" style="corner-shape: squircle" aria-label="Go back ten seconds"  onclick={() => seekBy(-10)}>
							<Icon name="arrow-left" size={18} />
						</button>
						<button class="player-control-button" type="button" style="corner-shape: squircle" aria-label="Forward ten seconds"  onclick={() => seekBy(10)}>
							<Icon name="arrow-left" size={18} mirrored />
						</button>
						<div class="player-volume" style="corner-shape: squircle">
							<button class="player-volume__button" type="button" aria-label={isMuted ? 'Unmute' : 'Mute'}  onclick={toggleMuted}>
								<Icon name="volume" size={18} />
							</button>
							<input class="player-volume__range" type="range" min="0" max="100" value={volume} style={`--volume: ${volume}%`} aria-label="Volume" oninput={setVolume} />
						</div>
					</div>

					<div class="player-transport__group player-transport__group--end">
						<button class="player-chip" type="button" style="corner-shape: squircle" aria-label="Subtitles"  onclick={() => (subtitlePanelOpen = !subtitlePanelOpen)}><Icon name="captions" size={17} /><span>Subtitles</span></button>
						{#if audioTracks.length > 1}
							<div class="player-chip"><AppSelect value={activeAudio} label="Audio track" variant="plain" options={audioTracks.map(track => ({value:track.index,label:track.label}))} onchange={selectAudio} /></div>
						{/if}
						<div class="player-chip player-rate" style="corner-shape: squircle"><span class="player-rate__caption">Speed</span><AppSelect value={playbackRate} label="Playback speed" variant="plain" options={[0.75,1,1.25,1.5,2].map(rate => ({value:rate,label:`${rate}×`}))} onchange={setPlaybackRate} /></div>
						<button class="player-control-button" type="button" style="corner-shape: squircle" aria-label="Toggle fullscreen"  onclick={toggleFullscreen}><Icon name="fullscreen" size={18} /></button>
					</div>
				</div>
				{/if}
			</div>{/if}
		</div>
		{#if mobilePlayer && screenLocked}
			<div class="mobile-player-lock-shield">
				<button type="button" onclick={unlockScreen} aria-label="Unlock screen controls"><Icon name="unlock" size={18} /><span>Screen locked · Tap to unlock</span></button>
			</div>
		{/if}

		{#if !isPlaying && !mobilePlayer && !isLoading && !activeEngine}
			{#if !playbackError}<button class="player-center-play" type="button" style="corner-shape: squircle" aria-label="Play" onclick={togglePlayback}>
				<Icon name="play" size={28} weight="fill" />
			</button>{/if}
		{/if}

		{#if subtitlePanelOpen}
			<section class="subtitle-panel" aria-label="Subtitle settings" style="corner-shape: squircle">
				<header class="subtitle-panel__header"><strong><Icon name="captions" size={17} /> Subtitles</strong><button type="button" aria-label="Close subtitle settings" onclick={() => (subtitlePanelOpen = false)}><Icon name="close" size={16} /></button></header>
				<div class="subtitle-panel__body">
					<section class="subtitle-panel__section" aria-label="Subtitle track">
						<h3>Subtitle track</h3>
						<div class="subtitle-panel__tracks">
							{#if activeEngine}
								<button type="button" class:active={!nativeSubtitleTracks.some((track) => track.selected)} aria-pressed={!nativeSubtitleTracks.some((track) => track.selected)} onclick={() => selectNativeSubtitle(-1)}><span>Off</span>{#if !nativeSubtitleTracks.some((track) => track.selected)}<Icon name="check" size={14} />{/if}</button>
								{#each nativeSubtitleTracks as track (track.id)}<button type="button" class:active={track.selected} aria-pressed={track.selected} onclick={() => selectNativeSubtitle(track.id)}><span>{track.label}{track.language && track.language.toLowerCase() !== track.label.toLowerCase() ? ` · ${track.language}` : ''}</span>{#if track.selected}<Icon name="check" size={14} />{/if}</button>{/each}
								{#each subtitleTracks.filter((track) => track.path && track.streamIndex == null) as track (track.path)}<button type="button" onclick={() => selectNativeExternalSubtitle(track)}>{track.label}</button>{/each}
							{:else}
								<button type="button" class:active={activeSubtitle === -1} aria-pressed={activeSubtitle === -1} onclick={() => selectSubtitle(-1)}><span>Off</span>{#if activeSubtitle === -1}<Icon name="check" size={14} />{/if}</button>
								{#each subtitleTracks as track, index}<button type="button" disabled={track.supported === false} class:active={activeSubtitle === index} aria-pressed={activeSubtitle === index} onclick={() => selectSubtitle(index)}><span>{track.label}{track.supported === false ? ' · Not supported by this player' : ''}</span>{#if activeSubtitle === index}<Icon name="check" size={14} />{/if}</button>{/each}
							{/if}
						</div>
						{#if subtitleWarning && !activeEngine}<p class="subtitle-online__error" role="status">{subtitleWarning}</p>{/if}
						{#if subtitleError}<p class="subtitle-online__error" role="alert">{subtitleError}</p>{/if}
					</section>
					<section class="subtitle-panel__section subtitle-panel__appearance" aria-label="Subtitle appearance">
						<h3>Appearance</h3>
						<label><span class="subtitle-panel__setting-label">Text size <output>{subtitleSize}%</output></span><SteppedRange min={70} max={150} step={10} bind:value={subtitleSize} oninput={changeSubtitleSize} ariaLabel="Subtitle text size" disabled={activeEngine === 'vlc'} /></label>
						<label><span class="subtitle-panel__setting-label">Vertical position <output>{subtitleOffset === 0 ? 'Default' : `${subtitleOffset} / 8`}</output></span><SteppedRange min={0} max={8} step={1} bind:value={subtitleOffset} oninput={changeSubtitleOffset} ariaLabel="Subtitle vertical position" disabled={activeEngine === 'vlc'} /></label>
						{#if vlcSubtitleNote}<p class="subtitle-panel__note">VLC styling preferences apply on the next open; use libmpv for live size and position adjustments.</p>{/if}
					</section>
					<div class="subtitle-panel__actions">
						<button type="button" onclick={chooseSubtitle}><Icon name="captions" size={15} /> Load subtitle file</button>
						<input id="player-subtitle-file-input" class="sr-only" type="file" accept=".srt,.vtt,text/vtt,application/x-subrip" onchange={loadSubtitleFile} />
					</div>
					<details class="subtitle-online">
						<summary><Icon name="search" size={15} /><span>Find on OpenSubtitles</span><span class="subtitle-online__chevron"><Icon name="chevron-down" size={14} /></span></summary>
						<div class="subtitle-online__content">
							<label class="subtitle-online__field">API key <input type="password" bind:value={subtitleApiKey} autocomplete="off" placeholder="OpenSubtitles API key" /></label>
							<label class="subtitle-online__field">Title <input bind:value={subtitleQuery} maxlength="120" aria-label="Subtitle search title" /></label>
							<div class="subtitle-online__field">Language
								<AppSelect bind:value={subtitleLanguage} label="Subtitle language" options={[{value:"en",label:"English"},{value:"pt-br",label:"Portuguese (Brazil)"},{value:"es",label:"Spanish"},{value:"fr",label:"French"},{value:"ja",label:"Japanese"},{value:"ko",label:"Korean"},{value:"de",label:"German"}]} />
							</div>
							<button class="subtitle-online__button subtitle-online__button--primary" type="button" disabled={subtitleSearchBusy} onclick={findOpenSubtitles}>{subtitleSearchBusy ? 'Searching…' : 'Search subtitles'}</button>
							{#if subtitleStatus}<p class="subtitle-online__status" role="status">{subtitleStatus}</p>{/if}
							{#if subtitleResults.length > 0}
								<div class="subtitle-online__results" aria-label="Subtitle results">
									{#each subtitleResults as result (result.fileId)}
										<div><span><strong>{result.release}</strong><small>{result.language}{result.hearingImpaired ? ' · SDH' : ''} · {result.downloads} downloads</small></span><button type="button" aria-label={`Download ${result.release}`} disabled={subtitleBusyFile === result.fileId} onclick={() => downloadSubtitle(result)}>{subtitleBusyFile === result.fileId ? '…' : 'Get'}</button></div>
									{/each}
								</div>
							{/if}
							<details class="subtitle-online__credentials">
								<summary><span>Sign in to download</span><span class="subtitle-online__chevron"><Icon name="chevron-down" size={14} /></span></summary>
								<div class="subtitle-online__credentials-content">
									<div class="subtitle-online__row">
										<label class="subtitle-online__field">Username <input bind:value={subtitleUsername} autocomplete="username" placeholder="Username" /></label>
										<label class="subtitle-online__field">Password <input type="password" bind:value={subtitlePassword} autocomplete="current-password" placeholder="Password" /></label>
									</div>
									<button class="subtitle-online__button" type="button" onclick={signInToOpenSubtitles}>Sign in</button>
									<p class="subtitle-online__note">Your API key and sign-in stay in memory for this session. Your password is not saved.</p>
								</div>
							</details>
							<a class="subtitle-online__account" href="https://www.opensubtitles.com/" target="_blank" rel="noreferrer">OpenSubtitles account and API key ↗</a>
						</div>
					</details>
				</div>
			</section>
		{/if}
	</div>
</div>

<style>
	.player-overlay { position: fixed; inset: 0; z-index: 80; border: 0; color: #f4f6f7; background: #020304; outline: none; box-shadow: none; }
	.player-overlay--native { background: transparent; }
	.player-stage--native .player-stage__image, .player-stage--native .player-stage__ambient, .player-stage--native .player-stage__vignette { display: none; }
	.player-stage { position: relative; width: 100%; height: 100%; overflow: hidden; isolation: isolate; border: 0; background: #000; }
	.player-stage::before { position:absolute; top:0; right:0; left:0; z-index:2; height:1px; background:#020304; content:""; pointer-events:none; }
	.player-stage:fullscreen { width: 100vw; height: 100vh; background: #000; }
	.player-stage.player-stage--native, .player-stage.player-stage--native:fullscreen { background: transparent; }
	.player-stage__image { position: absolute; inset: -2.5%; background-image: var(--backdrop); background-position: center; background-size: cover; filter: saturate(0.8) contrast(1.06) brightness(0.74); transform: scale(1.025); z-index: -3; transition: opacity 320ms ease, visibility 0s; }
	.player-stage__ambient { position: absolute; inset: 0; background: radial-gradient(circle at 70% 42%, rgba(151, 193, 211, 0.13), transparent 32%), radial-gradient(circle at 18% 78%, rgba(45, 65, 76, 0.24), transparent 34%); mix-blend-mode: screen; pointer-events: none; z-index: -2; transition: opacity 320ms ease, visibility 0s; }
	.player-stage__vignette { position: absolute; inset: 0; background: radial-gradient(ellipse 82% 72% at 50% 48%, transparent 24%, rgba(2, 4, 6, 0.34) 72%, rgba(2, 4, 6, 0.78) 100%), linear-gradient(180deg, rgba(2,4,6,0.58), transparent 25%, transparent 57%, rgba(2,4,6,0.74)); pointer-events: none; z-index: -1; transition: opacity 320ms ease, visibility 0s; }
	.player-stage--media-ready .player-stage__image,
	.player-stage--media-ready .player-stage__ambient,
	.player-stage--media-ready .player-stage__vignette { visibility: hidden; opacity: 0; transition: opacity 320ms ease, visibility 0s linear 320ms; }
	.player-video { position: absolute; inset: 0; z-index: 0; display: block; width: 100%; height: 100%; border: 0; background: #000; object-fit: contain; outline: none; }
	.player-video--hidden { visibility: hidden; }
	.player-video::-webkit-media-controls,
	.player-video::-webkit-media-controls-start-playback-button { display:none !important; -webkit-appearance:none; }
	.player-video::cue { color: #fff; font-family: var(--caption-font, Manrope, sans-serif); font-size: var(--caption-size, 100%); background: transparent; text-shadow: 0 1px 2px rgba(0,0,0,0.8); }
	.player-message { position: absolute; top: 50%; left: 50%; z-index: 2; display: grid; justify-items: center; gap: 12px; width: min(440px, calc(100% - 36px)); color: rgba(244,247,249,0.72); font-size: 0.78rem; text-align: center; transform: translate(-50%,-50%); }
	.desktop-volume-feedback { position:absolute; bottom:26px; left:26px; z-index:7; display:flex; align-items:center; gap:12px; padding:14px 18px; border:1px solid rgba(255,255,255,.12); border-radius:16px; color:#f4f7f9; background:rgba(14,18,22,.78); box-shadow:0 14px 42px rgba(0,0,0,.28); pointer-events:none; animation:volume-feedback-in 120ms ease-out; }
	.desktop-volume-feedback strong { min-width:3ch; font-size:1rem; font-variant-numeric:tabular-nums; text-align:right; }
	@keyframes volume-feedback-in { from { opacity:0; transform:translateY(6px); } to { opacity:1; transform:translateY(0); } }
	.player-message--error { padding: 22px 24px; border: 1px solid rgba(255,255,255,0.15); border-radius: 18px; background: rgba(13,16,20,0.72); box-shadow: 0 24px 60px rgba(0,0,0,0.38); backdrop-filter: blur(24px) saturate(135%); }
	.player-message--error strong { color: #fff; font-size: 0.95rem; font-weight: 650; }
	.player-message--mobile-error { z-index:4; width:min(340px,calc(100% - 48px)); gap:14px; padding:26px 22px; border:1px solid rgba(220,234,246,.13); border-radius:20px; background:#111b25; line-height:1.6; }
	.player-message--mobile-error strong { color:#edf3f7; font-size:1rem; }
	.player-message--mobile-error button { display:inline-flex; align-items:center; justify-content:center; gap:8px; min-height:44px; margin-top:6px; padding:0 18px; border:1px solid rgba(220,234,246,.17); border-radius:11px; background:#253440; color:#edf3f7; font:inherit; cursor:pointer; }
	.player-loading__spinner { width: 20px; height: 20px; border: 2px solid rgba(255,255,255,0.2); border-top-color: #e8f3f5; border-radius: 50%; animation: player-spin 700ms linear infinite; }
	@keyframes player-spin { to { transform: rotate(360deg); } }

	.player-ui { position: absolute; inset: 0; z-index: 3; visibility: visible; opacity: 1; pointer-events: none; transition: opacity 220ms ease, visibility 0s; }
	.player-ui--hidden { visibility: hidden; opacity: 0; transition: opacity 220ms ease, visibility 0s linear 220ms; }
	.player-ui--hidden * { pointer-events: none !important; }

	.player-topbar { position: absolute; top: 0; right: 0; left: 0; display: flex; align-items: flex-start; justify-content: space-between; padding: 22px 24px 86px; border: 0; background: linear-gradient(180deg, rgba(2,4,6,0.68), transparent); }
	.player-topbar > * { pointer-events: auto; }
	.player-options-anchor { position: relative; z-index: 6; }
	.player-options { position: absolute; top: 52px; right: 0; display: grid; gap: 9px; width: min(280px, calc(100vw - 36px)); padding: 15px; border: 1px solid rgba(255,255,255,0.14); border-radius: 16px; color: rgba(244,247,249,0.86); background: rgba(17,21,26,0.94); box-shadow: 0 18px 54px rgba(0,0,0,0.42); -webkit-backdrop-filter: blur(24px) saturate(140%); backdrop-filter: blur(24px) saturate(140%); }
	.player-options__label { color: rgba(244,247,249,0.58); font-size: 0.66rem; font-weight: 650; }
	.player-options p { margin: 0; color: rgba(244,247,249,0.56); font-size: 0.64rem; line-height: 1.45; }
	.player-options > button { min-height: 38px; padding: 0 11px; border: 1px solid rgba(255,255,255,0.15); border-radius: 10px; color: #11161b; background: rgba(240,246,248,0.94); cursor: pointer; font: inherit; font-size: 0.7rem; font-weight: 650; }
	.player-options > button:disabled { opacity: 0.55; cursor: progress; }
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
	.player-rate { position: relative; min-width: 108px; justify-content: center; cursor: pointer; }
	.subtitle-panel { position: absolute; right: 20px; bottom: 124px; z-index: 8; display: flex; flex-direction: column; width: min(380px, calc(100% - 40px)); max-height: min(680px, calc(100% - 148px)); overflow: hidden; border: 1px solid rgba(255,255,255,0.16); border-radius: 19px; color: rgba(244,247,249,0.76); background: rgba(20,24,29,0.91); box-shadow: 0 22px 66px rgba(0,0,0,0.42), inset 0 1px rgba(255,255,255,0.07); backdrop-filter: blur(34px) saturate(145%); pointer-events: auto; }
	.subtitle-panel__header { display: flex; flex: 0 0 auto; align-items: center; justify-content: space-between; padding: 14px 18px; border-bottom: 1px solid rgba(255,255,255,0.1); color: #f8fafb; font-size: 0.86rem; font-weight: 650; }
	.subtitle-panel__header strong { display: flex; align-items: center; gap: 9px; }
	.subtitle-panel__header button { display: grid; width: 28px; height: 28px; place-items: center; border: 0; border-radius: 9px; color: inherit; background: transparent; cursor: pointer; }
	.subtitle-panel__header button:hover { background: rgba(255,255,255,0.09); }
	.subtitle-panel__body { display: grid; flex: 1 1 auto; gap: 18px; min-height: 0; overflow-y: auto; overscroll-behavior: contain; padding: 18px; scrollbar-width: thin; scrollbar-color: rgba(235,240,244,0.18) transparent; }
	.subtitle-panel__body::-webkit-scrollbar { display: block !important; width: 4px !important; }
	.subtitle-panel__body::-webkit-scrollbar-thumb { border-radius: 999px; background: rgba(235,240,244,0.18); }
	.subtitle-panel__section { display: grid; gap: 10px; min-width: 0; }
	.subtitle-panel__section h3 { margin: 0; color: rgba(235,240,244,0.5); font-size: 0.62rem; font-weight: 650; letter-spacing: 0.06em; text-transform: uppercase; }
	.subtitle-panel__tracks { display: grid; gap: 4px; max-height: 145px; overflow: auto; }
	.subtitle-panel__tracks button { display: flex; align-items: center; justify-content: space-between; gap: 10px; min-width: 0; min-height: 36px; padding: 7px 10px; border: 1px solid transparent; border-radius: 9px; color: rgba(244,247,249,0.68); background: transparent; cursor: pointer; font: inherit; font-size: 0.73rem; text-align: left; }
	.subtitle-panel__tracks button span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.subtitle-panel__tracks button:hover { background: rgba(255,255,255,0.06); }
	.subtitle-panel__tracks button.active { border-color: rgba(255,255,255,0.12); color: #fff; background: rgba(255,255,255,0.1); }
	.subtitle-panel__tracks button:disabled { opacity: .55; cursor: default; }
	.subtitle-panel__appearance { gap: 12px; padding-top: 16px; border-top: 1px solid rgba(255,255,255,0.1); }
	.subtitle-panel__appearance label { display: grid; gap: 5px; font-size: 0.68rem; }
	.subtitle-panel__setting-label { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
	.subtitle-panel__setting-label output { color: rgba(235,240,244,0.5); font-size: 0.65rem; font-variant-numeric: tabular-nums; }
	.subtitle-panel__note { margin: 0; color: rgba(235,240,244,0.55); font-size: 0.64rem; line-height: 1.5; }
	.subtitle-panel__actions { display: grid; gap: 7px; }
	.subtitle-panel__actions button { display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 38px; padding: 0 11px; border: 1px solid rgba(255,255,255,0.12); border-radius: 10px; color: #f1f5f6; background: rgba(255,255,255,0.06); cursor: pointer; font: inherit; font-size: 0.7rem; font-weight: 600; }
	.subtitle-panel__actions button:hover:not(:disabled) { background: rgba(255,255,255,0.12); }
	.subtitle-panel__actions button:disabled { opacity: 0.45; cursor: not-allowed; }
	.subtitle-online { min-width: 0; padding-top: 16px; border-top: 1px solid rgba(255,255,255,0.1); }
	.subtitle-online summary { display: flex; align-items: center; gap: 8px; color: #f1f5f6; cursor: pointer; font-size: 0.72rem; font-weight: 640; list-style: none; }
	.subtitle-online summary::-webkit-details-marker { display: none; }
	.subtitle-online__chevron { display: flex; margin-left: auto; color: rgba(235,240,244,0.5); transition: transform 150ms ease; }
	.subtitle-online[open] > summary > .subtitle-online__chevron, .subtitle-online__credentials[open] > summary > .subtitle-online__chevron { transform: rotate(180deg); }
	.subtitle-online__content { display: grid; gap: 14px; padding-top: 16px; }
	.subtitle-online__note { margin: 0; color: rgba(235,240,244,0.5); font-size: 0.64rem; line-height: 1.5; }
	.subtitle-online__field { display: grid; gap: 6px; min-width: 0; color: rgba(244,247,249,0.66); font-size: 0.66rem; }
	.subtitle-online input { width: 100%; min-width: 0; height: 38px; padding: 0 11px 2px; border: 1px solid rgba(255,255,255,0.12); border-radius: 9px; outline: none; color: #f2f5f7; background: rgba(5,8,11,0.38); font: inherit; font-size: 0.7rem; line-height: 1.2; }
	.subtitle-online input:focus { border-color: rgba(225,242,248,0.38); }
	.subtitle-online input::placeholder { color: rgba(235,240,244,0.4); }
	.subtitle-online__row { display: grid; grid-template-columns: minmax(0,1fr) minmax(0,1fr); gap: 10px; }
	.subtitle-online__button { width: 100%; min-height: 38px; padding: 0 11px; border: 1px solid rgba(255,255,255,0.13); border-radius: 9px; color: rgba(244,247,249,0.8); background: rgba(255,255,255,0.055); cursor: pointer; font: inherit; font-size: 0.7rem; font-weight: 620; }
	.subtitle-online__button:hover:not(:disabled) { background: rgba(255,255,255,0.11); }
	.subtitle-online__button:disabled { opacity: 0.48; cursor: wait; }
	.subtitle-online__button--primary { color: #101418; background: rgba(232,243,245,0.94); }
	.subtitle-online__button--primary:hover:not(:disabled) { background: #fff; }
	.subtitle-online__credentials { padding-top: 14px; border-top: 1px solid rgba(255,255,255,0.1); }
	.subtitle-online__credentials > summary { color: rgba(244,247,249,0.7); font-size: 0.68rem; font-weight: 600; }
	.subtitle-online__credentials-content { display: grid; gap: 12px; padding-top: 14px; }
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
	.subtitle-online__account { display: block; justify-self: start; color: rgba(235,240,244,0.54); font-size: 0.63rem; line-height: 1.5; text-decoration: underline; text-underline-offset: 3px; }
	.subtitle-online__account:hover { color: #f1f5f6; }
	.subtitle-panel :global(button:focus-visible), .subtitle-panel :global(summary:focus-visible), .subtitle-panel :global(a:focus-visible) { outline: 2px solid var(--accent); outline-offset: 3px; }
	.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; clip-path: inset(50%); }

	@media (max-width: 760px) {
		.player-topbar { padding: 14px 12px 70px; }
		.player-options { top: 48px; right: 0; }
		.player-now-playing { top: 16px; width: min(56vw, 300px); }
		.player-glass-button { width: 40px; height: 40px; border-radius: 13px; }
		.player-control-deck { right: 10px; bottom: max(10px, env(safe-area-inset-bottom)); left: 10px; padding: 12px; border-radius: 18px; }
		.player-chip { width: 38px; padding: 0; }
		.player-chip span { display: none; }
		.player-volume__range { width: 54px; }
		.player-center-play { width: 66px; height: 66px; border-radius: 20px; }
		.subtitle-panel { right: 12px; bottom: 118px; width: min(380px, calc(100% - 24px)); max-height: min(680px, calc(100% - 142px)); }
	}

	@media (max-width: 520px) {
		.player-volume { padding-right: 4px; }
		.player-volume__range { display: none; }
		.player-transport { gap: 8px; }
		.player-transport__group { gap: 2px; }
		.player-chip:nth-child(2),
		.player-chip:nth-child(3) { display: none; }
		.subtitle-online__row { grid-template-columns: minmax(0,1fr); }
	}

	/* Handset controls are laid out for touch, independently of the desktop deck. */
	.player-overlay--mobile .player-topbar { align-items: center; gap: 14px; padding: max(18px, env(safe-area-inset-top)) 18px 80px; }
	.player-overlay--mobile .player-now-playing { position: static; flex: 1; min-width: 0; width: auto; text-align: left; transform: none; }
	.player-overlay--mobile .player-now-playing strong { display: block; overflow: hidden; font-size: .9rem; line-height: 1.4; text-overflow: ellipsis; white-space: nowrap; }
	.player-overlay--mobile .player-now-playing > span { display: block; margin-bottom: 3px; font-size: .65rem; color: rgba(229,238,244,.55); }
	.player-overlay--mobile .player-glass-button { flex: 0 0 auto; width: 44px; height: 44px; border-radius: 50%; background: rgba(27,37,47,.46); }
	/* Keep the handset deck out of the desktop glass/compositing rules entirely. */
	.mobile-player-control-deck { position: absolute; left: 0; right: 0; bottom: 0; margin: 0; padding: 64px 22px max(22px, env(safe-area-inset-bottom)); border: 0; border-radius: 0; background: transparent; box-shadow: none; backdrop-filter: none; -webkit-backdrop-filter: none; pointer-events: auto; text-shadow: 0 1px 4px #000, 0 0 12px #000; }
	/* Android WebView must not create backdrop-filter surfaces over live video. */
	.player-overlay--mobile :global(*) { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }
	.player-overlay--mobile .player-timeline { gap: 0; }
	.player-overlay--mobile .player-timeline__meta { order: 2; font-size: .69rem; color: rgba(237,242,247,.68); }
	.player-overlay--mobile .player-range { height: 36px; touch-action: none; }
	.player-overlay--mobile .player-range::-webkit-slider-thumb { width: 18px; height: 18px; margin-top: -7px; }
	.player-overlay--mobile .player-range::-moz-range-thumb { width: 18px; height: 18px; }
	.mobile-player-transport { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; gap: clamp(22px,7vw,44px); pointer-events: none; }
	.mobile-player-transport button { display: grid; position: relative; place-items: center; padding: 0; border: 1px solid rgba(228,239,247,.15); cursor: pointer; pointer-events: auto; }
	.mobile-player-play { width: 70px; height: 70px; border-radius: 50%; color: #101820; background: rgba(236,244,248,.95); box-shadow: 0 8px 30px rgba(0,0,0,.25); }
	.mobile-player-skip { width: 54px; height: 54px; border-radius: 50%; color: #edf4f7; background: rgba(20,31,42,.52); backdrop-filter: blur(16px); }
	.mobile-player-skip span { position: absolute; top: 23px; font-size: 9px; font-weight: 750; line-height: 1; }
	.mobile-player-tools { display: grid; grid-template-columns: repeat(4,minmax(0,1fr)); gap: 4px; margin-top: 22px; }
	.mobile-player-tools button { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 7px; min-width: 0; min-height: 54px; border: 0; border-radius: 12px; color: rgba(230,239,246,.7); background: transparent; font: inherit; font-size: .64rem; cursor: pointer; }
	.mobile-player-tools button.active { color: #e7f5fc; background: rgba(146,184,204,.12); }
	.mobile-player-speed { position: relative; min-width: 0; }
	.mobile-player-speed > button { width: 100%; }
	.mobile-player-speed__value { font-size: 1rem; line-height: 21px; font-weight: 650; }
	.mobile-player-speed__menu { position: absolute; bottom: calc(100% + 12px); left: 50%; transform: translateX(-50%); width: 150px; padding: 6px; border: 1px solid rgba(228,239,247,.15); border-radius: 14px; background: rgba(17,27,38,.96); backdrop-filter: blur(20px); box-shadow: 0 12px 32px rgba(0,0,0,.3); }
	.mobile-player-speed__menu button { width: 100%; min-height: 44px; font-size: .8rem; }
	.mobile-player-dimmer { position: absolute; inset: 0; z-index: 1; background: #000; pointer-events: none; }
	.mobile-player-gestures { position: absolute; inset: 0; z-index: 2; display: grid; grid-template-columns: 1fr 1fr; }
	.mobile-player-lock-shield { position: absolute; inset: 0; z-index: 20; display: flex; align-items: flex-start; justify-content: flex-end; padding: max(18px,env(safe-area-inset-top)) 18px 18px; background: transparent; touch-action: manipulation; }
	.mobile-player-lock-shield button { display: flex; align-items: center; gap: 9px; min-height: 44px; padding: 0 14px; border: 1px solid rgba(228,239,247,.16); border-radius: 999px; color: #edf4f7; background: rgba(17,27,38,.8); backdrop-filter: blur(16px); font: inherit; font-size: .75rem; cursor: pointer; }
	.mobile-player-gesture { position: relative; min-width: 0; padding: 0; border: 0; background: transparent; touch-action: none; user-select: none; -webkit-tap-highlight-color: transparent; }
	.mobile-player-level { position: absolute; top: 50%; left: 15px; display: flex; flex-direction: column; align-items: center; gap: 12px; color: rgba(235,243,249,.7); transform: translateY(-50%); opacity: 0; transition: opacity 150ms; pointer-events: none; }
	.mobile-player-gesture:last-child .mobile-player-level { left: auto; right: 15px; }
	.mobile-player-level--visible { opacity: 1; }
	.mobile-player-level__rail { position: relative; width: 3px; height: 90px; overflow: hidden; border-radius: 8px; background: rgba(234,243,249,.18); }
	.mobile-player-level__rail > span { position: absolute; bottom: 0; width: 100%; border-radius: inherit; background: rgba(234,243,249,.85); }
	.mobile-player-feedback { position: absolute; top: 35%; right: 20%; z-index: 5; display: flex; align-items: center; gap: 10px; padding: 12px 16px; border-radius: 16px; background: rgba(17,27,38,.85); color: #edf4f7; pointer-events: none; }
	.mobile-player-feedback--left { right: auto; left: 20%; }
	.player-overlay--mobile .player-options { top: 54px; width: min(280px,calc(100vw - 36px)); padding: 18px; gap: 12px; background: rgba(17,27,38,.96); }
	.player-overlay--mobile .player-options > button { min-height: 44px; }
	.player-overlay--mobile .subtitle-panel { left: 10px; right: 10px; bottom: max(10px,env(safe-area-inset-bottom)); width: auto; max-height: min(75dvh,640px); border-radius: 22px; background: rgba(16,25,35,.96); }
	.player-overlay--mobile .subtitle-panel__header { padding: 16px 18px; }
	.player-overlay--mobile .subtitle-panel__header button { width: 40px; height: 40px; }
	.player-overlay--mobile .subtitle-panel__tracks button,
	.player-overlay--mobile .subtitle-panel__actions button { min-height: 44px; }
	.player-overlay--mobile .subtitle-panel__appearance :global(input[type='range']) { min-height: 40px; }
	.player-overlay--preview .player-video { display: none; }
	.player-overlay--preview .player-stage__image { inset: 0; background-size: contain; background-repeat: no-repeat; filter: brightness(.8); transform: none; }
	@media (orientation: landscape) and (max-height: 520px) {
		.player-overlay--mobile .player-topbar { padding: max(12px,env(safe-area-inset-top)) 20px 50px; }
		.mobile-player-control-deck { padding: 30px 24px max(12px,env(safe-area-inset-bottom)); }
		.mobile-player-tools { display: flex; justify-content: flex-end; margin-top: 8px; gap: 14px; }
		.mobile-player-tools button { flex-direction: row; min-height: 40px; padding: 0 8px; }
		.mobile-player-play { width: 60px; height: 60px; }
		.player-overlay--mobile .subtitle-panel { left: auto; width: min(380px,calc(100% - 20px)); max-height: calc(100dvh - 20px); }
	}
	@media (prefers-reduced-transparency: reduce) {
		.player-control-deck,
		.player-glass-button { background: #161a1f; -webkit-backdrop-filter: none; backdrop-filter: none; }
		.mobile-player-control-deck { background: transparent; box-shadow: none; -webkit-backdrop-filter: none; backdrop-filter: none; }
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
