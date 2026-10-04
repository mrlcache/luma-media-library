import { readCatalogPage, searchTmdb } from '$lib/platform/desktop';
import { discoveryMedia } from './discovery';
import { cardMedia, sameFavorite } from './favorites';
import type { MediaItem } from '$lib/types';

export type SearchResults = { items: MediaItem[]; warning: string };
const cache = new Map<string, { time: number; result: SearchResults }>();
export async function searchTitles(query: string): Promise<SearchResults> {
	const key = query.trim().toLocaleLowerCase();
	if (key.length < 2) return { items: [], warning: '' };
	const cached = cache.get(key);
	if (cached && Date.now() - cached.time < 5 * 60_000) return cached.result;
	const [remote, local] = await Promise.allSettled([searchTmdb(query.trim()), readCatalogPage(0, 48, undefined, query.trim())]);
	const downloaded = local.status === 'fulfilled' ? (local.value?.items ?? []).map(cardMedia) : [];
	const items = remote.status === 'fulfilled' ? remote.value.map(discoveryMedia).map((item) => {
		const installed = downloaded.find((candidate) => sameFavorite(candidate, item));
		return installed ? { ...item, id: installed.id, installed: true } : item;
	}) : [];
	for (const item of downloaded) if (!items.some((candidate) => sameFavorite(candidate, item))) items.push({ ...item, genres: [item.kind === 'series' ? 'Series' : 'Movie'] });
	const unique = items.filter((item, index) => items.findIndex((candidate) => sameFavorite(candidate, item)) === index);
	const result = { items: unique, warning: remote.status === 'rejected' ? 'The online catalog could not be searched. Try again.' : '' };
	if (!result.warning) {
		cache.delete(key); cache.set(key, { time: Date.now(), result });
		while (cache.size > 24) cache.delete(cache.keys().next().value!);
	}
	return result;
}
