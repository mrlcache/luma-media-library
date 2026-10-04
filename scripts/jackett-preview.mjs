import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

/** Translate only metadata, keeping download URLs and credentials out of the client response.
 * @param {any} data
 */
export function normalizeJackettResults(data) {
	if (!data || !Array.isArray(data.Results) || !Array.isArray(data.Indexers)) throw new Error('Jackett retornou uma resposta inválida.');
	const indexer = data.Indexers.find((/** @type {any} */ item) => String(item.ID).toLowerCase() === '1337x');
	if (!indexer) throw new Error('Configure o indexador 1337x no Jackett.');
	if (indexer.Error) {
		if (/challenge|cloudflare|flaresolverr|captcha/i.test(indexer.Error)) throw new Error('1337x: verificação pendente no Jackett. Abra o painel e teste o indexador após configurar o acesso.');
		throw new Error('O indexador 1337x falhou. Consulte o teste no painel do Jackett.');
	}
	return { nextPage: null, note: 'via Jackett', results: data.Results.filter((/** @type {any} */ item) => String(item.TrackerId || '1337x').toLowerCase() === '1337x').map((/** @type {any} */ item) => ({
		name: String(item.Title || ''), source: '1337x', uploader: String(item.Author || item.Uploader || ''), group: String(item.ReleaseGroup || ''),
		size: '', sizeBytes: Number(item.Size) || 0, seeds: Number(item.Seeders) || 0,
		peers: Math.max(0, (Number(item.Peers) || 0) - (Number(item.Seeders) || 0)),
		infoHash: String(item.InfoHash || ''), url: typeof item.Details === 'string' && item.Details.startsWith('https://') ? item.Details : '',
		fileType: null
	})) };
}

/** @param {string} query @param {typeof fetch} fetcher */
export async function searchJackett(query, fetcher = fetch) {
	let config;
	try {
		config = JSON.parse(await readFile(join(process.env.ProgramData || 'C:/ProgramData', 'Jackett', 'ServerConfig.json'), 'utf8'));
	} catch { throw new Error('A configuração local do Jackett não foi encontrada.'); }
	if (!config.APIKey) throw new Error('Jackett não tem chave de API configurada.');
	const port = Number(config.Port || 9117);
	if (!Number.isSafeInteger(port) || port < 1 || port > 65535) throw new Error('Porta local do Jackett inválida.');
	const url = new URL(`http://127.0.0.1:${port}/api/v2.0/indexers/1337x/results`);
	url.search = new URLSearchParams({ apikey: config.APIKey, Query: query }).toString();
	let response;
	try { response = await fetcher(url, { signal: AbortSignal.timeout(90000), redirect: 'error' }); }
	catch { throw new Error('Não foi possível consultar o Jackett local. Confira se ele está aberto e teste o indexador no painel.'); }
	if (!response.ok) throw new Error(`Jackett respondeu HTTP ${response.status}. Confira a configuração no painel.`);
	let data;
	try { data = await response.json(); } catch { throw new Error('Jackett não retornou JSON válido.'); }
	return normalizeJackettResults(data);
}
