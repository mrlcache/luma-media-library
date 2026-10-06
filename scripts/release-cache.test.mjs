import assert from 'node:assert/strict';
import { mkdtemp, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createReleaseCache, RELEASE_CACHE_TTL_MS } from './release-cache.mjs';

const directory = await mkdtemp(join(tmpdir(), 'luma-release-cache-'));
const filePath = join(directory, 'cache.json');
try {
	let time = 1_000_000;
	let cache = createReleaseCache({ filePath, now: () => time });
	await cache.set('source=QXR&page=1', { results: [{ name: 'Sample', magnet: 'magnet:?xt=urn:btih:abc' }] });
	time += 5 * 24 * 60 * 60 * 1000;
	cache = createReleaseCache({ filePath, now: () => time });
	assert.equal((await cache.get('source=QXR&page=1'))?.data.results[0].magnet, 'magnet:?xt=urn:btih:abc', 'data survives a service restart');
	time += RELEASE_CACHE_TTL_MS - 1;
	assert.ok(await cache.get('source=QXR&page=1'), 'viewing the entry slides its expiration window');
	time += RELEASE_CACHE_TTL_MS + 1;
	assert.equal(await cache.get('source=QXR&page=1'), null, 'unseen entries expire after ten days');
	await cache.set('expired-on-restart', { results:[] });
	time += RELEASE_CACHE_TTL_MS + 1;
	cache = createReleaseCache({ filePath, now: () => time });
	await cache.prune();
	assert.equal(JSON.parse(await readFile(filePath, 'utf8')).entries.length, 0, 'pruning on startup removes expired records from disk');
	const second = createReleaseCache({ filePath, now: () => time });
	await Promise.all([cache.get('absent'), second.get('absent')]);
	await Promise.all([cache.set('first-service', { value: 1 }), second.set('second-service', { value: 2 })]);
	const stored = new Map(JSON.parse(await readFile(filePath, 'utf8')).entries);
	assert.equal(stored.get('first-service').data.value, 1, 'concurrent services preserve each other’s entries');
	assert.equal(stored.get('second-service').data.value, 2);
	console.log('Persistent release cache checks passed.');
} finally {
	await rm(directory, { recursive: true, force: true });
}
