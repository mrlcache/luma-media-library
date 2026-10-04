import { isDesktopRuntime } from '$lib/platform/desktop';
import { tmdbImageSize } from './artwork';
import type { MediaItem, TmdbSearchResult, TmdbTrailer } from '$lib/types';

export type DiscoveryFeed = { featured: TmdbSearchResult[]; sections: { title: string; items: TmdbSearchResult[] }[]; warning: string | null };
const storageKey = 'luma.discovery.v1';
const ttl = 30 * 60 * 1000;
let snapshot: { time: number; feed: DiscoveryFeed } | null = null;
let pending: Promise<DiscoveryFeed> | null = null;
const titles = new Map<string, TmdbSearchResult>();
const assets = new Map<string, Promise<unknown>>();
export function resetDiscovery() {
	snapshot = null; pending = null; titles.clear(); assets.clear(); preparedLogos.clear(); decodedImages.clear();
	try { localStorage.removeItem(storageKey); } catch { /* Session reset still works. */ }
}

function trimCache<T>(cache: Map<string, T>, limit: number) {
	while (cache.size > limit) {
		const oldest = cache.keys().next().value;
		if (oldest === undefined) break;
		cache.delete(oldest);
	}
}

function remember(feed: DiscoveryFeed) {
	for (const item of [...feed.featured, ...feed.sections.flatMap((section) => section.items)]) titles.set(`tmdb-${item.kind}-${item.id}`, item);
	trimCache(titles, 128);
}
export function cachedDiscovery(): DiscoveryFeed | null {
	if (!snapshot && typeof localStorage !== 'undefined') {
		try {
			const stored = JSON.parse(localStorage.getItem(storageKey) ?? 'null');
			if (stored && Number.isFinite(stored.time) && Array.isArray(stored.feed?.featured) && Array.isArray(stored.feed?.sections)) snapshot = stored;
		} catch { /* An invalid cache is replaced by a fresh feed. */ }
	}
	if (snapshot) remember(snapshot.feed);
	return snapshot?.feed ?? null;
}
export async function readDiscovery(force = false): Promise<DiscoveryFeed> {
	cachedDiscovery();
	if (!force && snapshot && Date.now() - snapshot.time < ttl) return snapshot.feed;
	if (pending) return pending;
	if (!isDesktopRuntime()) throw new Error('Open Luma desktop to load recommendations.');
	pending = import('$lib/platform/invoke').then(async ({ invoke }) => {
		for (let attempt = 0; ; attempt++) {
			try { return await invoke<DiscoveryFeed>('get_discovery_feed'); }
			catch (error) {
				// The webview can mount while a DEV rebuild is replacing its backend.
				if (attempt >= 2 || typeof error !== 'string' || !/not found|not managed|not initialized/i.test(error)) throw error;
				await new Promise((resolve) => setTimeout(resolve, 1000));
			}
		}
	}).then((feed) => {
		snapshot = { time: Date.now(), feed }; remember(feed);
		try { localStorage.setItem(storageKey, JSON.stringify(snapshot)); } catch { /* Session cache remains available. */ }
		return feed;
	}).finally(() => { pending = null; });
	return pending;
}
export function discoveryMedia(item: TmdbSearchResult): MediaItem {
	return { id: `tmdb-${item.kind}-${item.id}`, tmdbId: item.id, installed: false, title: item.title, kind: item.kind, year: item.year ?? 0,
		genres: [item.kind === 'series' ? 'Series' : 'Movie'], rating: item.voteAverage?.toFixed(1) ?? '', runtime: '',
		poster: tmdbImageSize(item.posterUrl, 'w780'), backdrop: tmdbImageSize(item.backdropUrl ?? item.posterUrl, 'w1280'), synopsis: item.overview, match: '' };
}
export async function readDiscoveryTitle(slug: string): Promise<MediaItem | null> {
	const match = /^tmdb-(movie|series)-(\d+)$/.exec(slug);
	if (!match || !Number.isSafeInteger(Number(match[2])) || Number(match[2]) <= 0) return null;
	cachedDiscovery();
	const cached = titles.get(slug);
	if (cached) return discoveryMedia(cached);
	if (!isDesktopRuntime()) return null;
	const { invoke } = await import('$lib/platform/invoke');
	const item = await invoke<TmdbSearchResult>('get_discovery_title', { id: Number(match[2]), kind: match[1] });
	titles.set(slug,item);
	trimCache(titles, 128);
	return discoveryMedia(item);
}
async function asset<T>(media: MediaItem, type: 'logo' | 'trailer'): Promise<T | null> {
	if (!media.tmdbId || !isDesktopRuntime()) return null;
	const key = `${media.id}:${type}`;
	let request = assets.get(key);
	if (!request) {
		request = import('$lib/platform/invoke').then(({ invoke }) => invoke<T | null>(`get_discovery_${type}`, {id: media.tmdbId, kind: media.kind})).catch((error) => { assets.delete(key); throw error; });
		assets.set(key, request);
		trimCache(assets, 16);
	} else {
		assets.delete(key); assets.set(key, request);
	}
	return await request as T | null;
}
const logoStorageKey = 'luma.logo-urls.v1';
type LogoEntry = {url:string|null;time:number};
function logoUrls(): Record<string,LogoEntry> {
	try {
		const value=JSON.parse(localStorage.getItem(logoStorageKey) ?? '{}');
		if(!value || typeof value!=='object' || Array.isArray(value))return {};
		return Object.fromEntries(Object.entries(value).filter(([,entry])=>{
			const saved=entry as LogoEntry|null;
			return saved && Number.isFinite(saved.time) && (saved.url===null || (typeof saved.url==='string' && saved.url.startsWith('https://image.tmdb.org/')));
		})) as Record<string,LogoEntry>;
	} catch { return {}; }
}
export async function readDiscoveryLogo(media: MediaItem): Promise<string|null> {
	if(!media.tmdbId || !isDesktopRuntime())return null;
	const entries=logoUrls(), saved=entries[media.id];
	if(saved && Number.isFinite(saved.time) && Date.now()-saved.time < 7*24*60*60*1000 && (saved.url===null || (typeof saved.url==='string' && saved.url.startsWith('https://image.tmdb.org/')))) return saved.url;
	const url=await asset<string>(media,'logo');
	entries[media.id]={url,time:Date.now()};
	// Persist only URL metadata, never decoded PNGs or base64 artwork.
	const recent=Object.entries(entries).sort((a,b)=>b[1].time-a[1].time).slice(0,32);
	try { localStorage.setItem(logoStorageKey,JSON.stringify(Object.fromEntries(recent))); } catch { /* Browsing still works without persistent storage. */ }
	return url;
}
export const readDiscoveryTrailer = (media: MediaItem) => asset<TmdbTrailer>(media, 'trailer');

