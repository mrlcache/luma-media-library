<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import FavoriteButton from './FavoriteButton.svelte';
	import { onDestroy, tick } from 'svelte';
	import { cardMedia } from '$lib/media/favorites';
	import { recoverRemoteArtwork, tmdbImageSize } from '$lib/media/artwork';
	import { isMobilePreview } from '$lib/platform/mobile-preview';
	import { invalidateCatalogPageCache, isDesktopRuntime, moveLibraryTitle, permanentlyDeleteLibraryTitle, readLibraryStatus } from '$lib/platform/desktop';
	import { nativeMobile, readComputerLibraryStatus } from '$lib/platform/mobile-connection';
	import { invoke } from '$lib/platform/invoke';
	import { usePlayer } from '$lib/player-context';
	import type { LibraryCard } from '$lib/torrents/library';

	type Props = { items: LibraryCard[] };
	let { items }: Props = $props();
	const mobilePreview = isMobilePreview();
	const player = usePlayer();
	let deleteDialog = $state<HTMLDialogElement>();
	let actionItem = $state<LibraryCard | null>(null);
	let deleteItem = $state<LibraryCard | null>(null);
	let libraryFolders = $state<string[]>([]);
	let foldersLoading = $state(false);
	let actionBusy = $state(false);
	let actionError = $state('');
	let openMenuId = $state<number | null>(null);
	let menuElement = $state<HTMLDivElement>();
	let menuView = $state<'actions' | 'folders'>('actions');
	let menuAnchor: DOMRect | undefined;
	let selectedPhoneItemId = $state<number | null>(null);
	let phoneLongPressTimer: number | undefined;
	let phonePressOrigin: { x: number; y: number } | null = null;
	let phoneLongPressTriggered = false;
	let suppressedItemId: number | null = null;
	let folderRequestId = 0;
	const canManageFiles = isDesktopRuntime();
	const isPhoneItem = (item: LibraryCard | null) => nativeMobile && typeof item?.id === 'number' && item.id >= 1_000_000_000;

	function portal(node: HTMLElement) {
		document.body.appendChild(node);
		return { destroy: () => node.remove() };
	}

	function closeMenu() {
		openMenuId = null;
		selectedPhoneItemId = null;
		folderRequestId++;
		if (mobilePreview && document.activeElement instanceof HTMLElement && document.activeElement.closest('.catalog-card')) document.activeElement.blur();
	}

	async function positionMenu() {
		await tick();
		if (!menuElement || !menuAnchor || openMenuId === null) return;
		const bounds = menuElement.getBoundingClientRect();
		const below = (mobilePreview ? menuAnchor.top + 42 : menuAnchor.bottom) + 6;
		menuElement.style.left = `${Math.max(12, Math.min(mobilePreview ? menuAnchor.left : menuAnchor.right - bounds.width, window.innerWidth - bounds.width - 12))}px`;
		menuElement.style.top = `${Math.max(12, Math.min(below + bounds.height <= window.innerHeight - 12 ? below : menuAnchor.top - bounds.height - 6, window.innerHeight - bounds.height - 12))}px`;
		menuElement.style.visibility = 'visible';
	}

	$effect(() => {
		if (openMenuId === null) return;
		const scroll = (event: Event) => {
			if (!(event.target instanceof Node) || !menuElement?.contains(event.target)) closeMenu();
		};
		window.addEventListener('scroll', scroll, true);
		window.addEventListener('resize', closeMenu);
		window.addEventListener('blur', closeMenu);
		return () => {
			window.removeEventListener('scroll', scroll, true);
			window.removeEventListener('resize', closeMenu);
			window.removeEventListener('blur', closeMenu);
		};
	});
	onDestroy(cancelPhoneLongPress);

	function closeActionMenu(event: PointerEvent) {
		const target = event.target;
		if (target instanceof Element && (target.closest('.catalog-card__actions') || menuElement?.contains(target))) return;
		closeMenu();
	}

	function handlePhoneMenuKey(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			closeMenu();
		}
	}

	function startPhoneLongPress(event: PointerEvent, item: LibraryCard) {
		if (!mobilePreview || !canManageFiles || typeof item.id !== 'number' || item.transfer) return;
		if (event.target instanceof Element && event.target.closest('button, .catalog-card__favorite, .catalog-card__actions, .catalog-card__menu')) return;
		clearTimeout(phoneLongPressTimer);
		suppressedItemId = null;
		phoneLongPressTriggered = false;
		phonePressOrigin = { x: event.clientX, y: event.clientY };
		const card = event.currentTarget as HTMLElement;
		const anchor = card.querySelector<HTMLElement>('.catalog-card__poster') ?? card;
		phoneLongPressTimer = window.setTimeout(() => {
			if (!anchor?.isConnected) return;
			void showActions(item, anchor);
			phoneLongPressTriggered = true;
			suppressedItemId = item.id as number;
			phoneLongPressTimer = undefined;
		}, 360);
	}

	function movePhoneLongPress(event: PointerEvent) {
		if (!phonePressOrigin) return;
		if (Math.hypot(event.clientX - phonePressOrigin.x, event.clientY - phonePressOrigin.y) > 18) {
			cancelPhoneLongPress();
			closeMenu();
		}
	}

	function cancelPhoneLongPress() {
		clearTimeout(phoneLongPressTimer);
		phoneLongPressTimer = undefined;
		phonePressOrigin = null;
	}

	function endPhoneLongPress(event: PointerEvent) {
		if (phoneLongPressTriggered && event.cancelable) event.preventDefault();
		cancelPhoneLongPress();
		phoneLongPressTriggered = false;
	}

	function cancelPhonePointer() {
		cancelPhoneLongPress();
		if (phoneLongPressTriggered) closeMenu();
		phoneLongPressTriggered = false;
	}

	function handleCardClick(event: MouseEvent) {
		if (suppressedItemId === null || !(event.target instanceof Element) || !event.target.closest('.catalog-card__poster-link, .catalog-card__title')) return;
		event.preventDefault();
		event.stopPropagation();
		suppressedItemId = null;
	}

	function folderName(path: string) {
		return path.replace(/[\\/]+$/, '').split(/[\\/]/).filter(Boolean).at(-1) || path;
	}

	async function showActions(item: LibraryCard, anchor: HTMLElement) {
		if (!canManageFiles || actionBusy || typeof item.id !== 'number' || item.transfer) return;
		if (openMenuId === item.id) {
			closeMenu();
			return;
		}
		actionItem = item;
		menuAnchor = anchor.getBoundingClientRect();
		menuView = 'actions';
		openMenuId = item.id;
		actionError = '';
		libraryFolders = [];
		selectedPhoneItemId = mobilePreview ? item.id : null;
		await positionMenu();
	}

	async function showFolders() {
		if (!actionItem || isPhoneItem(actionItem)) return;
		menuView = 'folders';
		foldersLoading = true;
		const requestId = ++folderRequestId;
		void (nativeMobile ? readComputerLibraryStatus<{folders: string[]}>() : readLibraryStatus()).then((status) => {
			if (requestId === folderRequestId && openMenuId !== null) libraryFolders = status?.folders ?? [];
		}).catch((error) => {
			if (requestId === folderRequestId && openMenuId !== null) actionError = error instanceof Error ? error.message : 'Could not load library folders.';
		}).finally(() => {
			if (requestId === folderRequestId) { foldersLoading = false; void positionMenu(); }
		});
		await positionMenu();
	}

	async function moveTo(folder: string) {
		if (!actionItem || typeof actionItem.id !== 'number' || actionBusy) return;
		actionBusy = true;
		actionError = '';
		try {
			await moveLibraryTitle(actionItem.id, folder, actionItem.playbackUuid);
			invalidateCatalogPageCache();
			window.dispatchEvent(new Event('luma-library-changed'));
			closeMenu();
			actionItem = null;
		} catch (error) { actionError = error instanceof Error ? error.message : 'Could not move this title.'; }
		finally { actionBusy = false; }
	}

	async function askDelete() {
		if (!actionItem) return;
		deleteItem = actionItem;
		closeMenu();
		actionError = '';
		await tick();
		deleteDialog?.showModal();
	}

	async function deletePermanently() {
		if (!deleteItem || typeof deleteItem.id !== 'number' || actionBusy) return;
		actionBusy = true;
		actionError = '';
		try {
			if (isPhoneItem(deleteItem)) {
				await invoke<void>('delete_local_media', { mediaId: deleteItem.id });
			} else {
				await permanentlyDeleteLibraryTitle(deleteItem.id, deleteItem.playbackUuid);
			}
			invalidateCatalogPageCache();
			window.dispatchEvent(new Event('luma-library-changed'));
			window.dispatchEvent(new Event('library-changed'));
			deleteDialog?.close();
		} catch (error) { actionError = error instanceof Error ? error.message : 'Could not delete this title.'; }
		finally { actionBusy = false; }
	}

	function play(item: LibraryCard) {
		player.open({
			id: String(item.id),
			playbackUuid: item.playbackUuid,
			title: item.title,
			kind: item.kind === 'series' ? 'series' : 'movie',
			year: item.year ?? 0,
			genres: [],
			rating: item.voteAverage?.toFixed(1) ?? '',
			runtime: '',
			poster: tmdbImageSize(item.posterUrl, 'w780'),
			backdrop: tmdbImageSize(item.backdropUrl, 'w1280'),
			synopsis: item.overview ?? '',
			match: ''
		});
	}
