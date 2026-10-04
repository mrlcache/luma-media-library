import { Window } from 'happy-dom';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const window = new Window({url:'http://tauri.localhost/',settings:{disableCSSFileLoading:true,disableJavaScriptFileLoading:true,disableIframePageLoading:true}});
const real = process.argv.includes('--real');
for (const key of ['window','document','navigator','location','history','localStorage','sessionStorage','Element','HTMLElement','HTMLMediaElement','HTMLVideoElement','Node','Text','Event','CustomEvent','MutationObserver','ResizeObserver','IntersectionObserver','getComputedStyle','getSelection','requestAnimationFrame','cancelAnimationFrame','pageXOffset','pageYOffset','innerWidth','innerHeight','scrollTo']) {
  if (key in window) Object.defineProperty(globalThis,key,{value:typeof window[key] === 'function' && /^(getComputedStyle|getSelection|scrollTo|request|cancel)/.test(key)?window[key].bind(window):window[key],configurable:true});
}
window.__TAURI_INTERNALS__ = {metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}}};
// Native Tauri defines these as read-only. A plain browser mock hides this constraint.
for (const key of ['invoke','transformCallback','unregisterCallback','runCallback','callbacks']) {
  const invoke = (command,args) => {
    if (real && command==='mobile_remote_command') {
      if(args.command==='desktop_bootstrap')return Promise.resolve({version:'0.2.0',platform:'android',mediaCoreStatus:'library-index-ready',nativeWindowFrame:true});
      return Promise.reject('Connect to your computer in Settings.');
    }
    if(real && command==='mobile_local_command')return Promise.resolve(args.command==='get_catalog_page'?{items:[],total:0}:[]);
    return Promise.resolve(null);
  };
  Object.defineProperty(window.__TAURI_INTERNALS__,key,{value:key==='callbacks'?new Map():key==='invoke'?invoke:()=>Promise.resolve(null)});
}
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {};
Object.defineProperty(window.__TAURI_EVENT_PLUGIN_INTERNALS__,'unregisterListener',{value:()=>{}});
const nativeBridge = window.__TAURI_INTERNALS__;
const nativeInvoke = nativeBridge.invoke;
for (const key of ['addEventListener','removeEventListener','dispatchEvent','matchMedia']) globalThis[key]=window[key].bind(window);
for (const key of Object.getOwnPropertyNames(window)) {
  if (/^[A-Z]/.test(key) && !(key in globalThis)) Object.defineProperty(globalThis,key,{value:window[key],configurable:true});
}
globalThis.SVGAElement ??= class extends window.SVGElement {};
const headAppend = document.head.appendChild.bind(document.head);
document.head.appendChild = node => { const result=headAppend(node); if(node.tagName==='LINK')setTimeout(()=>node.dispatchEvent(new Event('load')),0);return result; };
const html = fs.readFileSync('.artifacts/mobile-web/index.html','utf8');
const identifier = /(__sveltekit_\w+)\s*=/.exec(html)[1];
globalThis[identifier] = {base:''};
const entry=path.resolve('.artifacts/mobile-web/_app/immutable/entry');
const locate = prefix=>pathToFileURL(path.join(entry,fs.readdirSync(entry).find(f=>f.startsWith(prefix)))).href;
const kit = await import(locate('start.'));
const app = await import(locate('app.'));
const errors=[];
const originalError=console.error;
console.error=(...args)=>{errors.push(args.map(String).join(' '));originalError(...args);};
await kit.start(app,document.body);
await new Promise(resolve=>setTimeout(resolve,1500));
if (document.body.textContent.includes('Internal Error') || errors.length) { console.log(errors); process.exit(1); }
if (!real && (!document.body.textContent.includes('For you') || !document.body.textContent.includes('Continue watching'))) throw new Error('Home did not render');
if (document.documentElement.dataset.mobilePreview !== 'true') throw new Error('Mobile shell is not enabled');
if (real && (!document.body.textContent.includes('Library') || !document.body.textContent.includes('Home'))) throw new Error('Unpaired mobile navigation did not render');
if (window.__TAURI_INTERNALS__ !== nativeBridge || nativeBridge.invoke !== nativeInvoke) throw new Error('Demo replaced the native bridge');
console.log(real?'Packaged mobile rendered without a paired PC.':'Packaged mobile Home rendered successfully.');
process.exit(0);

