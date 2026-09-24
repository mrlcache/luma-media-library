import type { CatalogMedia, TmdbSearchResult } from '$lib/types';

export type DesktopBootstrap = {
	version: string;
	platform: string;
	mediaCoreStatus: 'library-index-ready';
	nativeWindowFrame: boolean;
	windowShape?: 'css-squircle' | 'system';
};

export type LibraryStatus = {
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
	page?: CatalogPage;
	pending?: Promise<CatalogPage | null>;
};

const catalogPageCache = new Map<string, CatalogPageCacheEntry>();
const catalogPageCacheLimit = 8;

export function invalidateCatalogPageCache() {
	catalogPageCache.clear();
}

export function isDesktopRuntime(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function readDesktopBootstrap(): Promise<DesktopBootstrap | null> {
	if (!isDesktopRuntime()) return null;

	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<DesktopBootstrap>('desktop_bootstrap');
}

export async function readLibraryStatus(): Promise<LibraryStatus | null> {
	if (!isDesktopRuntime()) return null;

	const { invoke } = await import('@tauri-apps/api/core');
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
	if (cached) {
		catalogPageCache.delete(cacheKey);
		catalogPageCache.set(cacheKey, cached);
		if (cached.page) return cached.page;
		if (cached.pending) return cached.pending;
	}

	const entry: CatalogPageCacheEntry = {};
	const pending = (async () => {
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			const page = await invoke<CatalogPage>('get_catalog_page', { offset, count, kind, query, sort });
			if (catalogPageCache.get(cacheKey) === entry) {
				catalogPageCache.set(cacheKey, { page });
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

export async function chooseMediaFolder(): Promise<string | null> {
	if (!isDesktopRuntime()) return null;

	const { open } = await import('@tauri-apps/plugin-dialog');
	const selected = await open({ directory: true, multiple: false, title: 'Choose a media folder' });
	return typeof selected === 'string' ? selected : null;
}

export async function scanLibrary(rootPath: string): Promise<ScanSummary> {
	if (!isDesktopRuntime()) throw new Error('Local library scanning is available in the desktop app.');

	const { invoke } = await import('@tauri-apps/api/core');
	const result = await invoke<ScanSummary>('scan_library', { rootPath });
	invalidateCatalogPageCache();
	return result;
}

export async function rescanLibrary(): Promise<LibraryScanSummary> {
	if (!isDesktopRuntime()) throw new Error('Local library scanning is available in the desktop app.');

	const { invoke } = await import('@tauri-apps/api/core');
	const result = await invoke<LibraryScanSummary>('rescan_library');
	invalidateCatalogPageCache();
	return result;
}

export async function testTmdbConnection(): Promise<void> {
	if (!isDesktopRuntime()) throw new Error('TMDb is available in the desktop app.');

	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<void>('test_tmdb_connection');
}

export async function searchTmdb(query: string): Promise<TmdbSearchResult[]> {
	if (!isDesktopRuntime()) throw new Error('TMDb search is available in the desktop app.');

	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<TmdbSearchResult[]>('search_tmdb', { query });
}
