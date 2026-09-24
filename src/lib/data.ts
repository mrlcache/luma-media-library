import type { MediaItem } from '$lib/types';

/** The app has no bundled demo catalog; library items come from the user's media folders. */
export const media: MediaItem[] = [];

export function getMedia(slug: string) {
	return media.find((item) => item.id === slug);
}
