import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(new URL('../src/lib/components/PlayerOverlay.svelte', import.meta.url), 'utf8');
const helper = source.slice(source.indexOf('\tfunction tryCompatiblePlayback()'), source.indexOf('\tfunction onPlaybackError()'));
assert.ok(helper.includes('function tryCompatiblePlayback'));
const javascript = ts.transpileModule(helper, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
function fixture(overrides = {}) {
	const state = { mobilePlayer: true, previewOnly: false, canTranscode: true, activeTranscoding: false, transcoding: false, playerDisposed: false, currentTime: 123, resumePosition: 45, mobilePendingResume: false, ...overrides };
	const calls = [];
	const create = new Function(...Object.keys(state), 'rememberTranscoding', 'loadMobileStream', `${javascript}; return tryCompatiblePlayback;`);
	return { retry: create(...Object.values(state), () => calls.push('saved'), (...args) => calls.push(args)), calls };
}
test('unsupported remote video converts at the current position only once', () => {
	const { retry, calls } = fixture();
	assert.equal(retry(), true);
	assert.deepEqual(calls, ['saved', [123, false]]);
	assert.equal(retry(), false);
});
test('initial resume survives conversion', () => {
	const { retry, calls } = fixture({ currentTime: 0, mobilePendingResume: true });
	assert.equal(retry(), true);
	assert.deepEqual(calls, ['saved', [45, true]]);
});
test('local phone files and already converted streams do not loop', () => {
	for (const overrides of [{ canTranscode: false }, { activeTranscoding: true }, { transcoding: true }, { playerDisposed: true }, { mobilePlayer: false }, { previewOnly: true }]) {
		const { retry, calls } = fixture(overrides);
		assert.equal(retry(), false);
		assert.deepEqual(calls, []);
	}
});
