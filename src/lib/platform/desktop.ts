import type { CatalogMedia, ContinueWatchingItem, LocalTitleDetail, OpenSubtitleSearchResult, PlaybackHistoryItem, ResolvedMediaFile, TmdbSearchResult, TmdbTrailer } from '$lib/types';
import { nativeMobile, localMobileInvoke, getTorrentTarget, type DownloadTarget } from './mobile-connection';

export type DesktopPlayer = 'mpv' | 'vlc';

export type TorrentTransfer = {
	infoHash: string;
	name: string;
	status: 'Metadata' | 'Downloading' | 'Seeding' | 'Paused' | 'Queued' | 'Checking';
	error: string;
	progress: number;
	sizeBytes: number;
	downloadedBytes: number;
	uploadedBytes: number;
	downloadRate: number;
	uploadRate: number;
	peers: number;
	seeds: number;
	queuePosition: number;
	etaSeconds: number | null;
};

export type TorrentSnapshot = { engine: string; downloadDirectory: string; transfers: TorrentTransfer[] };

async function torrentInvoke<T>(command: string, args?: Record<string, unknown>, target:DownloadTarget = getTorrentTarget()): Promise<T> {
	if (!isDesktopRuntime()) throw new Error('Torrent transfers require the desktop app.');
	if (nativeMobile && target === 'phone' && import.meta.env.VITE_LUMA_MOBILE_DEMO !== 'true') return localMobileInvoke<T>(command, args);
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<T>(command, args);
}

export const readTorrentSnapshot = () => torrentInvoke<TorrentSnapshot>('torrent_snapshot');
export const addMagnet = (uri: string, destination?: string, target?:DownloadTarget) => torrentInvoke<string>('torrent_add_magnet', { uri, destination }, target);
export const addTorrentFile = (path: string, destination?: string) => torrentInvoke<string>('torrent_add_file', { path, destination });
export const addTorrentData = (encoded: string, destination?: string, target?:DownloadTarget) => torrentInvoke<string>('torrent_add_data', { encoded, destination }, target);
export const setTorrentPaused = (infoHash: string, paused: boolean) => torrentInvoke<void>('torrent_set_paused', { infoHash, paused });
export const moveTorrentQueue = (infoHash: string, direction: number) => torrentInvoke<void>('torrent_move_queue', { infoHash, direction });
export const setTorrentLimits = (download: number, upload: number) => torrentInvoke<void>('torrent_set_limits', { download, upload });
export const removeTorrent = (infoHash: string) => torrentInvoke<void>('torrent_remove', { infoHash });

export async function chooseTorrentFile(): Promise<string | null> {
	if (!isDesktopRuntime()) return null;
	const { open } = await import('@tauri-apps/plugin-dialog');
	const selection = await open({ multiple: false, filters: [{ name: 'Torrent', extensions: ['torrent'] }] });
	return typeof selection === 'string' ? selection : null;
}

const desktopPlayerPreferenceKey = 'media-library.desktop-player';

export function readPreferredDesktopPlayer(): DesktopPlayer {
	try {
		return localStorage.getItem(desktopPlayerPreferenceKey) === 'vlc' ? 'vlc' : 'mpv';
	} catch {
		return 'mpv';
	}
}

export function savePreferredDesktopPlayer(player: DesktopPlayer): void {
	try { localStorage.setItem(desktopPlayerPreferenceKey, player); }
	catch { /* Keep the current session usable when browser storage is unavailable. */ }
}

export type DesktopBootstrap = {
	version: string;
	platform: string;
	mediaCoreStatus: 'library-index-ready';
	nativeWindowFrame: boolean;
	windowShape?: 'css-squircle' | 'system';
};

export type LibraryStatus = {
	folders: string[];
	rootCount: number;
	fileCount: number;
	matchedCount: number;
	unmatchedCount: number;
	pendingCount: number;
	lastScanAt: number | null;
	isScanning: boolean;
};

