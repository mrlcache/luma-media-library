import type { CatalogMedia, ContinueWatchingItem, PlaybackHistoryItem } from '$lib/types';

// Preserve the current view across route mounts; refresh quietly in the background.
export const homeSnapshot: { ready: boolean; heroIndex: number; catalog: CatalogMedia[]; resume: ContinueWatchingItem[]; history: PlaybackHistoryItem[] } = {
	ready: false, heroIndex: 0, catalog: [], resume: [], history: []
};
