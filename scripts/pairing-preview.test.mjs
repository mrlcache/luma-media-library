import {Window} from 'happy-dom';
import {compile} from 'svelte/compiler';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import assert from 'node:assert/strict';

const window=new Window({url:'http://localhost/pairing-preview?mobile=1'});
for(const key of ['window','document','navigator','HTMLElement','Element','Node','Text','Comment','Event','MouseEvent','sessionStorage'])Object.defineProperty(globalThis,key,{value:window[key],configurable:true});
globalThis.requestAnimationFrame=window.requestAnimationFrame.bind(window);
let destination='',ipcCalls=0;
globalThis.__previewGoto=url=>{destination=url;};
globalThis.__previewInvoke=()=>{ipcCalls++;throw new Error('Web preview must not call native IPC');};
const dir=new URL('../.artifacts/pairing-test/',import.meta.url);await mkdir(dir,{recursive:true});
const icon=compile('<script>let {name}=$props();</script><span data-icon={name}></span>',{generate:'client'});
await writeFile(new URL('Icon.mjs',dir),icon.js.code);
let source=await readFile(new URL('../src/lib/components/MobilePairing.svelte',import.meta.url),'utf8');
source=source.replace("import {goto} from '$app/navigation';",'const goto=globalThis.__previewGoto;')
 .replace("import {invoke} from '@tauri-apps/api/core';",'const invoke=globalThis.__previewInvoke;')
 .replace("import {nativeMobile,readMobileConnection} from '$lib/platform/mobile-connection';",'const nativeMobile=false;const readMobileConnection=()=>Promise.resolve({paired:false});')
 .replace("import {isDesktopRuntime,invalidateCatalogPageCache} from '$lib/platform/desktop';",'const isDesktopRuntime=()=>false;const invalidateCatalogPageCache=()=>{};')
 .replace("import {resetDiscovery} from '$lib/media/discovery';",'const resetDiscovery=()=>{};')
 .replace("'./Icon.svelte'","'./Icon.mjs'");
const result=compile(source,{generate:'client',filename:'MobilePairing.svelte'});
await writeFile(new URL('MobilePairing.mjs',dir),result.js.code);
const {mount,unmount,flushSync}=await import('svelte');
const {default:Pairing}=await import(new URL('MobilePairing.mjs',dir));
const component=mount(Pairing,{target:document.body,props:{preview:true}});
const settle=async()=>{flushSync();await Promise.resolve();flushSync();};await settle();
const click=async label=>{const button=[...document.querySelectorAll('button')].find(node=>node.textContent.trim()===label);assert.ok(button,`Button ${label}`);button.click();await settle();};
assert.match(document.body.textContent,/Computer address/);
assert.ok(result.css.code.includes('background:#0a1016'),'pairing screen is opaque');
await click('Find computers');await click('Your computer');
assert.match(document.body.textContent,/482193/);
await click('Back');assert.doesNotMatch(document.body.textContent,/482193/);
assert.ok(document.querySelector('input[type="url"]'),'manual address is accessible after returning');
await click('Use this phone only');assert.equal(destination,'/?mobile=1');
assert.equal(ipcCalls,0);
await unmount(component);
// A native request may finish after the user chooses phone-only. It must be cancelled.
let resolveRequest;
let deferPair=true;
const cancellations=[];
globalThis.__previewInvoke=(command,args)=>{
	if(command==='discover_luma_computers')return Promise.resolve([{name:'Your computer',url:'http://192.168.1.10:47631'}]);
	if(command==='request_mobile_pair')return deferPair ? new Promise(resolve=>{resolveRequest=resolve;}) : Promise.resolve({id:'b'.repeat(64),name:'Luma Mobile',code:'234567'});
	if(command==='cancel_mobile_pair'){cancellations.push(args);return Promise.resolve();}
	throw new Error(`Unexpected native command: ${command}`);
};
const nativeSource=source.replace('const nativeMobile=false;','const nativeMobile=true;').replace('Promise.resolve({paired:false})','Promise.resolve({paired:true})').replace('const isDesktopRuntime=()=>false;','const isDesktopRuntime=()=>true;').replace("import.meta.env.VITE_LUMA_MOBILE_DEMO","'false'");
await writeFile(new URL('MobilePairingNative.mjs',dir),compile(nativeSource,{generate:'client',filename:'MobilePairingNative.svelte'}).js.code);
const {default:NativePairing}=await import(new URL('MobilePairingNative.mjs',dir));
const native=mount(NativePairing,{target:document.body});await settle();
window.dispatchEvent(new Event('luma-pair-computer'));await settle();
await click('Your computer');await click('Use this phone only');
resolveRequest({id:'a'.repeat(64),name:'Luma Mobile',code:'123456'});await settle();
assert.equal(cancellations.length,1,'a stale response cancels the desktop pending request');
assert.equal(cancellations[0].id,'a'.repeat(64));
assert.equal(document.querySelector('.pair-screen'),null,'a stale response must not reopen pairing');
deferPair=false;
window.dispatchEvent(new Event('luma-pair-computer'));await settle();await click('Your computer');
assert.match(document.body.textContent,/234567/);
await click('Back');
assert.equal(cancellations.length,2,'Back cancels an active request');
assert.equal(cancellations[1].id,'b'.repeat(64));
assert.ok(document.querySelector('input[type="url"]'));
await unmount(native);
await window.happyDOM.abort();
console.log('Pairing: opaque preview, discovery, code/back, manual address, phone-only navigation and stale-request cancellation passed.');
