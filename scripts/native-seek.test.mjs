import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';
const source = await readFile(new URL('../src/lib/platform/native-seek.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText;
const { NativeSeekTimeline } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);
test('old polls and old seek responses cannot undo a newer seek', () => {
 const timeline = new NativeSeekTimeline();
 const old = timeline.generation;
 const first = timeline.request(110, 0);
 const last = timeline.request(120, 20);
 assert.equal(timeline.position(100, old, 25), null);
 assert.equal(timeline.position(110, first, 25), null);
 assert.equal(timeline.position(100, last, 25), null);
 assert.equal(timeline.position(120.1, last, 100), 120.1);
 assert.equal(timeline.seeking, false);
});
test('backward seeks ignore the previous position and recover if the engine cannot seek', () => {
 const timeline = new NativeSeekTimeline();
 const generation = timeline.request(80, 0);
 assert.equal(timeline.position(100, generation, 100), null);
 assert.equal(timeline.position(79, generation, 200), null);
 assert.equal(timeline.position(80.02, generation, 300), 80.02);
 const failed = timeline.request(50, 400);
 assert.equal(timeline.position(81, failed, 5500), 81);
 assert.equal(timeline.seeking, false);
});
test('reset invalidates in-flight results from the previous title', () => {
 const timeline = new NativeSeekTimeline();
 const old = timeline.request(70, 0);
 timeline.reset();
 assert.equal(timeline.position(70, old, 100), null);
 assert.equal(timeline.position(0, timeline.generation, 100), 0);
});
