import { Window } from 'happy-dom';
import { compile } from 'svelte/compiler';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import assert from 'node:assert/strict';

const window = new Window({ url: 'http://localhost/library?mobile=1', width: 412, height: 820 });
for (const key of ['window','document','navigator','HTMLElement','HTMLMediaElement','HTMLInputElement','HTMLDialogElement','Element','Node','Text','Comment','Event','MouseEvent','PointerEvent','sessionStorage']) {
	Object.defineProperty(globalThis, key, { value: window[key], configurable: true });
}
globalThis.requestAnimationFrame = window.requestAnimationFrame.bind(window);
window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new Event('close')); };
window.HTMLElement.prototype.getBoundingClientRect = function () {
	return this.classList.contains('catalog-card__menu')
		? { left: 0, right: 220, top: 0, bottom: 100, width: 220, height: 100 }
		: { left: 320, right: 400, top: 40, bottom: 72, width: 80, height: 32 };
};
const directory = new URL('../.artifacts/library-menu-test/', import.meta.url);
await mkdir(directory, { recursive: true });
const icon = compile('<script>let {name}=$props();</script><span data-icon={name}></span>', { generate:'client' });
await writeFile(new URL('Icon.mjs', directory), icon.js.code);
const favorite = compile('<button aria-label="Favorite">♡</button>', { generate:'client' });
await writeFile(new URL('Favorite.mjs', directory), favorite.js.code);
const source = await readFile(new URL('../src/lib/components/LocalCatalogGrid.svelte', import.meta.url), 'utf8');
const { mount, unmount, flushSync } = await import('svelte');
const settle = async () => { flushSync(); await Promise.resolve(); flushSync(); await Promise.resolve(); flushSync(); };
const item = id => ({ id, title:'Example', kind:'movie', year:2025, extension:'mkv', posterUrl:null });
const pointer = (node, type, values={}) => node.dispatchEvent(new window.PointerEvent(type, { bubbles:true, cancelable:true, pointerType:'touch', clientX:350, clientY:60, ...values }));

for (const mobile of [false, true]) {
	let fixture = source.replace("'$lib/components/Icon.svelte'", "'./Icon.mjs'")
		.replace("'./FavoriteButton.svelte'", "'./Favorite.mjs'")
		.replace(/import \{ cardMedia \}[^;]+;/, 'const cardMedia = item => item;')
		.replace(/import \{ recoverRemoteArtwork, tmdbImageSize \}[^;]+;/, 'const recoverRemoteArtwork=()=>{}; const tmdbImageSize=url=>url;')
		.replace(/import \{ isMobilePreview \}[^;]+;/, `const isMobilePreview=()=>${mobile};`)
		.replace(/import \{ invalidateCatalogPageCache[^;]+;/, 'const invalidateCatalogPageCache=()=>{}; const isDesktopRuntime=()=>true; const moveLibraryTitle=()=>Promise.resolve(); const permanentlyDeleteLibraryTitle=()=>Promise.resolve(); const readLibraryStatus=()=>Promise.resolve({folders:["D:/Movies"]});')
		.replace(/import \{ nativeMobile[^;]+;/, `const nativeMobile=${mobile}; const readComputerLibraryStatus=()=>Promise.resolve({folders:["D:/Movies"]});`)
		.replace(/import \{ invoke \}[^;]+;/, 'const invoke=()=>Promise.resolve();')
		.replace(/import \{ usePlayer \}[^;]+;/, 'const usePlayer=()=>({open:()=>{}});');
	const result = compile(fixture, { generate:'client', filename:`Catalog-${mobile}.svelte` });
	const moduleUrl = new URL(`Catalog-${mobile}.mjs`, directory);
	await writeFile(moduleUrl, result.js.code);
	const { default: Grid } = await import(moduleUrl);
	const instance = mount(Grid, { target:document.body, props:{items:[item(42),item(1_000_000_001)]} });
	await settle();
	const card = document.querySelector('.catalog-card');
	if (!mobile) {
		document.querySelector('.catalog-card__actions').click(); await settle();
	} else {
		assert.equal(document.querySelector('.catalog-card__actions'), null, 'phone uses long press rather than a dots button');
		pointer(card, 'pointerdown');
		await new Promise(resolve => setTimeout(resolve, 1100)); await settle();
		pointer(card, 'pointerup');
		const click = new MouseEvent('click', { bubbles:true, cancelable:true });
		card.querySelector('a').dispatchEvent(click);
		assert.ok(click.defaultPrevented, 'holding longer than a second must never navigate on release');
	}
	await settle();
	const menu = document.querySelector('[role="menu"]');
	assert.ok(menu, 'menu opens on the first interaction');
	assert.equal(menu.parentElement, document.body, 'menu is outside transformed and clipped cards');
	assert.equal(menu.style.visibility, 'visible');
	assert.ok(parseFloat(menu.style.left) + 220 <= 400, 'menu fits the phone viewport');
	assert.equal(menu.querySelectorAll('button').length, 2, 'folder list is initially collapsed');
	menu.querySelector('.catalog-card__menu-delete').click(); await settle();
	assert.ok(document.querySelector('dialog')?.open, 'confirmation waits for the dialog to be mounted');
	document.querySelector('dialog').close(); await settle();
	if (mobile) {
		pointer(card, 'pointerdown'); await new Promise(resolve => setTimeout(resolve, 390)); await settle(); pointer(card,'pointerup');
		window.dispatchEvent(new Event('scroll')); await settle();
		assert.equal(document.querySelector('[role="menu"]'), null, 'scroll dismisses the menu');
		assert.equal(document.querySelector('.catalog-card--selected'), null, 'selection does not remain stuck');
	}
	await unmount(instance); await settle();
	assert.equal(document.querySelector('[role="menu"]'), null, 'portal is cleaned up on navigation');
}
console.log('Library menus passed desktop click, extended mobile hold, viewport, confirmation and dismissal checks.');
