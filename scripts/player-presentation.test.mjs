import assert from 'node:assert/strict';
import {readFile, mkdtemp, writeFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';
import {build} from 'vite';
const source = await readFile(new URL('../src/lib/platform/player-presentation-queue.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const {PlayerPresentationQueue} = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);
const settle = () => new Promise(resolve => setImmediate(resolve));

test('exit waits for an in-flight enter, so late Android commands cannot leave orientation locked', async () => {
  const calls = []; let finish;
  const queue = new PlayerPresentationQueue(value => {
    calls.push(value);
    return calls.length === 1 ? new Promise(resolve => { finish=resolve; }) : Promise.resolve();
  }, error => { throw error; });
  queue.update({active:true, controlsVisible:true});
  queue.update({active:true, controlsVisible:false});
  queue.update({active:false, controlsVisible:false});
  assert.equal(calls.length, 1);
  finish(); await settle();
  assert.deepEqual(calls, [{active:true,controlsVisible:true},{active:false,controlsVisible:false}]);
});

test('an Android error does not prevent subsequent exit or controls updates', async () => {
  const calls=[]; const errors=[];
  const queue=new PlayerPresentationQueue(async value => { calls.push(value); if(calls.length===1) throw Error('temporary failure'); }, error => errors.push(error));
  queue.update({active:true,controlsVisible:true});
  queue.update({active:false,controlsVisible:false});
  await settle();
  assert.equal(errors.length,1);
  assert.deepEqual(calls.at(-1),{active:false,controlsVisible:false});
});

test('production Android CSS keeps the global video blur reset and mobile filter variable', async () => {
  const app=await readFile(new URL('../src/app.css',import.meta.url),'utf8');
  const player=await readFile(new URL('../src/lib/components/PlayerOverlay.svelte',import.meta.url),'utf8');
  const reset=app.slice(app.indexOf('/* Disable the app\'s underlying glass layers'));
  const variable=player.match(/\.player-overlay--mobile\s*\{[^}]*--player-surface-filter[^}]+\}/)?.[0];
  assert.ok(variable);
  const directory=await mkdtemp(path.join(tmpdir(),'luma-immersive-css-'));
  assert.equal(path.dirname(path.resolve(directory)),path.resolve(tmpdir()));
  try {
    await writeFile(path.join(directory,'style.css'),reset+'\n'+variable);
    await writeFile(path.join(directory,'entry.js'),"import './style.css';");
    const result=await build({configFile:false,root:directory,logLevel:'silent',build:{write:false,target:'chrome87',rollupOptions:{input:path.join(directory,'entry.js')}}});
    const css=result.output.filter(item=>item.type==='asset'&&item.fileName.endsWith('.css')).map(item=>String(item.source)).join('\n');
    assert.match(css,/(?<!-webkit-)backdrop-filter\s*:\s*none\s*!important/);
    assert.match(css,/--player-surface-filter\s*:\s*none/);
    assert.match(css,/mobile-nav[^}]+visibility:hidden/);
  } finally { await rm(directory,{recursive:true,force:true}); }
});
