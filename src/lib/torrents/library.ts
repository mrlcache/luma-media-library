import { writable } from 'svelte/store';
import { invalidateCatalogPageCache, isDesktopRuntime, readTorrentSnapshot, searchTmdb, type TorrentTransfer } from '$lib/platform/desktop';
import type { CatalogMedia, TmdbSearchResult } from '$lib/types';

export type LibraryTransfer = TorrentTransfer & { metadata?: TmdbSearchResult };
export type LibraryCard = Omit<CatalogMedia, 'id'> & { id: number | string; transfer?: LibraryTransfer };
export const libraryTransfers = writable<LibraryTransfer[]>([]);
const metadataCache = new Map<string, TmdbSearchResult>();
const pendingMetadata = new Set<string>();
const addedAt = new Map<string, number>();

export function transferKind(item: LibraryTransfer): 'movie' | 'series' {
	return item.metadata?.kind ?? (/\bS\d{1,2}(?:E\d{1,3})?\b|complete (?:series|season)|Planet Earth|Cowboy Bebop/i.test(item.name) ? 'series' : 'movie');
}

export function transferTitle(item: LibraryTransfer) {
	return item.metadata?.title ?? item.name.replace(/[._]/g, ' ').split(/\s+(?:\(?\d{4}\)?|S\d{1,2}(?:E\d{1,3})?\b|\d{3,4}p\b|UHD\b|BluRay\b|HDR\b|IMAX\b|Complete\b|Remastered\b)/i)[0].trim();
}

export function transferCard(item: LibraryTransfer): LibraryCard {
	if (!addedAt.has(item.infoHash)) addedAt.set(item.infoHash, Date.now() / 1000);
	return { id: `download-${item.infoHash}`, title: transferTitle(item), kind: transferKind(item), year: item.metadata?.year ?? null,
		extension: 'mkv', sizeBytes: item.sizeBytes, modifiedAt: addedAt.get(item.infoHash)!, overview: item.metadata?.overview ?? null,
		voteAverage: item.metadata?.voteAverage ?? null, posterUrl: item.metadata?.posterUrl ?? null, backdropUrl: item.metadata?.backdropUrl ?? null, transfer: item };
}

export function startTorrentLibraryUpdates() {
	let stopped = false;
	let busy = false;
	let timer: number | undefined;
	let release: (() => void) | undefined;
	let latest: TorrentTransfer[] = [];
	const completed=new Set<string>();
	const publish = () => { if (!stopped) libraryTransfers.set(latest.map((item) => ({ ...item, metadata: metadataCache.get(item.infoHash) }))); };
	async function resolveMetadata(item: TorrentTransfer) {
		if (!isDesktopRuntime() || metadataCache.has(item.infoHash) || pendingMetadata.has(item.infoHash)) return;
		pendingMetadata.add(item.infoHash);
		try {
			const query = transferTitle(item);
			const kind = transferKind(item);
			const year = /\b(19\d{2}|20\d{2})\b/.exec(item.name)?.[1];
			const normalize = (title: string) => title.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().replace(/[^\p{L}\p{N}]/gu, '');
			const results = (await searchTmdb(query)).filter((result) => result.kind === kind);
			const match = results.find((result) => normalize(result.title) === normalize(query) && (!year || result.year === Number(year)))
				?? results.find((result) => normalize(result.title) === normalize(query))
				?? (year ? results.find((result) => result.year === Number(year)) : undefined);
			if (match) { metadataCache.set(item.infoHash, match); publish(); }
		} catch { /* Retry on the next library update if the API is temporarily unavailable. */ }
		finally { pendingMetadata.delete(item.infoHash); }
	}
	async function refresh() {
		if (busy || stopped) return;
		busy = true;
		try {
			if (!isDesktopRuntime()) return;
			const snapshot = (await readTorrentSnapshot()).transfers;
			if(snapshot.some(item=>item.progress>=1 && !completed.has(item.infoHash))){
				for(const item of snapshot)if(item.progress>=1)completed.add(item.infoHash);
				invalidateCatalogPageCache();window.dispatchEvent(new Event('luma-library-changed'));
			}
			const transfers = snapshot.filter((item) => item.progress < 1);
			latest = transfers;
			publish();
			// Limit concurrent metadata requests while the transfer list keeps updating.
			for (const item of transfers) { if (pendingMetadata.size >= 3) break; void resolveMetadata(item); }
		} catch { /* A missing torrent engine must not hide the existing library. */ }
		finally { busy = false; }
	}
	function stopPolling() {
		if (timer === undefined) return;
		window.clearInterval(timer);
		timer = undefined;
	}
	function startPolling() {
		if (stopped || document.hidden || timer !== undefined) return;
		timer = window.setInterval(() => void refresh(), 1500);
	}
	function handleVisibilityChange() {
		if (document.hidden) {
			stopPolling();
			return;
		}
		void refresh();
		startPolling();
	}
	void refresh();
	startPolling();
	document.addEventListener('visibilitychange', handleVisibilityChange);
	if (isDesktopRuntime()) {
		void import('@tauri-apps/api/event').then(({ listen }) => listen('library-changed', () => {
			invalidateCatalogPageCache();
			window.dispatchEvent(new Event('luma-library-changed'));
		})).then((unlisten) => { if (stopped) unlisten(); else release = unlisten; });
	}
	return () => {
		stopped = true;
		stopPolling();
		document.removeEventListener('visibilitychange', handleVisibilityChange);
		release?.();
	};
}
