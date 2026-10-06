import type { MediaItem } from '../types';

export type ReleaseScope = { type: 'movie' } | { type: 'season'; season: number } | { type: 'episode'; season: number; episode: number };
export type TorrentRelease = {
	name: string; source: string; uploader: string; group: string; sizeBytes: number; size: string;
	seeds: number; peers: number; infoHash: string; magnet: string; url: string; downloadKey: string | null;
	quality: string; codec: string; fileType: string; qxr: boolean;
};
export type ReleaseSort = 'seeds' | 'size' | 'name';

const words = (value: string) => value.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase().replace(/[^\p{L}\p{N}]+/gu, ' ').trim();
const numeric = (value: unknown) => typeof value === 'number' && Number.isFinite(value) ? Math.max(0, value) : 0;
const text = (value: unknown) => typeof value === 'string' ? value : '';

export function normalizeRelease(raw: Record<string, unknown>): TorrentRelease {
	const name = text(raw.name);
	const qxr = /(?:^|[^a-z0-9])qxr(?:$|[^a-z0-9])/i.test(`${name} ${text(raw.group)} ${text(raw.uploader)}`);
	const quality = /\b(2160p|1080p|720p|480p)\b/i.exec(name)?.[1]?.toLowerCase() ?? (/\b(?:4k|uhd)\b/i.test(name) ? '2160p' : text(raw.quality));
	const codecText = `${name} ${text(raw.codec)}`;
	const codec = /\b(?:x265|h[ .]?265|hevc)\b/i.test(codecText) ? 'HEVC' : /\b(?:x264|h[ .]?264|avc)\b/i.test(codecText) ? 'H.264' : /\bav1\b/i.test(codecText) ? 'AV1' : '';
	const fileType = /\.(mkv|mp4|avi|m4v|ts)(?:$|[\s\]])/i.exec(`${text(raw.filename)} ${name}`)?.[1]?.toUpperCase() ?? text(raw.fileType).toUpperCase();
	const infoHash = text(raw.infoHash);
	const magnet = text(raw.magnet) || (/^(?:[a-f0-9]{40}|[a-z2-7]{32}|[a-f0-9]{64})$/i.test(infoHash)
		? `magnet:?xt=urn:${infoHash.length === 64 && /^[a-f0-9]+$/i.test(infoHash) ? `btmh:1220${infoHash}` : `btih:${infoHash}`}&dn=${encodeURIComponent(name)}`
		: '');
	return { name, source: text(raw.source), uploader: text(raw.uploader), group: text(raw.group) || (qxr ? 'QXR' : ''),
		sizeBytes: numeric(raw.sizeBytes), size: text(raw.size), seeds: numeric(raw.seeds), peers: numeric(raw.peers),
		infoHash, magnet, url: text(raw.url), downloadKey: text(raw.downloadKey) || null, quality, codec, fileType, qxr };
}

export function releaseMatches(release: TorrentRelease, media: Pick<MediaItem, 'title' | 'year'>, scope: ReleaseScope): boolean {
	if (!(` ${words(release.name)} `).includes(` ${words(media.title)} `)) return false;
	const matches = [...release.name.matchAll(/\bS(\d{1,2})[ ._-]*E(\d{1,3})(?!\d)|\b(\d{1,2})x(\d{1,3})(?!\d)|\bSeason[ ._-]*(\d{1,2})[ ._-]+Episode[ ._-]*(\d{1,3})(?!\d)/gi)];
	if (scope.type === 'movie') {
		const year = /\b(19\d{2}|20\d{2})\b/.exec(release.name)?.[1];
		return !matches.length && (!year || !media.year || Number(year) === media.year);
	}
	if (scope.type === 'episode') {
		if (matches.length !== 1) return false;
		const match = matches[0];
		const tail = release.name.slice((match.index ?? 0) + match[0].length);
		if (/^(?:[ ._]*E\d{1,3}(?!\d)|[ ._]*[-,][ ._]*E?\d{1,3}(?!\d|p))/i.test(tail)) return false;
		return Number(match[1] ?? match[3] ?? match[5]) === scope.season && Number(match[2] ?? match[4] ?? match[6]) === scope.episode;
	}
	if (matches.length) return false;
	const range = /\bS(\d{1,2})[ ._-]+S(\d{1,2})\b|\bSeasons?[ ._-]+(\d{1,2})[ ._-]+(\d{1,2})\b/i.exec(release.name);
	if (range && scope.season >= Number(range[1] ?? range[3]) && scope.season <= Number(range[2] ?? range[4])) return true;
	return [...release.name.matchAll(/\bS(\d{1,2})(?!\d)|\bSeason[ ._-]*(\d{1,2})\b/gi)].some((match) => Number(match[1] ?? match[2]) === scope.season)
		|| /\bcomplete[ ._-]+(?:series|collection)\b/i.test(release.name);
}

export function sortReleases(releases: TorrentRelease[], sort: ReleaseSort): TorrentRelease[] {
	return [...releases].sort((a, b) => Number(b.qxr) - Number(a.qxr) ||
		(sort === 'size' ? a.sizeBytes - b.sizeBytes : sort === 'name' ? a.name.localeCompare(b.name) : b.seeds - a.seeds) ||
		b.seeds - a.seeds || a.name.localeCompare(b.name));
}

export function releaseQuery(title: string, year: number, scope: ReleaseScope): string {
	if (scope.type === 'movie') return `${title}${year ? ` ${year}` : ''}`;
	return `${title} S${String(scope.season).padStart(2, '0')}${scope.type === 'episode' ? `E${String(scope.episode).padStart(2, '0')}` : ''}`;
}
