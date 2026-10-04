import { writable, get } from 'svelte/store';
import type { MediaItem } from '../types';
import type { LibraryCard } from '../torrents/library';

export type Favorite = { media: MediaItem; savedAt: number };
const storageKey = 'luma.favorites.v1';
export const favorites = writable<Favorite[]>([], (set) => {
	if (typeof localStorage === 'undefined') return;
	const read = () => {
		try {
			const records: unknown = JSON.parse(localStorage.getItem(storageKey) ?? '[]');
			if (Array.isArray(records)) set(records.filter((entry): entry is Favorite =>
				!!entry?.media && typeof entry.media.id === 'string' && typeof entry.media.title === 'string'
				&& ['movie', 'series'].includes(entry.media.kind) && Number.isFinite(entry.savedAt)));
		} catch { /* Keep the current collection if storage cannot be read. */ }
	};
	read();
	const sync = (event: StorageEvent) => { if (event.key === storageKey) read(); };
	window.addEventListener('storage', sync);
	return () => window.removeEventListener('storage', sync);
});

const titleKey = (media: MediaItem) => `${media.kind}:${media.year}:${media.title.normalize('NFKC').trim().toLocaleLowerCase()}`;
export function sameFavorite(a: MediaItem, b: MediaItem): boolean {
	if (a.tmdbId && b.tmdbId) return a.kind === b.kind && a.tmdbId === b.tmdbId;
	return a.id === b.id || titleKey(a) === titleKey(b);
}
export function isFavorite(entries: Favorite[], media: MediaItem): boolean {
	return entries.some((entry) => sameFavorite(entry.media, media));
}
export function toggleFavorite(media: MediaItem): void {
	const current = get(favorites);
	const next = isFavorite(current, media) ? current.filter((entry) => !sameFavorite(entry.media, media))
		: [...current, { media: { ...media, nextEpisode: undefined, episodes: undefined }, savedAt: Date.now() }];
	// Persist before publishing so the heart never claims an unsaved change.
	localStorage.setItem(storageKey, JSON.stringify(next));
	favorites.set(next);
}
export function cardMedia(item: LibraryCard): MediaItem {
	const match = typeof item.id === 'string' ? /^tmdb-(movie|series)-(\d+)$/.exec(item.id) : null;
	return { id: String(item.id), tmdbId: match ? Number(match[2]) : item.transfer?.metadata?.id,
		installed: typeof item.id === 'number', title: item.title, kind: item.kind === 'series' ? 'series' : 'movie',
		year: item.year ?? 0, genres: [], rating: item.voteAverage?.toFixed(1) ?? '', runtime: '',
		poster: item.posterUrl ?? '', backdrop: item.backdropUrl ?? '', synopsis: item.overview ?? '', match: '' };
}
export function favoriteCard(entry: Favorite): LibraryCard {
	const media = entry.media;
	return { id: media.tmdbId ? `tmdb-${media.kind}-${media.tmdbId}` : media.id, title: media.title,
		kind: media.kind, year: media.year || null, overview: media.synopsis, voteAverage: Number(media.rating) || null,
		posterUrl: media.poster, backdropUrl: media.backdrop, extension: '', sizeBytes: 0, modifiedAt: entry.savedAt / 1000 };
}
export function savedFavorite(slug: string): MediaItem | undefined {
	const media = get(favorites).find((entry) => entry.media.id === slug || (entry.media.tmdbId && `tmdb-${entry.media.kind}-${entry.media.tmdbId}` === slug))?.media;
	return media ? { ...media, id: slug, installed: false } : undefined;
}
