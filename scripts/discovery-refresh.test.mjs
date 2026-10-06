import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';

const stored = new Map();
globalThis.localStorage = { getItem:key=>stored.get(key)??null, setItem:(key,value)=>stored.set(key,value), removeItem:key=>stored.delete(key) };
let clock = 1_000_000, calls = 0;
const originalNow = Date.now;
Date.now = () => clock;
globalThis.__discoveryInvoke = async () => ({ featured:[], sections:[{title:`Feed ${++calls}`,items:[]}], warning:null });
let source = await readFile(new URL('../src/lib/media/discovery.ts',import.meta.url),'utf8');
source = source.replace("import { isDesktopRuntime } from '$lib/platform/desktop';",'const isDesktopRuntime=()=>true;')
    .replace("import { tmdbImageSize } from './artwork';",'const tmdbImageSize=(url)=>url;')
    .replaceAll("import('$lib/platform/invoke')",'Promise.resolve({invoke:globalThis.__discoveryInvoke})');
const output = ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
try {
    const {readDiscovery,cachedDiscovery}=await import(`data:text/javascript;base64,${Buffer.from(output).toString('base64')}`);
    await Promise.all([readDiscovery(),readDiscovery()]);
    assert.equal(calls,1,'concurrent refreshes share one request');
    clock+=4*60_000;
    await readDiscovery(); assert.equal(calls,1,'navigation reuses fresh results');
    clock+=60_001;
    await readDiscovery(); assert.equal(calls,2,'recommendations refresh after five minutes');
    await readDiscovery(true); assert.equal(calls,3,'history changes can refresh immediately');
    clock+=5*60_000;
    globalThis.__discoveryInvoke=async()=>{throw new Error('offline');};
    await assert.rejects(readDiscovery(),/offline/);
    assert.equal(cachedDiscovery().sections[0].title,'Feed 3','a failed refresh keeps the visible feed');
    console.log('Discovery refresh: expiry, coalescing, history invalidation and offline continuity passed.');
} finally { Date.now=originalNow; }
