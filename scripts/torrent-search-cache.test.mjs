import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createReleaseSearch } from './torrent-search-preview.mjs';

const directory = await mkdtemp(join(tmpdir(), 'luma-search-service-'));
const cachePath = join(directory, 'release-cache.json');
const params = new URLSearchParams({ action: 'source', source: 'YTS', id: '42', kind: 'movie', query: 'Sample Film 2025', page: '1' });
const hash = 'a'.repeat(40);
try {
	let time = 2_000_000;
	const first = createReleaseSearch({ cachePath, now: () => time, requestHandler: async () => ({ nextPage: null, results: [{ name: 'Sample Film 2025 1080p', infoHash: hash }] }) });
	const searched = await first(params);
	assert.equal(searched.results[0].magnet, `magnet:?xt=urn:btih:${hash}&dn=Sample%20Film%202025%201080p`, 'search results include a usable magnet');

	// A new service instance represents restarting the app; cached results remain usable.
	time += 21 * 60 * 1000;
	const reopened = createReleaseSearch({ cachePath, now: () => time, requestHandler: async () => { throw new Error('source offline'); } });
	const offline = await reopened(params);
	assert.equal(offline.results[0].name, 'Sample Film 2025 1080p', 'stored results are returned when the provider is down');
	assert.equal(offline.results[0].magnet, searched.results[0].magnet, 'stored magnet remains available offline');
	await reopened.settled();
	const reordered = new URLSearchParams([...params.entries()].reverse());
	assert.equal((await reopened(reordered)).results[0].name, searched.results[0].name, 'parameter order does not invalidate the cache');
	const forced = new URLSearchParams(params); forced.set('refresh', '1');
	assert.equal((await reopened(forced)).results[0].magnet, searched.results[0].magnet, 'failed manual refresh retains the saved list');
	const updated = createReleaseSearch({ cachePath, now: () => time, requestHandler: async () => ({ results: [{ name:'Updated release', infoHash:hash }] }) });
	assert.equal((await updated(forced)).results[0].name, 'Updated release', 'manual refresh actually queries the provider');
	const opaque = createReleaseSearch({ cachePath, now: () => time, requestHandler: async request => request.get('action') === 'resolve'
		? { magnet: searched.results[0].magnet }
		: { results: [{ name:'Opaque release', infoHash:'', downloadKey:'temporary-indexer-key' }] } });
	await opaque(new URLSearchParams({ action:'source', query:'Opaque sample', source:'1337x' }));
	await opaque.settled();
	const afterRestart = createReleaseSearch({ cachePath, now: () => time, requestHandler: async () => { throw new Error('indexer offline'); } });
	assert.equal((await afterRestart(new URLSearchParams({ action:'resolve', key:'temporary-indexer-key' }))).magnet, searched.results[0].magnet, 'opaque downloads are resolved and retained before the indexer goes offline');
	const emptyParams = new URLSearchParams({action:'source', source:'1337x', query:'Recovered indexer'});
	let online = false;
	const recovering = createReleaseSearch({cachePath, now:() => time, requestHandler:async () => ({results:online ? [{name:'Recovered release', infoHash:hash}] : []})});
	assert.equal((await recovering(emptyParams)).results.length, 0);
	online = true;
	time += 30_001;
	assert.equal((await recovering(emptyParams)).results.length, 1, 'empty lists expire quickly when the indexer recovers');
	const failedEmptyParams = new URLSearchParams({action:'source', source:'1337x', query:'Unavailable indexer'});
	const empty = createReleaseSearch({cachePath, now:() => time, requestHandler:async () => ({results:[]})});
	await empty(failedEmptyParams);
	time += 30_001;
	const unavailable = createReleaseSearch({cachePath, now:() => time, requestHandler:async () => {throw new Error('proxy offline');}});
	await assert.rejects(unavailable(failedEmptyParams), /proxy offline/, 'an empty cached list must not hide a provider error');
	await Promise.all([first.settled(), reopened.settled(), updated.settled(), opaque.settled(), afterRestart.settled(), recovering.settled(), empty.settled(), unavailable.settled()]);
	console.log('Torrent search persistence and offline fallback checks passed.');
} finally {
	await rm(directory, { recursive: true, force: true });
}
