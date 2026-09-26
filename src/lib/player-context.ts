import { getContext } from 'svelte';
import type { MediaItem } from '$lib/types';

export const PLAYER_CONTEXT = Symbol('player-context');
export const PLAYBACK_HISTORY_UPDATED_EVENT = 'media-library:playback-history-updated';

export type PlayerActions = {
	open: (media: MediaItem) => void;
	close: () => void;
};

export function usePlayer() {
	return getContext<PlayerActions>(PLAYER_CONTEXT);
}