export type ScanSummary = {
	rootName: string;
	fileCount: number;
	matchedCount: number;
	unmatchedCount: number;
	pendingCount: number;
	metadataError: string | null;
};

export type LibraryScanSummary = {
	rootCount: number;
	fileCount: number;
	matchedCount: number;
	unmatchedCount: number;
	pendingCount: number;
	metadataError: string | null;
};

export type CatalogPage = {
	items: CatalogMedia[];
	total: number;
	offset: number;
};

type CatalogPageCacheEntry = {
	createdAt?: number;
	page?: CatalogPage;
	pending?: Promise<CatalogPage | null>;
};

const catalogPageCache = new Map<string, CatalogPageCacheEntry>();
const catalogPageCacheLimit = 8;
const titleDetailCache = new Map<number, Promise<LocalTitleDetail | null>>();

/** Return an already loaded view without a loading frame when navigating back to it. */
export function peekCatalogPage(
	offset = 0,
	count = 48,
	kind?: 'movie' | 'series',
	query?: string,
	sort?: string
): CatalogPage | undefined {
	const entry = catalogPageCache.get(JSON.stringify([offset, count, kind, query, sort]));
	return nativeMobile && entry?.createdAt && Date.now() - entry.createdAt > 30_000 ? undefined : entry?.page;
}

export function invalidateCatalogPageCache() {
	catalogPageCache.clear();
	titleDetailCache.clear();
	trailerCache.clear();
	logoCache.clear();
}

