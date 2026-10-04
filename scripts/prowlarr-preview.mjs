import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';

const configPath = process.env.LUMA_PROWLARR_CONFIG || fileURLToPath(new URL('../.artifacts/tools/prowlarr/data/config.xml', import.meta.url));
const downloads = new Map();

/** Resolve only an opaque release returned by this connector; credentials stay in the proxy.
 * @param {string} key @param {typeof fetch} fetcher
 */
export async function resolveProwlarrRelease(key, fetcher = fetch) {
	const entry = downloads.get(key);
	if (!entry || entry.until < Date.now()) throw new Error('Release expired. Search again.');
	let response;
	try { response = await fetcher(entry.url, { headers: entry.headers, redirect: 'manual', signal: AbortSignal.timeout(90000) }); }
	catch { throw new Error('Could not retrieve this release from Prowlarr.'); }
	if (response.status >= 300 && response.status < 400) {
		const target = response.headers.get('location') || '';
		if (/^magnet:\?xt=urn:bt(?:ih|mh):/i.test(target)) return { magnet: target };
		throw new Error('Prowlarr did not return a magnet or torrent file.');
	}
	if (!response.ok) throw new Error(`Prowlarr could not retrieve the release (HTTP ${response.status}).`);
	if (Number(response.headers.get('content-length')) > 5_000_000) throw new Error('Torrent metadata is too large.');
	const reader = response.body?.getReader();
	if (!reader) throw new Error('The torrent file was empty.');
	const chunks = []; let size = 0;
	try {
		while (true) {
			const { value, done } = await reader.read();
			if (done) break;
			size += value.length;
			if (size > 5_000_000) { await reader.cancel(); throw new Error('Torrent metadata is too large.'); }
			chunks.push(Buffer.from(value));
		}
	} finally { reader.releaseLock(); }
	const body = Buffer.concat(chunks);
	if (body[0] !== 100) throw new Error('Prowlarr did not return a valid torrent file.');
	return { torrent: body.toString('base64') };
}

/** @param {string} kind */
function mediaCategory(kind) {
	if (kind === 'movie') return 2000;
	if (kind === 'series') return 5000;
	throw new Error('Tipo de título inválido para a pesquisa.');
}

/** Only release metadata is exposed to the renderer.
 * @param {any} data @param {number} indexerId @param {string} kind @param {(item: any) => string | null} [register]
 */
export function normalizeProwlarrResults(data, indexerId, kind, register) {
	if (!Array.isArray(data)) throw new Error('Prowlarr retornou uma resposta inválida.');
	const category = mediaCategory(kind);
	const matches = data.filter((/** @type {any} */ item) => item.indexerId === indexerId
		&& Array.isArray(item.categories) && item.categories.some((/** @type {any} */ entry) => {
			const id = Number(entry.id);
			return id >= category && id < category + 1000;
		}));
	return {
		nextPage: null,
		note: 'via Prowlarr · resultados retornados pelo indexador',
		results: matches.map((/** @type {any} */ item) => ({
			downloadKey: register?.(item) ?? null,
			name: String(item.title || ''), source: '1337x',
			uploader: '', group: String(item.subGroup || ''),
			size: '', sizeBytes: Number(item.size) || 0,
			seeds: Number(item.seeders) || 0, peers: Number(item.leechers) || 0,
			infoHash: String(item.infoHash || ''), filename: String(item.fileName || ''),
			url: typeof item.infoUrl === 'string' && item.infoUrl.startsWith('https://') ? item.infoUrl : '',
			fileType: null
		}))
	};
}

/** @param {string} query @param {string} kind @param {typeof fetch} fetcher */
export async function searchProwlarr(query, kind, fetcher = fetch) {
	const category = mediaCategory(kind);
	let config;
	try { config = await readFile(configPath, 'utf8'); }
	catch { throw new Error('Prowlarr local não está instalado. Execute o launcher de Prowlarr do DEV.'); }
	const key = config.match(/<ApiKey>([^<]+)<\/ApiKey>/i)?.[1]?.trim();
	const port = Number(config.match(/<Port>(\d+)<\/Port>/i)?.[1] || 9696);
	if (!key || !Number.isSafeInteger(port) || port < 1 || port > 65535) throw new Error('Configuração local do Prowlarr inválida.');
	const headers = { 'X-Api-Key': key };
	/** @param {string} path */
	async function request(path) {
		let response;
		try { response = await fetcher(new URL(path, `http://127.0.0.1:${port}`), {
			headers, redirect: 'error', signal: AbortSignal.timeout(90000)
		}); }
		catch { throw new Error('Não foi possível consultar o Prowlarr local. Abra o painel e confira se ele está rodando.'); }
		if (!response.ok) throw new Error(`Prowlarr respondeu HTTP ${response.status}. Confira o teste do indexador 1337x no painel.`);
		try { return await response.json(); }
		catch { throw new Error('Prowlarr não retornou JSON válido.'); }
	}
	const indexers = await request('/api/v1/indexer');
	if (!Array.isArray(indexers)) throw new Error('Prowlarr retornou indexadores inválidos.');
	const indexer = indexers.find((/** @type {any} */ item) => String(item.definitionName || item.name).toLowerCase() === '1337x');
	if (!indexer) throw new Error('Adicione o indexador 1337x no painel do Prowlarr para conectar esta fonte.');
	if (!indexer.enable) throw new Error('O indexador 1337x está desativado no Prowlarr. Configure e teste o acesso no painel.');
	if (!Number.isSafeInteger(indexer.id) || indexer.id <= 0) throw new Error('Identificador do indexador 1337x inválido.');
	const params = new URLSearchParams({ query, type: 'search', indexerIds: String(indexer.id), categories: String(category) });
	return normalizeProwlarrResults(await request(`/api/v1/search?${params}`), indexer.id, kind, (item) => {
		if (typeof item.downloadUrl !== 'string') return null;
		let url;
		try { url = new URL(item.downloadUrl); } catch { return null; }
		if (url.protocol !== 'http:' || !['127.0.0.1', 'localhost'].includes(url.hostname) || Number(url.port) !== port ||
			!/^\/(?:api\/v1\/)?(?:\d+\/download|indexer\/\d+\/download)\/?$/.test(url.pathname)) return null;
		url.hostname = '127.0.0.1';
		for (const [key, entry] of downloads) if (entry.until < Date.now()) downloads.delete(key);
		while (downloads.size >= 512) downloads.delete(downloads.keys().next().value);
		const key = randomUUID();
		downloads.set(key, { url: url.href, headers, until: Date.now() + 15 * 60_000 });
		return key;
	});
}
