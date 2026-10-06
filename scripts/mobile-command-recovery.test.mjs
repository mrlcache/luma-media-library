import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(new URL('../src/lib/platform/mobile-command-recovery.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const { recoverMobileRead, mobileMediaOrigin } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('a transient playback connection failure retries once and gets a fresh response', async () => {
	let attempts = 0;
	const result = await recoverMobileRead('resolve_media_file', async () => {
		if (++attempts === 1) throw 'Cannot reach Luma Desktop: error sending request for url';
		return {path:'fresh'};
	});
	assert.equal(attempts, 2);
	assert.deepEqual(result, {path:'fresh'});
});
test('persistent failure stops after one retry', async () => {
	let attempts = 0;
	await assert.rejects(recoverMobileRead('get_local_title_detail', async () => { attempts++; throw new Error('connection reset'); }), /connection reset/);
	assert.equal(attempts, 2);
});

test('browser network failures recover without requiring new pairing', async () => {
	let attempts = 0;
	const value = await recoverMobileRead('resolve_media_file', async () => {
		if (++attempts === 1) throw new TypeError('Failed to fetch');
		return 'recovered';
	});
	assert.equal(value, 'recovered');
	assert.equal(attempts, 2);
});
test('edits and missing media errors are never replayed', async () => {
	for (const [command, message] of [['torrent_add_magnet','connection reset'],['resolve_media_file','This media file is no longer in the library'],['resolve_media_file','Desktop pairing is incomplete']]) {
		let attempts = 0;
		await assert.rejects(recoverMobileRead(command, async () => { attempts++; throw new Error(message); }), {message});
		assert.equal(attempts, 1);
	}
});
test('signed media uses the reachable paired origin and retains its signature and metadata', () => {
	const path = 'http://10.8.0.2:47631/api/v1/media/21?expires=123&signature=test-signature';
	const source = {path,streamUrl:path,subtitles:[{path:path.replace('/21?','/21/subtitles/0?'),language:'pt',supported:true}],subtitleWarning:'notice'};
	const result = mobileMediaOrigin('resolve_media_file', source, 'http://192.168.1.2:47640');
	assert.equal(result.path, path.replace('10.8.0.2:47631','192.168.1.2:47640'));
	assert.equal(result.streamUrl, result.path);
	assert.equal(result.subtitles[0].path, result.path.replace('/21?','/21/subtitles/0?'));
	assert.equal(result.subtitles[0].language,'pt');
	assert.equal(result.subtitleWarning,'notice');
	assert.equal(source.path,path);
});
test('local files, external URLs, unsigned addresses and unrelated commands are unchanged', () => {
	for (const path of ['C:\\Video.mp4','http://external.example/trailer.mp4','http://external.example/api/v1/media/21']) {
		assert.equal(mobileMediaOrigin('resolve_media_file',{path},'http://192.168.1.2:47631').path,path);
	}
	const value = {path:'http://external.example/api/v1/media/21?expires=123&signature=test'};
	assert.equal(mobileMediaOrigin('get_title_trailer',value,'http://192.168.1.2:47631'),value);
	assert.equal(mobileMediaOrigin('resolve_media_file',value,null),value);
});