</script>

<svelte:window onpointerdown={closeActionMenu} onkeydown={handlePhoneMenuKey} />
<div class="catalog-grid" role="list" aria-label="Local media results">
	{#each items as item, index (item.id)}
		<article class="catalog-card" class:catalog-card--mobile={mobilePreview} class:catalog-card--selected={mobilePreview && selectedPhoneItemId === item.id} class:catalog-card--menu-open={openMenuId === item.id} data-item-id={typeof item.id === 'number' ? item.id : undefined} role="listitem" aria-setsize={items.length} aria-posinset={index + 1} onpointerdown={(event) => startPhoneLongPress(event, item)} onpointermove={movePhoneLongPress} onpointerup={endPhoneLongPress} onpointercancel={cancelPhonePointer} oncontextmenu={mobilePreview ? (event) => event.preventDefault() : undefined}>
			<div class="catalog-card__poster">
				<a class="catalog-card__poster-link" href={`/title/${item.id}`} aria-label={`Details for ${item.title}`} onclick={handleCardClick}>
					{#if item.posterUrl}
						<img use:recoverRemoteArtwork src={tmdbImageSize(item.posterUrl, mobilePreview ? 'w500' : 'w780')} alt={`Poster for ${item.title}`} width="780" height="1170" loading={index < (mobilePreview ? 4 : 8) ? 'eager' : 'lazy'} decoding="async" />
					{:else}
						<span class="catalog-card__placeholder" aria-hidden="true"><Icon name={item.kind === 'series' ? 'tv' : 'film'} size={25} /></span>
					{/if}
				</a>
				<div class="catalog-card__favorite"><FavoriteButton media={cardMedia(item)} compact /></div>
				{#if canManageFiles && !mobilePreview && typeof item.id === 'number' && !item.transfer}
					<button class="catalog-card__actions" class:catalog-card__actions--open={openMenuId === item.id} type="button" aria-label={`More options for ${item.title}`} aria-haspopup="menu" aria-expanded={openMenuId === item.id} onclick={(event) => { event.stopPropagation(); void showActions(item, event.currentTarget); }}><Icon name="more" size={18} /></button>
				{/if}
				{#if typeof item.id !== 'number' && !item.transfer}<span class="catalog-card__availability"  role="img" aria-label="Not downloaded"><Icon name="download" size={13} /></span>{/if}
				{#if item.transfer && item.transfer.progress < 1}
					<div class="download-overlay">
						<span><strong>{Math.floor(item.transfer.progress * 100)}%</strong></span>
						<div class="download-progress" role="progressbar" aria-label={`Download progress for ${item.title}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.floor(item.transfer.progress * 100)}><i style={`width: ${item.transfer.progress * 100}%`}></i></div>
					</div>
				{:else if typeof item.id === 'number'}
					<button class="catalog-card__play" type="button" aria-label={`Play ${item.title}`}  onclick={() => play(item)}>
						<Icon name="play" size={16} weight="fill" />
					</button>
				{/if}
			</div>
			<div class="catalog-card__copy">
				<a class="catalog-card__title" href={`/title/${item.id}`} onclick={handleCardClick}>{item.title}</a>
				<span>
					{#if item.year}{item.year}{:else if item.kind}{item.kind === 'series' ? 'Series' : 'Movie'}{:else}{item.extension.toUpperCase()} file{/if}
					{#if item.year && item.kind}<i aria-hidden="true">·</i> {item.kind === 'series' ? 'Series' : 'Movie'}{/if}
					{#if item.voteAverage}<i aria-hidden="true">·</i> {item.voteAverage.toFixed(1)}{/if}
				</span>
			</div>
		</article>
	{/each}
</div>

{#if openMenuId !== null && actionItem}
	<div bind:this={menuElement} use:portal class="catalog-card__menu" class:catalog-card__menu--mobile={mobilePreview} role="menu" aria-label={`Actions for ${actionItem.title}`} tabindex="-1" data-lenis-prevent>
		{#if menuView === 'actions'}
			{#if !isPhoneItem(actionItem)}<button type="button" role="menuitem" disabled={actionBusy} onclick={showFolders}><Icon name="folder" size={16} /><span>Move to folder</span><Icon name="chevron-right" size={13} /></button>{/if}
			<button class="catalog-card__menu-delete" type="button" role="menuitem" disabled={actionBusy} onclick={askDelete}><Icon name="trash" size={16} /><span>Delete</span></button>
		{:else}
			<button class="catalog-card__menu-back" type="button" role="menuitem" onclick={() => { menuView = 'actions'; actionError = ''; void positionMenu(); }}><Icon name="arrow-left" size={14} /><span>Move to folder</span></button>
			{#if foldersLoading}<p class="catalog-card__menu-status">Loading folders…</p>
			{:else if libraryFolders.length}
				{#each libraryFolders as folder (folder)}<button class="catalog-card__menu-folder" type="button" role="menuitem" disabled={actionBusy} onclick={() => void moveTo(folder)}><Icon name="folder" size={15} /><span>{folderName(folder)}</span></button>{/each}
			{:else}<p class="catalog-card__menu-status">No folders available.</p>{/if}
		{/if}
		{#if actionError}<p class="catalog-card__menu-status catalog-card__menu-error" role="alert">{actionError}</p>{/if}
	</div>
{/if}

{#if deleteItem}
	<dialog bind:this={deleteDialog} class="library-actions library-actions--confirm" aria-labelledby="delete-title" onclose={() => deleteItem = null} onclick={(event) => { if (event.target === deleteDialog) deleteDialog?.close(); }}>
		<div class="confirm-icon"><Icon name="close" size={18} /></div>
		<h2 id="delete-title">Delete {deleteItem.title}?</h2>
		<p>{isPhoneItem(deleteItem) ? 'This removes the downloaded file from this phone. It does not delete anything from your computer.' : 'This permanently deletes its video files from the computer, including every episode and Extras. It will not go to the Recycle Bin.'}</p>
		{#if actionError}<p class="dialog-error" role="alert">{actionError}</p>{/if}
		<div class="confirm-actions"><button class="cancel-action" type="button" disabled={actionBusy} onclick={() => deleteDialog?.close()}>Cancel</button><button class="confirm-delete" type="button" disabled={actionBusy} onclick={() => void deletePermanently()}>{actionBusy ? 'Deleting…' : isPhoneItem(deleteItem) ? 'Delete from phone' : 'Delete permanently'}</button></div>
	</dialog>
{/if}

<style>
	.catalog-card__favorite { position:absolute; z-index:3; left:9px; top:9px; opacity:0; transition:opacity 140ms ease; }
	.catalog-card:hover .catalog-card__favorite, .catalog-card:focus-within .catalog-card__favorite, .catalog-card__favorite:has(:global([aria-pressed='true'])) { opacity:1; }
	.catalog-card__actions { position:absolute; z-index:3; right:9px; top:9px; display:grid; width:32px; height:32px; place-items:center; border:1px solid rgba(255,255,255,.17); border-radius:50%; color:var(--text-soft); background:rgba(8,12,17,.78); backdrop-filter:blur(12px); cursor:pointer; opacity:0; transition:opacity 140ms ease, background 140ms ease; }
	.catalog-card:hover .catalog-card__actions, .catalog-card:focus-within .catalog-card__actions { opacity:1; }
	.catalog-card__actions:hover { background:rgba(25,34,42,.96); }
	.catalog-card__menu { position:fixed; z-index:1000; top:0; left:0; visibility:hidden; box-sizing:border-box; width:min(220px,calc(100vw - 24px)); max-height:min(300px,calc(100dvh - 24px)); overflow:auto; padding:5px; border:1px solid var(--line-strong); border-radius:9px; background:#151d24; color:var(--text-soft); box-shadow:0 12px 32px rgba(0,0,0,.35); animation:catalog-menu-in 100ms ease-out 1; }
	.catalog-card__menu button { display:flex; align-items:center; gap:10px; width:100%; min-height:36px; padding:8px 10px; border:0; border-radius:5px; color:var(--text-soft); background:transparent; font:inherit; font-size:.72rem; text-align:left; cursor:pointer; }
	.catalog-card__menu button span { flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
	.catalog-card__menu button :global(svg) { flex-shrink:0; }
	.catalog-card__menu button:hover, .catalog-card__menu button:focus-visible { background:rgba(255,255,255,.08); }
	.catalog-card__menu .catalog-card__menu-delete { color:#efaaa8; }
	.catalog-card__menu .catalog-card__menu-back { margin-bottom:4px; border-bottom:1px solid var(--line-subtle); border-radius:0; color:var(--text-muted); }
	.catalog-card__menu--mobile button { min-height:44px; font-size:.78rem; }
	.catalog-card__menu-status { margin:5px 8px; color:var(--text-muted); font-size:.64rem; line-height:1.4; }
	.catalog-card__menu-error { color:#ff9d9d; overflow-wrap:anywhere; }
	.catalog-card__menu button:disabled { opacity:.55; cursor:default; }
	@keyframes catalog-menu-in { from { opacity:0; } to { opacity:1; } }
	.catalog-card__availability { position:absolute; left:9px; top:9px; display:grid; place-items:center; width:24px; height:24px; border:1px solid rgba(255,255,255,.14); border-radius:50%; color:rgba(255,255,255,.85); background:rgba(8,12,17,.65); }
	.download-overlay { position: absolute; inset: auto 0 0; padding: 28px 12px 14px; background: linear-gradient(transparent, rgba(5,9,13,.94)); pointer-events: none; }
	.download-overlay > span { display: flex; align-items: center; justify-content: flex-end; gap: 6px; margin-bottom: 8px; color: var(--text-soft); font-size: .65rem; }
	.download-overlay strong { font-variant-numeric: tabular-nums; font-weight: 600; }
	.download-progress { height: 3px; overflow: hidden; border-radius: 999px; background: rgba(210,229,237,.18); }
	.download-progress i { display: block; height: 100%; border-radius: inherit; background: var(--accent); transition: width 1s linear; }
	.catalog-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 146px), 1fr)); gap: 18px; margin: -14px; padding: 14px 22px 36px 15px; }
	.catalog-card { position:relative; min-width: 0; transform-origin: center bottom; transition: transform 190ms cubic-bezier(0.2, 0.72, 0.2, 1); }
	.catalog-card--menu-open { z-index:10; }
	.catalog-card__poster { position: relative; overflow: hidden; aspect-ratio: 2 / 3; border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; background: var(--surface-2); box-shadow: 0 12px 28px rgba(0,0,0,0.22); transition: border-color 180ms ease, box-shadow 180ms ease; }
	.catalog-card__poster-link { position: absolute; inset: 0; display: block; overflow: hidden; border-radius: inherit; color: inherit; text-decoration: none; }
	.catalog-card__poster img { display: block; width: 100%; height: 100%; object-fit: cover; transition: transform 260ms ease, filter 260ms ease; }
	.catalog-card__placeholder { display: grid; place-items: center; width: 100%; height: 100%; color: var(--text-dim); background: linear-gradient(145deg, var(--surface-2), var(--surface-1)); }
	.catalog-card__play { position: absolute; z-index: 2; right: 11px; bottom: 11px; display: grid; width: 36px; height: 36px; place-items: center; border: 1px solid rgba(255,255,255,0.22); border-radius: 50%; color: #101317; background: rgba(245,247,249,0.94); box-shadow: 0 8px 24px rgba(0,0,0,0.3); cursor: pointer; opacity: 0; transform: translateY(4px); transition: opacity 160ms ease, transform 160ms ease; }
	.catalog-card:hover .catalog-card__play, .catalog-card:focus-within .catalog-card__play { opacity: 1; transform: translateY(0); }
	.catalog-card:hover, .catalog-card:focus-within { transform: translateY(-3px) scale(1.018); }
	.catalog-card:hover .catalog-card__poster, .catalog-card:focus-within .catalog-card__poster { border-color: rgba(255,255,255,0.34); box-shadow: 0 0 0 2px rgba(255,255,255,0.34), 0 18px 36px rgba(0,0,0,0.34); }
	.catalog-card:hover .catalog-card__poster img, .catalog-card:focus-within .catalog-card__poster img { filter: brightness(1.05); transform: scale(1.012); }
	.catalog-card__play:hover { background: #fff; }
	.catalog-card__copy { display: grid; gap: 3px; padding: 10px 2px 0; }
	.catalog-card__title { overflow: hidden; color: var(--text-soft); font-size: 0.81rem; font-weight: 610; text-decoration: none; text-overflow: ellipsis; white-space: nowrap; }
	.catalog-card__title:hover { color: var(--text-strong); }
	.catalog-card__copy span { overflow: hidden; color: var(--text-muted); font-size: 0.69rem; text-overflow: ellipsis; white-space: nowrap; }
	.catalog-card__copy i { padding: 0 3px; color: var(--text-dim); font-style: normal; }
	.catalog-card__actions--open { opacity:1; }
	.catalog-card--mobile, .catalog-card--mobile:hover, .catalog-card--mobile:focus-within { transform:none; }
	.catalog-card--mobile .catalog-card__favorite { right:8px; left:auto; top:8px; opacity:1; }
	.catalog-card--mobile .catalog-card__play { display:none; }
	.catalog-card--mobile .catalog-card__poster, .catalog-card--mobile:hover .catalog-card__poster, .catalog-card--mobile:focus-within .catalog-card__poster { border-color:rgba(255,255,255,.08); box-shadow:0 12px 28px rgba(0,0,0,.22); transition:border-color 90ms ease; }
	.catalog-card--mobile .catalog-card__poster img, .catalog-card--mobile:hover .catalog-card__poster img, .catalog-card--mobile:focus-within .catalog-card__poster img { transform:none; filter:none; transition:none; }
	.catalog-card--mobile .catalog-card__poster-link { -webkit-touch-callout:none; touch-action:pan-y; }
	.catalog-card--mobile.catalog-card--selected .catalog-card__poster { border-color:rgba(158,198,214,.7); }
	.library-actions { width:min(430px,calc(100vw - 32px)); max-height:min(80dvh,640px); overflow:auto; box-sizing:border-box; padding:22px; border:1px solid var(--line-strong); border-radius:18px; color:var(--text-soft); background:#12191f; box-shadow:0 24px 80px rgba(0,0,0,.48); }
	.library-actions::backdrop { background:rgba(0,0,0,.58); backdrop-filter:blur(3px); }
	.library-actions h2 { margin:4px 0 0; color:var(--text-strong); font-size:1.05rem; }
	.dialog-error { margin:12px 0 0; color:#ff9d9d; font-size:.69rem; line-height:1.5; overflow-wrap:anywhere; }
	.library-actions--confirm { width:min(390px,calc(100vw - 32px)); padding:24px; }
	.library-actions--confirm h2 { margin:12px 0 8px; }
	.library-actions--confirm > p:not(.dialog-error) { margin:0; color:var(--text-muted); font-size:.73rem; line-height:1.6; }
	.confirm-icon { display:grid; width:38px; height:38px; place-items:center; border-radius:12px; color:#ff9d9d; background:rgba(211,71,71,.13); }
	.confirm-actions { display:flex; justify-content:flex-end; gap:9px; margin-top:22px; }
	.cancel-action, .confirm-delete { min-height:37px; padding:0 13px; border:1px solid var(--line-subtle); border-radius:9px; color:var(--text-soft); background:transparent; font:inherit; font-size:.7rem; cursor:pointer; }
	.confirm-delete { border-color:#a94343; color:white; background:#9f3939; }
	.confirm-delete:hover { background:#b64242; }
	.cancel-action:hover { background:rgba(255,255,255,.06); }
	@media (max-width: 760px) { .catalog-grid { gap: 15px 14px; padding-right: 17px; } }
	@media (hover: none) { .catalog-card__play { opacity: 1; transform: none; } }
	@media (prefers-reduced-motion: reduce) { .catalog-card, .catalog-card__poster, .catalog-card__poster img, .catalog-card__play { transition: none; } .catalog-card:hover, .catalog-card:focus-within { transform: none; } .catalog-card__menu { animation:none; } }
</style>
