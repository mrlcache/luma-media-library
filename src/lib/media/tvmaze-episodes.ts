type TvMazeImage = { medium?: string | null; original?: string | null } | null;
type TvMazeSearchResult = {
	score: number;
	show: { id: number; name: string; premiered?: string | null };
};
type TvMazeEpisode = {
	season: number;
	number: number;
	image: TvMazeImage;
	summary?: string | null;
};

export type TvMazeEpisodeDetails = { image: string | null; summary: string | null };

const API = 'https://api.tvmaze.com';
const MAX_SHOWS = 32;
const REQUEST_INTERVAL_MS = 550;
const showEpisodes = new Map<string, Promise<Map<string, TvMazeEpisodeDetails>>>();
let requestQueue: Promise<void> = Promise.resolve();
let nextRequestAt = 0;

export function findTvMazeEpisodeImage(
	title: string,
	year: number | null,
	season: number | null,
	episode: number | null
): Promise<string | null> {
	return findTvMazeEpisodeDetails(title, year, season, episode).then((details) => details?.image ?? null);
}

export function findTvMazeEpisodeDetails(
	title: string,
	year: number | null,
	season: number | null,
	episode: number | null
): Promise<TvMazeEpisodeDetails | null> {
	if (!title.trim() || season === null || episode === null) return Promise.resolve(null);
	const cacheKey = `${normalize(title)}:${year ?? ''}`;
	let pending = showEpisodes.get(cacheKey);
	if (!pending) {
		pending = loadShowEpisodes(title, year);
		showEpisodes.set(cacheKey, pending);
		while (showEpisodes.size > MAX_SHOWS) {
			const oldest = showEpisodes.keys().next().value;
			if (oldest === undefined) break;
			showEpisodes.delete(oldest);
		}
	}
	return pending.then((details) => details.get(`${season}:${episode}`) ?? null);
}

async function loadShowEpisodes(title: string, year: number | null): Promise<Map<string, TvMazeEpisodeDetails>> {
	try {
		const results = await requestJson<TvMazeSearchResult[]>(`${API}/search/shows?q=${encodeURIComponent(title)}`);
		if (!results?.length) return new Map();

		const normalizedTitle = normalize(title);
		const candidates = results
			.filter((result) => Number.isInteger(result.show?.id))
			.sort((left, right) => {
				const leftExact = normalize(left.show.name) === normalizedTitle ? 1 : 0;
				const rightExact = normalize(right.show.name) === normalizedTitle ? 1 : 0;
				if (leftExact !== rightExact) return rightExact - leftExact;
				const leftYear = yearDistance(left.show.premiered, year);
				const rightYear = yearDistance(right.show.premiered, year);
				if (leftYear !== rightYear) return leftYear - rightYear;
				return right.score - left.score;
			});
		const match = candidates[0];
		if (!match || (normalize(match.show.name) !== normalizedTitle && match.score < 0.72)) return new Map();

		const episodes = await requestJson<TvMazeEpisode[]>(`${API}/shows/${match.show.id}/episodes`);
		const details = new Map<string, TvMazeEpisodeDetails>();
		for (const item of episodes ?? []) {
			const url = validImageUrl(item.image?.original ?? item.image?.medium);
			if (Number.isInteger(item.season) && Number.isInteger(item.number)) {
				details.set(`${item.season}:${item.number}`, { image: url, summary: plainText(item.summary) });
			}
		}
		return details;
	} catch {
		return new Map();
	}
}

function plainText(value: string | null | undefined): string | null {
	if (!value?.trim()) return null;
	if (typeof DOMParser !== 'undefined') {
		const parsed = new DOMParser().parseFromString(value, 'text/html');
		const text = parsed.body.textContent?.replace(/\s+/g, ' ').trim();
		return text || null;
	}
	const text = value.replace(/<[^>]*>/g, ' ').replace(/&nbsp;/gi, ' ').replace(/&amp;/gi, '&').replace(/&quot;/gi, '"').replace(/&#39;/gi, "'").replace(/&lt;/gi, '<').replace(/&gt;/gi, '>').replace(/\s+/g, ' ').trim();
	return text || null;
}

function requestJson<T>(url: string): Promise<T | null> {
	const request = requestQueue.then(async () => {
		const wait = Math.max(0, nextRequestAt - Date.now());
		if (wait > 0) await new Promise((resolve) => window.setTimeout(resolve, wait));
		nextRequestAt = Date.now() + REQUEST_INTERVAL_MS;
		const response = await fetch(url, { headers: { Accept: 'application/json' } });
		if (!response.ok) return null;
		return (await response.json()) as T;
	});
	requestQueue = request.then(() => undefined, () => undefined);
	return request;
}

function normalize(value: string): string {
	return value.normalize('NFKD').replace(/[\u0300-\u036f]/g, '').toLowerCase().replace(/[^a-z0-9]/g, '');
}

function yearDistance(premiered: string | null | undefined, year: number | null): number {
	if (!year || !premiered) return 1000;
	const premieredYear = Number(premiered.slice(0, 4));
	return Number.isFinite(premieredYear) ? Math.abs(premieredYear - year) : 1000;
}

function validImageUrl(value: string | null | undefined): string | null {
	if (!value) return null;
	try {
		const image = new URL(value);
		return image.protocol === 'https:' && image.hostname === 'static.tvmaze.com' ? image.href : null;
	} catch {
		return null;
	}
}
