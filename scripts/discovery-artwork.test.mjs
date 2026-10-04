import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import ts from 'typescript';

// Exercise the real cache/preloader with deterministic IPC and image failures.
const stored=new Map();
globalThis.localStorage={getItem:key=>stored.get(key)??null,setItem:(key,value)=>stored.set(key,value),removeItem:key=>stored.delete(key)};
let calls=0,failDecode=false;
globalThis.__artworkInvoke=async()=>{calls++;return 'https://image.tmdb.org/t/p/original/logo.png';};
globalThis.Image=class {src='';decode(){return failDecode?Promise.reject(new Error('Network failure')):Promise.resolve();}};
let source=await readFile(new URL('../src/lib/media/discovery.ts',import.meta.url),'utf8');
source=source.replace("import { isDesktopRuntime } from '$lib/platform/desktop';",'const isDesktopRuntime=()=>true;')
 .replace("import { tmdbImageSize } from './artwork';",'const tmdbImageSize=(url,size)=>url?.replace(/original/,size);')
 .replaceAll("import('$lib/platform/invoke')",'Promise.resolve({invoke:globalThis.__artworkInvoke})');
const output=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText;
const load=seed=>import(`data:text/javascript;base64,${Buffer.from(output+`\n// session ${seed}`).toString('base64')}`);
const media={id:'tmdb-series-1',tmdbId:1,kind:'series'};
const first=await load(1);
assert.equal(await first.prepareDiscoveryLogo(media),'https://image.tmdb.org/t/p/w500/logo.png');
assert.equal(calls,1);
const reboot=await load(2);
assert.equal(await reboot.prepareDiscoveryLogo(media),'https://image.tmdb.org/t/p/w500/logo.png');
assert.equal(calls,1,'reopening uses the stored URL without another metadata request');
failDecode=true;
const failed=await load(3);
assert.equal(await failed.prepareDiscoveryLogo(media),null);
assert.equal(JSON.parse(stored.get('luma.logo-urls.v1'))[media.id],undefined,'unavailable URL is evicted');
failDecode=false;
assert.equal(await failed.prepareDiscoveryLogo(media),'https://image.tmdb.org/t/p/w500/logo.png');
assert.equal(calls,2,'failed image permits a fresh metadata lookup');
stored.set('luma.logo-urls.v1',JSON.stringify({bad:null,invalid:{time:3,url:'file:///secret'}}));
for(let id=2;id<42;id++)await first.readDiscoveryLogo({...media,id:`tmdb-series-${id}`,tmdbId:id});
const cache=JSON.parse(stored.get('luma.logo-urls.v1'));
assert.equal(Object.keys(cache).length,32);
assert.equal(cache.bad,undefined);
assert.equal(cache.invalid,undefined);
console.log('Artwork cache: reopen, failed-image recovery, invalid data and 32-entry bound passed.');