const preparedLogos = new Map<string, Promise<string | null>>();
const decodedImages = new Map<string, HTMLImageElement>();
export async function decodeHeroArtwork(url:string): Promise<HTMLImageElement|null> {
	const image=new Image();
	let timeout:ReturnType<typeof setTimeout>|undefined;
	try {
		image.src=url;
		await Promise.race([image.decode(),new Promise<never>((_,reject)=>{timeout=setTimeout(()=>reject(new Error('Artwork timeout')),8000);})]);
		return image;
	} catch { image.src=''; return null; }
	finally { if(timeout)clearTimeout(timeout); }
}

export async function prepareDiscoveryLogo(media: MediaItem): Promise<string | null> {
	let pending = preparedLogos.get(media.id);
	if (!pending) {
		pending = readDiscoveryLogo(media).then(async (source) => {
			if (!source) return null;
			// The hero displays this at at most 480px; avoid decoding an oversized original PNG.
			const url = tmdbImageSize(source, 'w500');
			const image = await decodeHeroArtwork(url);
			if(!image) {
				preparedLogos.delete(media.id); assets.delete(`${media.id}:logo`);
				const entries=logoUrls();delete entries[media.id];
				try {localStorage.setItem(logoStorageKey,JSON.stringify(entries));} catch { /* Retry on the next request. */ }
				return null;
			}
			decodedImages.set(url, image);
			trimCache(decodedImages, 2);
			return url;
		}).catch(() => { preparedLogos.delete(media.id); return null; });
		preparedLogos.set(media.id, pending);
		trimCache(preparedLogos, 2);
	} else {
		preparedLogos.delete(media.id); preparedLogos.set(media.id, pending);
	}
	return pending;
}
