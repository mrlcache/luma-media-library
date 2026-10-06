import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { searchProwlarr, resolveProwlarrRelease } from './prowlarr-preview.mjs';
import { createReleaseCache } from './release-cache.mjs';

// Shared release resolver used by development previews and the desktop service.
/** @param {{ cachePath?: string, now?: () => number, requestHandler?: (params: URLSearchParams) => Promise<any> }} [options] */
export function createReleaseSearch({ cachePath, now = Date.now, requestHandler } = {}) {
	const cache = createReleaseCache({ filePath: cachePath, now });
	const refreshes = new Map();
	const lastRefreshAttempt = new Map();
	/** @type {Map<string, Promise<void>>} */
	const warming = new Map();
	const warmTails = [Promise.resolve(), Promise.resolve()];
	let warmSlot = 0;
	let lastPrune = 0;
	const refreshAfterMs = 20 * 60 * 1000;
	const refreshCooldownMs = 20 * 60 * 1000;
	/** @param {any} data */
	function addMagnets(data) {
		if (!Array.isArray(data?.results)) return data;
		return { ...data, results: data.results.map(/** @param {any} item */ (item) => {
			if ((typeof item?.magnet === 'string' && /^magnet:\?xt=urn:bt(?:ih|mh):/i.test(item.magnet)) || typeof item?.infoHash !== 'string') return item;
			const hash = item.infoHash.trim();
			if (!/^(?:[a-f0-9]{40}|[a-z2-7]{32}|[a-f0-9]{64})$/i.test(hash)) return item;
			const urn = hash.length === 64 && /^[a-f0-9]+$/i.test(hash) ? `btmh:1220${hash}` : `btih:${hash}`;
			return { ...item, magnet: `magnet:?xt=urn:${urn}&dn=${encodeURIComponent(String(item.name || ''))}` };
		}) };
	}
	/** @param {string} url @param {boolean} json @param {Record<string, string>} headers @returns {Promise<any>} */
	async function request(url, json = true, headers = {}) {
		const response = await fetch(url, { signal: AbortSignal.timeout(15000), headers: { 'User-Agent': 'Luma/0.1 (search preview)', ...headers } });
		if (!response.ok) throw new Error(`A fonte respondeu HTTP ${response.status}.`);
		const body = await response.text();
		if (body.length > 5_000_000) throw new Error('Resposta grande demais.');
		if (!json) return body;
		try { return JSON.parse(body); } catch { throw new Error('A fonte não retornou JSON válido.'); }
	}
	/** @param {string} path @param {Record<string, string>} params */
	async function tmdb(path, params = {}) {
		let token;
		try { token = (await readFile(join(process.env.LUMA_APP_DATA || join(process.env.APPDATA || '', 'local.media.platform'), 'tmdb_read_access_token'), 'utf8')).trim(); }
		catch { throw new Error('Configure a chave do TMDb nas configurações do Luma.'); }
		if (!token) throw new Error('A chave do TMDb está vazia.');
		return request(`https://api.themoviedb.org/3/${path}?${new URLSearchParams(params)}`, true, { Authorization: `Bearer ${token}` });
	}
	/** @param {URLSearchParams} params */
	async function handle(params) {
		const action = params.get('action');
		if (action === 'resolve') return resolveProwlarrRelease(params.get('key') || '');
		const query = (params.get('query') || '').trim().slice(0, 120);
		if (action === 'titles') {
			if (query.length < 2) return { titles: [] };
			const data = await tmdb('search/multi', { query, include_adult: 'false', language: 'en-US' });
			return { titles: (data.results || []).filter((/** @type {any} */ item) => ['movie', 'tv'].includes(item.media_type)).slice(0, 20).map((/** @type {any} */ item) => ({
				id: item.id, title: item.title || item.name, kind: item.media_type === 'tv' ? 'series' : 'movie',
				year: Number((item.release_date || item.first_air_date || '').slice(0, 4)) || null,
				overview: item.overview || '', voteAverage: item.vote_average || null,
				posterUrl: item.poster_path ? `https://image.tmdb.org/t/p/w342${item.poster_path}` : null,
				backdropUrl: item.backdrop_path ? `https://image.tmdb.org/t/p/w1280${item.backdrop_path}` : null
			})) };
		}
		if (action !== 'source' || query.length < 2) throw new Error('Pesquisa inválida.');
		const source = params.get('source');
		const page = Number(params.get('page') || '1');
		if (!Number.isSafeInteger(page) || page < 1 || page > 10000) throw new Error('Página inválida.');
		const kind = params.get('kind') || '';
		if (!['movie', 'series'].includes(kind)) throw new Error('Tipo de título inválido.');
		if (source === '1337x') {
			return searchProwlarr(query, kind);
		}
		const id = Number(params.get('id'));
		if (!Number.isSafeInteger(id) || id <= 0 || !['movie', 'series'].includes(kind)) throw new Error('Título inválido.');
		const external = await tmdb(`${kind === 'series' ? 'tv' : 'movie'}/${id}/external_ids`);
		if (source === 'YTS') {
			if (kind !== 'movie') return { results: [], note: 'YTS pesquisa filmes.' };
			const data = await request(`https://movies-api.accel.li/api/v2/list_movies.json?${new URLSearchParams({ query_term: external.imdb_id || query, limit: '50', page: String(page) })}`);
			if (data.status !== 'ok') throw new Error('YTS não conseguiu concluir a pesquisa.');
			return { nextPage: page * 50 < Number(data.data?.movie_count || 0) ? page + 1 : null, results: (data.data?.movies || []).flatMap((/** @type {any} */ movie) => (movie.torrents || []).map((/** @type {any} */ torrent) => ({
				name: `${movie.title} (${movie.year}) ${torrent.quality} ${torrent.type || ''} ${torrent.video_codec || ''}`.trim(),
				size: torrent.size || '', sizeBytes: Number(torrent.size_bytes) || 0, seeds: Number(torrent.seeds) || 0,
				peers: Number(torrent.peers) || 0, uploader: 'YTS', group: 'YTS', quality: torrent.quality || '', codec: torrent.video_codec || '', releaseType: torrent.type || '',
				fileType: null, infoHash: torrent.hash || '', url: movie.url, source: 'YTS'
			}))) };
		}
		if (source === 'EZTV') {
			if (page > 100) throw new Error('A API do EZTV permite até 100 páginas.');
			if (kind !== 'series') return { results: [], note: 'EZTV pesquisa séries.' };
			if (!external.imdb_id) return { results: [], note: 'Esta série não tem IMDb associado no TMDb.' };
			const data = await request(`https://eztvx.to/api/get-torrents?${new URLSearchParams({ imdb_id: external.imdb_id.replace(/^tt/, ''), limit: '100', page: String(page) })}`);
			if (!Array.isArray(data.torrents)) throw new Error('EZTV não retornou uma lista válida.');
			const hasMore = data.torrents.length > 0 && (data.torrents_count != null ? page * 100 < Number(data.torrents_count) : data.torrents.length === 100);
			return { nextPage: hasMore && page < 100 ? page + 1 : null, note: hasMore && page === 100 ? 'Limite de 100 páginas da API do EZTV atingido.' : '',
				results: data.torrents.map((/** @type {any} */ torrent) => ({ name: torrent.title, filename: torrent.filename || '', size: '', sizeBytes: Number(torrent.size_bytes) || 0,
				seeds: Number(torrent.seeds) || 0, peers: Number(torrent.peers) || 0, uploader: torrent.uploader || '', infoHash: torrent.hash || '',
				url: torrent.episode_url || '', source: 'EZTV' })) };
		}
		throw new Error('Fonte desconhecida.');
	}
	const execute = requestHandler || handle;
	/** Resolve opaque indexer links while the provider is online, with two workers. @param {any} data */
	function preserveDownloads(data) {
		if (!Array.isArray(data?.results)) return;
		for (const item of data.results) {
			if (item.magnet || typeof item.downloadKey !== 'string' || warming.has(item.downloadKey)) continue;
			const key = item.downloadKey;
			const slot = warmSlot++ % warmTails.length;
			const task = warmTails[slot].catch(() => {}).then(async () => {
				const resolvedKey = `resolve:${key}`;
				if (await cache.get(resolvedKey).catch(() => null)) return;
				const resolved = await execute(new URLSearchParams({ action: 'resolve', key }));
				if (resolved?.magnet || resolved?.torrent) await cache.set(resolvedKey, resolved).catch(() => {});
			});
			warming.set(key, task);
			warmTails[slot] = task.catch(() => {});
			void task.finally(() => warming.delete(key)).catch(() => {});
		}
	}
	/** @param {string} key @param {URLSearchParams} params */
	async function refreshInBackground(key, params) {
		if (refreshes.has(key) || now() - (lastRefreshAttempt.get(key) || 0) < refreshCooldownMs) return;
		lastRefreshAttempt.set(key, now());
		const refresh = execute(params).then(async (data) => {
			const result = addMagnets(data);
			await cache.set(key, result, { viewed: false });
			preserveDownloads(result);
		}).catch(() => {}).finally(() => refreshes.delete(key));
		refreshes.set(key, refresh);
	}
	const search = async (/** @type {URLSearchParams} */ params) => {
		if (now() - lastPrune > 60 * 60 * 1000) {
			lastPrune = now();
			await cache.prune().catch(() => {});
			for (const [key, timestamp] of lastRefreshAttempt) if (now() - timestamp >= refreshCooldownMs) lastRefreshAttempt.delete(key);
		}
		const isResolve = params.get('action') === 'resolve';
		const forceRefresh = params.get('refresh') === '1';
		const normalized = new URLSearchParams(params);
		normalized.delete('refresh');
		normalized.sort();
		const cacheKey = isResolve ? `resolve:${params.get('key') || ''}` : `search:${normalized.toString()}`;
		const saved = await cache.get(cacheKey).catch(() => null);
		if (saved && !forceRefresh) {
			if (!isResolve && now() - saved.refreshedAt >= refreshAfterMs) void refreshInBackground(cacheKey, new URLSearchParams(params));
			preserveDownloads(saved.data);
			return saved.data;
		}
		try {
			const data = addMagnets(await execute(params));
			await cache.set(cacheKey, data).catch(() => {});
			preserveDownloads(data);
			return data;
		} catch (error) {
			if (!saved) throw error;
			preserveDownloads(saved.data);
			return saved.data;
		}
	};
	search.settled = async () => { await Promise.allSettled([...refreshes.values()]); await Promise.allSettled(warmTails); await cache.flush().catch(() => {}); };
	return search;
}
/** @returns {import('vite').Plugin} */
export function torrentSearchPreview() {
	const search=createReleaseSearch();
	return {
		name: 'luma-torrent-search-preview', apply: 'serve',
		configureServer(server) {
			server.middlewares.use(async (req, res, next) => {
				const url = new URL(req.url || '/', 'http://127.0.0.1');
				if (url.pathname !== '/__luma-preview/search') return next();
				res.setHeader('Content-Type', 'application/json; charset=utf-8');
				res.setHeader('Cache-Control', 'no-store');
				if (req.method !== 'GET') { res.statusCode = 405; return res.end(JSON.stringify({ error: 'Método inválido.' })); }
				try {
					res.end(JSON.stringify(await search(url.searchParams)));
				} catch (error) { res.statusCode = 502; res.end(JSON.stringify({ error: error instanceof Error ? error.message : 'A pesquisa falhou.' })); }
			});
		}
	};
}
