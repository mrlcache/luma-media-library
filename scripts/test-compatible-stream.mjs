import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

class FakeTarget {
	listeners = new Map();

	addEventListener(name, listener, options = {}) {
		const entries = this.listeners.get(name) ?? [];
		entries.push({ listener, once: !!options.once });
		this.listeners.set(name, entries);
	}

	removeEventListener(name, listener) {
		this.listeners.set(name, (this.listeners.get(name) ?? []).filter(entry => entry.listener !== listener));
	}

	dispatch(name) {
		for (const entry of [...(this.listeners.get(name) ?? [])]) {
			entry.listener.call(this, { type: name, target: this });
			if (entry.once) this.removeEventListener(name, entry.listener);
		}
	}
}

class FakeSourceBuffer extends FakeTarget {
	mediaBuffered = false;
	appendError;
	buffered = {
		get length() { return this.owner.mediaBuffered ? 1 : 0; },
		start: () => 0,
		end: () => 1,
		owner: this
	};

	constructor(appendError) {
		super();
		this.appendError = appendError;
	}

	appendBuffer(data) {
		if (this.appendError) throw this.appendError;
		if (new Uint8Array(data)[0] === 2) this.mediaBuffered = true;
		queueMicrotask(() => this.dispatch('updateend'));
	}
}

class FakeMediaSource extends FakeTarget {
	static isTypeSupported() { return true; }
	readyState = 'closed';
	buffer;
	duration;

	constructor({ appendError } = {}) {
		super();
		this.buffer = new FakeSourceBuffer(appendError);
		queueMicrotask(() => {
			this.readyState = 'open';
			this.dispatch('sourceopen');
		});
	}

	addSourceBuffer() { return this.buffer; }
	endOfStream() { this.readyState = 'ended'; }
}

function responseFor(chunks, { holdAfterChunks = false, appendError } = {}) {
	let index = 0;
	let pendingRead;
	let cancelled = false;
	const buffer = {
		getReader() {
			return {
				read() {
					if (index < chunks.length) return Promise.resolve({ value: Uint8Array.of(chunks[index++]), done: false });
					if (holdAfterChunks) return new Promise(resolve => { pendingRead = resolve; });
					return Promise.resolve({ done: true });
				},
				cancel() {
					cancelled = true;
					pendingRead?.({ done: true });
					return Promise.resolve();
				}
			};
		}
	};
	return {
		response: { ok: true, status: 200, body: buffer },
		cancelled: () => cancelled,
		appendError
	};
}

const source = await readFile(new URL('../src/lib/platform/compatible-stream.ts', import.meta.url), 'utf8');
const helper = ts.transpileModule(source, {
	compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 }
}).outputText.replace(/^export /gm, '');

function openFixture({ fetch, timeout = 100, appendError } = {}) {
	let mediaSource;
	const revoked = [];
	const objectUrl = {
		createObjectURL(source) { mediaSource = source; return 'blob:fixture'; },
		revokeObjectURL(url) { revoked.push(url); }
	};
	const FakeMediaSourceClass = class extends FakeMediaSource {
		constructor() {
			super({ appendError });
			mediaSource = this;
		}
	};
	const open = new Function('MediaSource', 'URL', 'fetch', 'AbortController', 'DOMException', `${helper}\nreturn openCompatibleStream;`)(
		FakeMediaSourceClass,
		objectUrl,
		fetch,
		AbortController,
		DOMException
	);
	const video = { currentTime: 0, src: '', pause() {}, load() {}, removeAttribute() {} };
	const errors = [];
	const stream = open(video, '/stream', 120, error => errors.push(error), { startupTimeoutMs: timeout });
	return { stream, errors, video, revoked, mediaSource: () => mediaSource };
}

test('ready waits for an actual buffered media range, then the stream completes', async () => {
	const body = responseFor([1, 2]);
	const fixture = openFixture({ fetch: async () => body.response });
	await fixture.stream.ready;
	assert.equal(fixture.mediaSource().buffer.buffered.length, 1);
	assert.deepEqual(fixture.errors, []);
	fixture.stream.stop();
	assert.deepEqual(fixture.revoked, ['blob:fixture']);
});

test('a fetch failure rejects ready and immediately aborts and reports the stream', async () => {
	let requestSignal;
	const failure = new Error('fixture fetch failed');
	const fixture = openFixture({
		fetch: async (_url, options) => {
			requestSignal = options.signal;
			throw failure;
		}
	});
	await assert.rejects(fixture.stream.ready, /fixture fetch failed/);
	assert.equal(fixture.errors[0], failure);
	assert.equal(requestSignal.aborted, true);
	assert.deepEqual(fixture.revoked, ['blob:fixture']);
});

test('a stream that sends only its init segment hits the startup deadline and cancels its reader', async () => {
	const body = responseFor([1], { holdAfterChunks: true });
	const fixture = openFixture({ fetch: async () => body.response, timeout: 25 });
	await assert.rejects(fixture.stream.ready, /did not send media data in time/);
	assert.match(fixture.errors[0].message, /did not send media data in time/);
	assert.equal(body.cancelled(), true);
	assert.deepEqual(fixture.revoked, ['blob:fixture']);
});

test('a synchronous append failure rejects the pending append and stops the request', async () => {
	const body = responseFor([2]);
	const fixture = openFixture({ fetch: async () => body.response, appendError: new Error('append failed') });
	await assert.rejects(fixture.stream.ready, /append failed/);
	assert.match(fixture.errors[0].message, /append failed/);
	assert.equal(body.cancelled(), true);
	assert.deepEqual(fixture.revoked, ['blob:fixture']);
});

test('an explicit stop rejects pending startup without reporting a playback failure', async () => {
	const body = responseFor([1], { holdAfterChunks: true });
	const fixture = openFixture({ fetch: async () => body.response });
	await new Promise(resolve => setTimeout(resolve, 0));
	fixture.stream.stop();
	await assert.rejects(fixture.stream.ready, { name: 'AbortError' });
	assert.deepEqual(fixture.errors, []);
	assert.equal(body.cancelled(), true);
	assert.deepEqual(fixture.revoked, ['blob:fixture']);
});
