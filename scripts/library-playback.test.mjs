import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(new URL('../src/lib/platform/library-playback.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const { identifyLibraryPlayback, verifyPlaybackIdentity } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);
const detail = (id, title, files, tmdbId = 42, kind = 'series') => ({media:{id,title,kind},files,tmdbId});
const file = (mediaId, episode) => ({mediaId,season:1,episode});
function reader(details, pages) {
	return {detail:async id => details.get(id) ?? null, page:async (offset,count,query) => pages(offset,count,query)};
}
const page = items => ({items,total:items.length,offset:0});

test('a nonrepresentative episode ID resolves through its series without an episode label', async () => {
	const series = detail(10,'MobLand',[file(10,1),file(11,2)]);
	assert.equal(await identifyLibraryPlayback(reader(new Map([[10,series]]),()=>page([series.media])),11,'MobLand',undefined,42,'series'),11);
});
test('separated episode labels verify the exact legacy file and reject a changed ID', async () => {
	const series = detail(10,'MobLand',[file(10,1),file(11,2),file(12,3)]);
	for (const label of ['S01 · E03','S01E03','S01 - E03']) {
		assert.equal(await identifyLibraryPlayback(reader(new Map([[10,series]]),()=>page([series.media])),12,'MobLand',label,42,'series'),12);
		await assert.rejects(identifyLibraryPlayback(reader(new Map([[10,series]]),()=>page([series.media])),99,'MobLand',label,42,'series'));
	}
});
test('API identity resolves localized titles even when the text search is empty', async () => {
	const movie = detail(10,'Título local',[file(10,null)],42,'movie');
	assert.equal(await identifyLibraryPlayback(reader(new Map([[10,movie]]),(offset,count,query)=>page(query?[]:[movie.media])),10,'API title',undefined,42,'movie'),10);
});
test('matching ignores punctuation without a TMDB identity', async () => {
	const movie = detail(10,'Marvel’s Zombies',[file(10,null)],null,'movie');
	assert.equal(await identifyLibraryPlayback(reader(new Map([[10,movie]]),()=>page([movie.media])),10,"Marvels Zombies",undefined,undefined,'movie'),10);
});
test('recovery checks every catalog page instead of stopping at the first 48 results', async () => {
	const wrong = detail(1,'Same title',[file(1,1)],12);
	const right = detail(10,'Same title',[file(10,1),file(11,2)],42);
	const calls = [];
	const r = reader(new Map([[1,wrong],[10,right]]),(offset,count)=>{
		calls.push(offset); return {items:offset===0?[wrong.media]:[right.media],total:2,offset};
	});
	assert.equal(await identifyLibraryPlayback(r,11,'Same title','S01 · E02',42,'series'),11);
	assert.deepEqual(calls,[0,1]);
});

test('UUID responses are confirmed and missing or mismatched identities never fall back to numeric IDs', () => {
	const source = {mediaId:19,playbackUuid:'uuid-file-1',path:'video',subtitles:[],resumePositionSeconds:0};
	assert.equal(verifyPlaybackIdentity(source,'uuid-file-1'),source);
	assert.throws(()=>verifyPlaybackIdentity({...source,playbackUuid:'uuid-file-2'},'uuid-file-1'));
	assert.throws(()=>verifyPlaybackIdentity({...source,playbackUuid:undefined},'uuid-file-1'));
});
test('a different TMDB title and ambiguous episode versions are never guessed', async () => {
	const wrong = detail(10,'MobLand',[file(10,1)],12);
	await assert.rejects(identifyLibraryPlayback(reader(new Map([[10,wrong]]),()=>page([wrong.media])),99,'MobLand','S01E01',42,'series'));
	const ambiguous = detail(10,'MobLand',[file(10,1),file(11,1)]);
	await assert.rejects(identifyLibraryPlayback(reader(new Map([[10,ambiguous]]),()=>page([ambiguous.media])),99,'MobLand','S01E01',42,'series'));
	assert.equal(await identifyLibraryPlayback(reader(new Map([[10,ambiguous]]),()=>page([ambiguous.media])),11,'MobLand','S01E01',42,'series'),11);
});
