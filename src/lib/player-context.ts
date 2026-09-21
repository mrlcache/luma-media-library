import { getContext } from 'svelte';
import type { MediaItem } from '$lib/types';

export const PLAYER_CONTEXT = Symbol('player-context');

export type PlayerActions = {
	open: (media: MediaItem) => void;
	close: () => void;
};

export function usePlayer() {
	return getContext<PlayerActions>(PLAYER_CONTEXT);
}