export function isDesktopRuntime(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function readDesktopBootstrap(): Promise<DesktopBootstrap | null> {
	if (!isDesktopRuntime()) return null;

	const { invoke } = await import('$lib/platform/invoke');
	return invoke<DesktopBootstrap>('desktop_bootstrap');
}

export async function readLibraryStatus(): Promise<LibraryStatus | null> {
	if (!isDesktopRuntime()) return null;

	const { invoke } = await import('$lib/platform/invoke');
	return invoke<LibraryStatus>('get_library_status');
}

export async function readCatalogPage(
	offset = 0,
	count = 48,
	kind?: 'movie' | 'series',
	query?: string,
	sort?: string
): Promise<CatalogPage | null> {
	if (!isDesktopRuntime()) return null;

	const cacheKey = JSON.stringify([offset, count, kind, query, sort]);
	const cached = catalogPageCache.get(cacheKey);
	if (cached && !(nativeMobile && cached.createdAt && Date.now() - cached.createdAt > 30_000)) {
		catalogPageCache.delete(cacheKey);
		catalogPageCache.set(cacheKey, cached);
		if (cached.page) return cached.page;
		if (cached.pending) return cached.pending;
	}

	const entry: CatalogPageCacheEntry = {};
	const pending = (async () => {
		try {
			const { invoke } = await import('$lib/platform/invoke');
			const page = await invoke<CatalogPage>('get_catalog_page', { offset, count, kind, query, sort });
			if (catalogPageCache.get(cacheKey) === entry) {
				catalogPageCache.set(cacheKey, { page, createdAt: Date.now() });
				while (catalogPageCache.size > catalogPageCacheLimit) {
					const oldestKey = catalogPageCache.keys().next().value;
					if (oldestKey === undefined) break;
					catalogPageCache.delete(oldestKey);
				}
			}
			return page;
		} catch (error) {
			if (catalogPageCache.get(cacheKey) === entry) catalogPageCache.delete(cacheKey);
			throw error;
		}
	})();
	entry.pending = pending;
	catalogPageCache.set(cacheKey, entry);
	return pending;
}

export async function readLocalTitleDetail(mediaId: number, force = false): Promise<LocalTitleDetail | null> {
	if (!isDesktopRuntime()) return null;
	if (force) titleDetailCache.delete(mediaId);
	let pending = titleDetailCache.get(mediaId);
	if (!pending) {
		pending = import('$lib/platform/invoke').then(({ invoke }) =>
			invoke<LocalTitleDetail | null>('get_local_title_detail', { mediaId })
		).catch((error) => {
			titleDetailCache.delete(mediaId);
			throw error;
		});
		titleDetailCache.set(mediaId, pending);
		while (titleDetailCache.size > 12) {
			const oldest = titleDetailCache.keys().next().value;
			if (oldest === undefined) break;
			titleDetailCache.delete(oldest);
		}
	}
	return pending;
}

const trailerCache = new Map<number, Promise<TmdbTrailer | null>>();
const logoCache = new Map<number, Promise<string | null>>();
const logoCacheLimit = 32;

export async function readTitleLogo(mediaId: number): Promise<string | null> {
	if (!isDesktopRuntime() || !Number.isSafeInteger(mediaId) || mediaId <= 0) return null;
	let pending = logoCache.get(mediaId);
	if (pending) {
		logoCache.delete(mediaId);
		logoCache.set(mediaId, pending);
	}
	if (!pending) {
		pending = import('$lib/platform/invoke').then(({ invoke }) =>
			invoke<string | null>('get_title_logo', { mediaId })
		).catch((error) => {
			logoCache.delete(mediaId);
			throw error;
		});
		logoCache.set(mediaId, pending);
		while (logoCache.size > logoCacheLimit) {
			const oldest = logoCache.keys().next().value;
			if (oldest === undefined) break;
			logoCache.delete(oldest);
		}
	}
	return pending;
}

export async function readTitleTrailer(mediaId: number): Promise<TmdbTrailer | null> {
	if (!isDesktopRuntime() || !Number.isSafeInteger(mediaId) || mediaId <= 0) return null;
	let pending = trailerCache.get(mediaId);
	if (!pending) {
		pending = import('$lib/platform/invoke').then(({ invoke }) =>
			invoke<TmdbTrailer | null>('get_title_trailer', { mediaId })
		).then((trailer) => {
			if (trailer) trailerCache.set(mediaId, Promise.resolve(trailer));
			else trailerCache.delete(mediaId);
			return trailer;
		}).catch((error) => {
			trailerCache.delete(mediaId);
			throw error;
		});
		trailerCache.set(mediaId, pending);
		while (trailerCache.size > 32) {
			const oldest = trailerCache.keys().next().value;
			if (oldest === undefined) break;
			trailerCache.delete(oldest);
		}
	}
	return pending;
}

export async function resolveMediaFile(mediaId: number, expectedTitle?: string, expectedEpisode?: string, expectedTmdbId?: number): Promise<ResolvedMediaFile> {
	if (!isDesktopRuntime()) throw new Error('Local playback is available in the desktop app.');
	const { invoke } = await import('$lib/platform/invoke');
	if (nativeMobile && mediaId < 1_000_000_000 && expectedTitle) {
		const current = await invoke<LocalTitleDetail | null>('get_local_title_detail', { mediaId });
		const normalize = (title: string) => title.trim().toLocaleLowerCase().replace(/\s+/g, ' ');
		const marker = expectedEpisode?.match(/S(\d+)E(\d+)/i);
		const sameTitle = (detail: LocalTitleDetail | null) => !!detail && normalize(detail.media.title) === normalize(expectedTitle) && (!expectedTmdbId || !detail.tmdbId || detail.tmdbId === expectedTmdbId);
		const sameEpisode = (detail: LocalTitleDetail | null) => !marker || detail?.files.some(file => file.mediaId === mediaId && file.season === Number(marker[1]) && file.episode === Number(marker[2]));
		if (!sameTitle(current) || !sameEpisode(current)) {
			invalidateCatalogPageCache();
			const page = await invoke<CatalogPage>('get_catalog_page', {offset:0,count:48,query:expectedTitle});
			const matches = new Set<number>();
			for (const item of page.items.filter(item => item.id < 1_000_000_000 && normalize(item.title) === normalize(expectedTitle))) {
				const detail = await invoke<LocalTitleDetail | null>('get_local_title_detail', {mediaId:item.id});
				if (!sameTitle(detail) || !detail) continue;
				if (marker) {
					for (const file of detail.files) if (file.season === Number(marker[1]) && file.episode === Number(marker[2])) matches.add(file.mediaId);
				} else if (detail.media.kind === 'movie' || detail.files.length === 1) matches.add(detail.media.id);
			}
			if (matches.size !== 1) throw new Error('Couldn’t identify this video in your computer’s current library.');
			mediaId = [...matches][0];
		}
	}
	try {
		return {...await invoke<ResolvedMediaFile>('resolve_media_file', { mediaId }),mediaId};
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		if (/Could not open the indexed media file|This media file is no longer in the library/i.test(message)) {
			throw new Error(`The file for ${expectedTitle || 'this title'} is missing from its library folder on your computer. Restore the file or refresh the library. Your saved viewing position has been kept.`);
		}
		throw error;
	}
}

export async function savePlaybackProgress(mediaId: number, positionSeconds: number, durationSeconds: number): Promise<void> {
	if (!isDesktopRuntime()) return;
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<void>('save_playback_progress', { mediaId, positionSeconds, durationSeconds });
}

export async function recordPlaybackActivity(mediaId: number, markPreviousEpisodesWatched = false): Promise<void> {
	if (!isDesktopRuntime()) return;
	const { invoke } = await import('$lib/platform/invoke');
	await invoke<void>('record_playback_activity', { mediaId, markPreviousEpisodesWatched });
	titleDetailCache.clear();
	window.dispatchEvent(new CustomEvent('media-library:playback-history-updated'));
}

export async function readPlaybackHistory(count = 12): Promise<PlaybackHistoryItem[]> {
	if (!isDesktopRuntime()) return [];
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<PlaybackHistoryItem[]>('get_playback_history', { count });
}

export async function readContinueWatching(count = 12): Promise<ContinueWatchingItem[]> {
	if (!isDesktopRuntime()) return [];
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<ContinueWatchingItem[]>('get_continue_watching', { count });
}

export async function localMediaUrl(path: string): Promise<string> {
	if (import.meta.env.VITE_LUMA_MOBILE_DEMO === 'true' && path.startsWith('/mobile-demo/')) return path;
	if (/^https?:\/\//i.test(path)) return path;
	if (!isDesktopRuntime()) throw new Error('Local media files require the desktop app.');
	const { convertFileSrc } = await import('@tauri-apps/api/core');
	return convertFileSrc(path);
}

export async function setOpenSubtitlesApiKey(apiKey: string): Promise<void> {
	if (!isDesktopRuntime()) throw new Error('OpenSubtitles integration is available in the desktop app.');
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<void>('set_opensubtitles_api_key', { apiKey });
}

export async function loginOpenSubtitles(username: string, password: string): Promise<void> {
	if (!isDesktopRuntime()) throw new Error('OpenSubtitles integration is available in the desktop app.');
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<void>('login_opensubtitles', { username, password });
}

export async function searchOpenSubtitles(
	query: string,
	language: string,
	year: number | undefined,
	kind: 'movie' | 'series'
): Promise<OpenSubtitleSearchResult[]> {
	if (!isDesktopRuntime()) throw new Error('OpenSubtitles integration is available in the desktop app.');
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<OpenSubtitleSearchResult[]>('search_opensubtitles', { query, language, year, kind });
}

export async function downloadOpenSubtitle(mediaId: number, fileId: number): Promise<string> {
	if (!isDesktopRuntime()) throw new Error('OpenSubtitles integration is available in the desktop app.');
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<string>('download_opensubtitle', { mediaId, fileId });
}

export async function openMediaInDesktopPlayer(mediaId: number, player: DesktopPlayer): Promise<void> {
	if (!isDesktopRuntime()) throw new Error('External playback is available in the desktop app.');
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<void>('open_media_in_desktop_player', { mediaId, player });
}

export type NativePlaybackSnapshot = {
	engine: DesktopPlayer;
	playing: boolean;
	ended: boolean;
	positionSeconds: number;
	durationSeconds: number;
	volume: number;
	muted: boolean;
	rate: number;
	audioTracks: { id: number; label: string; language: string; selected: boolean }[];
	subtitleTracks: { id: number; label: string; language: string; selected: boolean }[];
};

export async function startNativePlayer(mediaId: number, engine: DesktopPlayer, preferences: import('./playback-preferences').PlaybackPreferences): Promise<NativePlaybackSnapshot> {
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<NativePlaybackSnapshot>('start_native_player', { mediaId, engine, preferences });
}

export async function nativePlayerStatus(): Promise<NativePlaybackSnapshot> {
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<NativePlaybackSnapshot>('native_player_status');
}

export async function nativePlayerAction(action: 'play' | 'pause' | 'seek' | 'volume' | 'mute' | 'rate' | 'audio-track' | 'subtitle-track' | 'subtitle-size' | 'subtitle-position' | 'subtitle-font', value?: number): Promise<NativePlaybackSnapshot> {
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<NativePlaybackSnapshot>('native_player_action', { action, value });
}

export async function nativePlayerLoadSubtitle(path: string): Promise<NativePlaybackSnapshot> {
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<NativePlaybackSnapshot>('native_player_load_subtitle', { path });
}

export async function chooseSubtitleFile(): Promise<string | null> {
	if (!isDesktopRuntime()) return null;
	const { open } = await import('@tauri-apps/plugin-dialog');
	const selected = await open({ multiple: false, title: 'Choose a subtitle file', filters: [{ name: 'Subtitles', extensions: ['srt', 'vtt', 'ass', 'ssa', 'sub'] }] });
	return typeof selected === 'string' ? selected : null;
}

export async function resizeNativePlayer(): Promise<void> {
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<void>('resize_native_player');
}

export async function stopNativePlayer(): Promise<void> {
	const { invoke } = await import('$lib/platform/invoke');
	return invoke<void>('stop_native_player');
}

export async function chooseMediaFolder(): Promise<string | null> {
	if (!isDesktopRuntime()) return null;

	const { open } = await import('@tauri-apps/plugin-dialog');
	const selected = await open({ directory: true, multiple: false, title: 'Choose a media folder' });
	return typeof selected === 'string' ? selected : null;
}

export async function scanLibrary(rootPath: string): Promise<ScanSummary> {
	if (!isDesktopRuntime()) throw new Error('Local library scanning is available in the desktop app.');

	const { invoke } = await import('$lib/platform/invoke');
	const result = await invoke<ScanSummary>('scan_library', { rootPath });
	invalidateCatalogPageCache();
	return result;
}

export async function rescanLibrary(): Promise<LibraryScanSummary> {
	if (!isDesktopRuntime()) throw new Error('Local library scanning is available in the desktop app.');

	const { invoke } = await import('$lib/platform/invoke');
	const result = await invoke<LibraryScanSummary>('rescan_library');
	invalidateCatalogPageCache();
	return result;
}

export async function testTmdbConnection(): Promise<void> {
	if (!isDesktopRuntime()) throw new Error('TMDb is available in the desktop app.');

	const { invoke } = await import('$lib/platform/invoke');
	return invoke<void>('test_tmdb_connection');
}

export async function searchTmdb(query: string): Promise<TmdbSearchResult[]> {
	if (!isDesktopRuntime()) throw new Error('TMDb search is available in the desktop app.');

	const { invoke } = await import('$lib/platform/invoke');
	return invoke<TmdbSearchResult[]>('search_tmdb', { query });
}
