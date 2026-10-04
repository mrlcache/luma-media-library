import assert from 'node:assert/strict';
import { normalizeProwlarrResults, searchProwlarr, resolveProwlarrRelease } from './prowlarr-preview.mjs';

const releases = [{ indexerId: 3, title: 'Example 2160p HEVC-GROUP', categories: [{ id: 2040 }], subGroup: 'GROUP', size: 1024, seeders: 8, leechers: 2,
	infoUrl: 'https://example.com/release', infoHash: 'hash', downloadUrl: 'http://localhost/private?apikey=secret', magnetUrl: 'magnet:private' },
	{ indexerId: 7, title: 'Other provider', categories: [{ id: 2000 }] },
	{ indexerId: 3, title: 'Example game', categories: [{ id: 4050 }] },
	{ indexerId: 3, title: 'Example show', categories: [{ id: 5040 }] },
	{ indexerId: 3, title: 'Example book', categories: [{ id: 7000 }] },
	{ indexerId: 3, title: 'Example with unknown category' }];
const normalized = normalizeProwlarrResults(releases, 3, 'movie');
assert.equal(normalized.results.length, 1);
assert.equal(normalized.results[0].group, 'GROUP');
assert.equal(normalized.results[0].uploader, '');
assert.equal(normalized.results[0].peers, 2);
assert.equal(normalized.results[0].downloadUrl, undefined);
assert.equal(normalized.results[0].magnetUrl, undefined);
assert.equal(normalizeProwlarrResults([{ indexerId: 3, categories: [{ id: 2000 }], infoUrl: 'javascript:bad' }], 3, 'movie').results[0].url, '');
assert.equal(normalizeProwlarrResults(releases, 3, 'series').results[0].name, 'Example show');
assert.equal(normalizeProwlarrResults(releases, 3, 'series').results.length, 1);
assert.throws(() => normalizeProwlarrResults({}, 3, 'movie'), /inválida/);

let requests = 0;
await searchProwlarr('Example', 'movie', async (input, options) => {
	const url = new URL(String(input));
	assert.equal(url.hostname, '127.0.0.1');
	assert.equal(url.searchParams.has('apikey'), false);
	assert.ok(options.headers['X-Api-Key']);
	requests++;
	if (url.pathname === '/api/v1/indexer') return Response.json([{ id: 3, name: 'My 1337x', definitionName: '1337x', enable: true }]);
	assert.equal(url.pathname, '/api/v1/search');
	assert.equal(url.searchParams.get('indexerIds'), '3');
	assert.equal(url.searchParams.get('query'), 'Example');
	assert.equal(url.searchParams.get('categories'), '2000');
	return Response.json(releases);
});
assert.equal(requests, 2);

for (const indexers of [[], [{ id: 3, definitionName: '1337x', enable: false }]]) {
	let calls = 0;
	await assert.rejects(searchProwlarr('Example', 'movie', async () => { calls++; return Response.json(indexers); }), /Adicione|desativado/);
	assert.equal(calls, 1, 'Missing or disabled indexers must never trigger a site search');
}
await assert.rejects(searchProwlarr('Example', 'movie', async () => { throw new Error('private URL with secret'); }), /Não foi possível/);
await assert.rejects(searchProwlarr('Example', 'game', async () => { throw new Error('Should not fetch'); }), /Tipo de título inválido/);
await searchProwlarr('Example', 'series', async (input) => {
	const url = new URL(String(input));
	if (url.pathname === '/api/v1/indexer') return Response.json([{ id: 3, definitionName: '1337x', enable: true }]);
	assert.equal(url.searchParams.get('categories'), '5000');
	return Response.json(releases);
});
console.log('Prowlarr connector checks passed with fixtures; no indexer searches made.');

const downloadable = await searchProwlarr('Example', 'series', async (input) => {
	const url = new URL(String(input));
	if (url.pathname === '/api/v1/indexer') return Response.json([{ id: 3, definitionName: '1337x', enable: true }]);
	return Response.json([{ indexerId: 3, title: 'Example S01E03', categories: [{ id: 5000 }],
		downloadUrl: new URL('/3/download?apikey=fixture-secret&link=opaque', url.origin).href }]);
});
const key = downloadable.results[0].downloadKey;
assert.match(key, /^[a-f0-9-]{36}$/);
assert.equal(JSON.stringify(downloadable).includes('fixture-secret'), false);
assert.deepEqual(await resolveProwlarrRelease(key, async () => new Response(null, { status: 302, headers: { location: 'magnet:?xt=urn:btih:0123456789012345678901234567890123456789' } })),
	{ magnet: 'magnet:?xt=urn:btih:0123456789012345678901234567890123456789' });
const torrent = 'd4:infod4:name7:exampleee';
assert.deepEqual(await resolveProwlarrRelease(key, async () => new Response(torrent)), { torrent: Buffer.from(torrent).toString('base64') });
await assert.rejects(resolveProwlarrRelease('unknown', async () => { throw new Error('Must not fetch'); }), /expired/);
await assert.rejects(resolveProwlarrRelease(key, async () => new Response('<html>error</html>')), /valid torrent/);
await assert.rejects(resolveProwlarrRelease(key, async () => new Response(null, { status: 302, headers: { location: 'https://example.com' } })), /magnet or torrent/);
console.log('Opaque release resolution checks passed; no downloads were started.');
