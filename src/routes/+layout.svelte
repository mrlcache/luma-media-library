<script lang="ts">
	import { onMount, setContext } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import Icon from '$lib/components/Icon.svelte';
	import PlayerHost from '$lib/components/PlayerHost.svelte';
	import { PLAYER_CONTEXT, PLAYBACK_HISTORY_UPDATED_EVENT } from '$lib/player-context';
	import { isDesktopRuntime, readDesktopBootstrap } from '$lib/platform/desktop';
	import { requestNativeAcrylic } from '$lib/platform/native-acrylic';
	import { applyAppearancePreferences } from '$lib/platform/appearance-preferences';
	import { registerLenis } from '$lib/scroll/lenis';
	import type Lenis from 'lenis';
	import type { IconName } from '$lib/components/Icon.svelte';
	import type { MediaItem } from '$lib/types';
	import '../app.css';

	let { children } = $props();
	let activeMedia = $state<MediaItem | null>(null);
	let desktopRuntime = $state(false);
	let windowMaximized = $state(false);
	let contentArea: HTMLElement;
	let desktopScroll: Lenis | undefined;

	afterNavigate(() => {
		if (desktopScroll) desktopScroll.scrollTo(0, { immediate: true });
		else contentArea?.scrollTo({ top: 0, left: 0, behavior: 'instant' });
	});

	function scrollContentFromChrome(event: WheelEvent) {
		if (!contentArea || event.deltaY === 0) return;
		const unit = event.deltaMode === WheelEvent.DOM_DELTA_LINE
			? 24
			: event.deltaMode === WheelEvent.DOM_DELTA_PAGE
				? contentArea.clientHeight
				: 1;
		const delta = event.deltaY * unit;
		if (desktopScroll) desktopScroll.scrollTo(desktopScroll.targetScroll + delta);
		else contentArea.scrollBy({ top: delta, behavior: 'smooth' });
	}

	function openPlayer(media: MediaItem) {
		activeMedia = media;
	}

	function closePlayer() {
		activeMedia = null;
		window.dispatchEvent(new Event(PLAYBACK_HISTORY_UPDATED_EVENT));
	}

	function handleGlobalKeydown(event: KeyboardEvent) {
		if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'a') return;
		const target = event.target;
		if (target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return;
		event.preventDefault();
	}

	function blockDesktopBrowserShortcuts(event: KeyboardEvent) {
		const key = event.key.toLowerCase();
		const reload = key === 'f5' || (key === 'r' && (event.ctrlKey || event.metaKey));
		const devtools = key === 'f12' || ((event.ctrlKey || event.metaKey) && event.shiftKey && ['i', 'j', 'c'].includes(key));
		if (!reload && !devtools) return;
		event.preventDefault();
		event.stopImmediatePropagation();
	}

	function blockDesktopContextMenu(event: MouseEvent) {
		event.preventDefault();
	}

	function markPointerInput() {
		document.documentElement.dataset.inputModality = 'pointer';
	}

	function markKeyboardInput(event: KeyboardEvent) {
		if (['Alt', 'Control', 'Meta', 'Shift'].includes(event.key)) return;
		document.documentElement.dataset.inputModality = 'keyboard';
	}

	setContext(PLAYER_CONTEXT, { open: openPlayer, close: closePlayer });

	type NavItem = {
		label: string;
		href: string;
		icon: IconName;
		isActive: (url: URL) => boolean;
	};

	const navItems: NavItem[] = [
		{ label: 'Home', href: '/', icon: 'home', isActive: (url) => url.pathname === '/' },
		{
			label: 'Library',
			href: '/library',
			icon: 'library',
			isActive: (url) => url.pathname === '/library' && !url.searchParams.has('type')
		},
		{
			label: 'Movies',
			href: '/library?type=movie',
			icon: 'film',
			isActive: (url) => url.pathname === '/library' && url.searchParams.get('type') === 'movie'
		},
		{
			label: 'Series',
			href: '/library?type=series',
			icon: 'tv',
			isActive: (url) => url.pathname === '/library' && url.searchParams.get('type') === 'series'
		},
		{ label: 'Torrents', href: '/torrents', icon: 'download', isActive: (url) => url.pathname === '/torrents' }
	];

	const searchItem: NavItem = {
		label: 'Search',
		href: '/search',
		icon: 'search',
		isActive: (url) => url.pathname === '/search'
	};

	const settingsItem: NavItem = {
		label: 'Settings',
		href: '/settings',
		icon: 'settings',
		isActive: (url) => url.pathname === '/settings'
	};

	const mobileNavItems = [...navItems, searchItem];

	onMount(() => {
		let disposed = false;
		let unlistenResize: (() => void) | undefined;
		const acrylicRequester = Symbol('Application surface');
		applyAppearancePreferences();
		requestNativeAcrylic(acrylicRequester, true);
		window.addEventListener('pointerdown', markPointerInput, true);
		window.addEventListener('keydown', markKeyboardInput, true);

		if (isDesktopRuntime()) {
			desktopRuntime = true;
			document.documentElement.dataset.runtime = 'desktop';
			window.addEventListener('keydown', blockDesktopBrowserShortcuts, true);
			window.addEventListener('contextmenu', blockDesktopContextMenu, true);
			void import('@tauri-apps/api/window')
				.then(async ({ getCurrentWindow }) => {
					const appWindow = getCurrentWindow();
					windowMaximized = await appWindow.isMaximized();
					const stopListening = await appWindow.onResized(async () => {
						windowMaximized = await appWindow.isMaximized();
					});
					if (disposed) stopListening();
					else unlistenResize = stopListening;
				})
				.catch((error) => console.warn('Window controls unavailable', error));
		}

		void readDesktopBootstrap()
			.then((bootstrap) => {
				if (!bootstrap) return;
				document.documentElement.dataset.runtime = bootstrap.platform;
				document.documentElement.dataset.appVersion = bootstrap.version;
				const nativeFrame = typeof bootstrap.nativeWindowFrame === 'boolean'
					? bootstrap.nativeWindowFrame
					: bootstrap.windowShape === 'css-squircle';
				document.documentElement.dataset.nativeFrame = nativeFrame ? 'dwm' : 'css';
			})
			.catch((error) => console.warn('Desktop bootstrap unavailable', error));

		return () => {
			disposed = true;
			window.removeEventListener('pointerdown', markPointerInput, true);
			window.removeEventListener('keydown', markKeyboardInput, true);
			window.removeEventListener('keydown', blockDesktopBrowserShortcuts, true);
			window.removeEventListener('contextmenu', blockDesktopContextMenu, true);
			unlistenResize?.();
			requestNativeAcrylic(acrylicRequester, false);
		};
	});

	onMount(() => {
		if (!isDesktopRuntime() || !contentArea) return;
		const scroller = contentArea;
		const content = scroller.firstElementChild;
		if (!(content instanceof HTMLElement)) return;
		let disposed = false;
		let releaseScroll: (() => void) | undefined;
		void import('lenis')
			.then(({ default: Lenis }) => {
				if (disposed) return;
				desktopScroll = new Lenis({
					wrapper: scroller,
					content,
					eventsTarget: scroller,
					autoRaf: false,
					smoothWheel: true,
					lerp: 0.13,
					syncTouch: false,
					overscroll: false,
					// Keep horizontal rows native; smooth every vertical wheel gesture consistently.
					virtualScroll: ({ event, deltaX, deltaY }) => {
						if (!(event instanceof WheelEvent) || event.ctrlKey || event.shiftKey) return false;
						return deltaY !== 0 && Math.abs(deltaY) >= Math.abs(deltaX);
					}
				});
				releaseScroll = registerLenis(desktopScroll);
			})
			.catch((error) => console.warn('Smooth scrolling unavailable', error));
		return () => {
			disposed = true;
			releaseScroll?.();
			desktopScroll = undefined;
		};
	});

	async function runWindowAction(action: 'minimize' | 'toggle-maximize' | 'close') {
		if (!isDesktopRuntime()) return;

		try {
			const { getCurrentWindow } = await import('@tauri-apps/api/window');
			const appWindow = getCurrentWindow();

			if (action === 'minimize') await appWindow.minimize();
			if (action === 'toggle-maximize') {
				await appWindow.toggleMaximize();
				windowMaximized = await appWindow.isMaximized();
			}
			if (action === 'close') await appWindow.close();
		} catch (error) {
			console.warn(`Could not ${action} the app window`, error);
		}
	}

</script>

<svelte:head>
	<title>Luma</title>
	<meta name="description" content="Your personal collection, with Luma." />
</svelte:head>

<div class="app-stage">
	<div
		class="app-shell"
		class:window-restored={desktopRuntime && !windowMaximized}
		data-sveltekit-preload-data="hover"
	>
		<aside class="sidebar" aria-label="Application navigation" onwheel={scrollContentFromChrome}>
			<div class="library-context" aria-label="Luma">
				<img class="library-wordmark" src="/luma-wordmark.svg" alt="Luma" />
			</div>

			<a
				class="sidebar-search"
				class:active={searchItem.isActive(page.url)}
				href={searchItem.href}
				style="corner-shape: squircle"
				aria-current={searchItem.isActive(page.url) ? 'page' : undefined}
			>
				<Icon name="search" size={17} weight="bold" />
				<span>Search</span>
				<kbd>⌘ K</kbd>
			</a>

			<nav class="sidebar-nav" aria-label="Primary navigation">
				{#each navItems as item}
					<a
						class="sidebar-link"
						class:active={item.isActive(page.url)}
						href={item.href}
						style="corner-shape: squircle"
						aria-current={item.isActive(page.url) ? 'page' : undefined}
					>
						<Icon name={item.icon} size={19} weight={item.isActive(page.url) ? 'fill' : 'regular'} />
						<span>{item.label}</span>
					</a>
				{/each}
			</nav>

			<div class="sidebar-footer">
				<a
					class="sidebar-link"
					class:active={settingsItem.isActive(page.url)}
					href={settingsItem.href}
					aria-current={settingsItem.isActive(page.url) ? 'page' : undefined}
				>
					<Icon name="settings" size={19} weight={settingsItem.isActive(page.url) ? 'fill' : 'regular'} />
					<span>Settings</span>
				</a>
				<button class="profile-row" type="button" style="corner-shape: squircle" aria-label="Open profile menu">
					<span class="profile-avatar">M</span>
					<span class="profile-copy"><strong>Murilo</strong><small>Profile</small></span>
					<Icon name="more" size={18} weight="bold" />
				</button>
			</div>
		</aside>

		<div class="workspace">
			<main class="content-area" bind:this={contentArea}><div class="route-content">{@render children()}</div></main>
		</div>

		<nav class="mobile-nav" aria-label="Primary navigation">
			{#each mobileNavItems as item}
				<a
					class="mobile-nav__link"
					class:active={item.isActive(page.url)}
					href={item.href}
					aria-label={item.label}
					aria-current={item.isActive(page.url) ? 'page' : undefined}
				>
					<Icon name={item.icon} size={21} weight={item.isActive(page.url) ? 'fill' : 'regular'} />
					<span>{item.label}</span>
				</a>
			{/each}
		</nav>

		<div class="window-chrome">
			<div class="window-drag-region" data-tauri-drag-region aria-hidden="true" onwheel={scrollContentFromChrome}></div>
			<div class="window-controls" role="group" aria-label="Window controls">
				<button class="window-control" type="button" aria-label="Minimize window" title="Minimize" onclick={() => runWindowAction('minimize')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14" /></svg>
				</button>
				<button
					class="window-control"
					type="button"
					aria-label={windowMaximized ? 'Restore window' : 'Maximize window'}
					title={windowMaximized ? 'Restore' : 'Maximize'}
					onclick={() => runWindowAction('toggle-maximize')}
				>
					{#if windowMaximized}
						<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 8V5.5h10.5V16H16M5.5 8H16v10.5H5.5z" /></svg>
					{:else}
						<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5.5" y="5.5" width="13" height="13" rx="1.75" /></svg>
					{/if}
				</button>
				<button class="window-control window-control--close" type="button" aria-label="Close window" title="Close" onclick={() => runWindowAction('close')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m7 7 10 10M17 7 7 17" /></svg>
				</button>
			</div>
		</div>
	</div>
</div>

{#if activeMedia}
	{#key activeMedia.id}<PlayerHost media={activeMedia} onClose={closePlayer} />{/key}
{/if}

<svelte:window onkeydown={handleGlobalKeydown} />

<style>
	.app-stage {
		min-height: 100vh;
		padding: 0;
	}

	:global(html[data-runtime='desktop']) .app-stage { height: 100%; min-height: 0; overflow: hidden; }
	:global(html[data-runtime='desktop']) .app-stage { background: transparent; }
	:global(html[data-native-player]) .app-stage { opacity: 0; pointer-events: none; }

	.app-shell {
		position: relative;
		display: grid;
		grid-template-columns: 224px minmax(0, 1fr);
		height: 100vh;
		overflow: hidden;
		border: 0;
		border-radius: 0;
		background: transparent;
		box-shadow: none;
	}

	:global(html[data-runtime='desktop']) .app-shell { height: 100%; min-height: 0; }
	:global(html[data-runtime='desktop'][data-native-frame='css']) .app-shell.window-restored {
		border: 0;
		border-radius: 32px;
	}

	:global(html[data-runtime='desktop']) .app-shell {
		background: transparent;
	}

	.sidebar {
		position: relative;
		isolation: isolate;
		overflow: hidden;
		display: flex;
		min-height: 0;
		padding: 18px 13px 14px;
		flex-direction: column;
		border-right: 1px solid rgba(218, 231, 239, 0.12);
		background:
			radial-gradient(ellipse 130% 48% at 8% -8%, rgba(181, 216, 232, 0.13), transparent 48%),
			radial-gradient(ellipse 104% 38% at 86% 104%, rgba(91, 125, 151, 0.12), transparent 52%),
			linear-gradient(158deg, #12171d 0%, #040609 50%, #090c10 100%);
		box-shadow:
			inset -1px 0 rgba(0, 0, 0, 0.44),
			inset 1px 0 rgba(255, 255, 255, 0.06),
			inset 0 1px rgba(255, 255, 255, 0.09);
	}

	.sidebar::before {
		position: absolute;
		inset: 0;
		z-index: 0;
		background: linear-gradient(116deg, rgba(255, 255, 255, 0.11) -12%, transparent 28%, transparent 68%, rgba(172, 206, 220, 0.035) 100%);
		content: '';
		pointer-events: none;
	}

	.sidebar > * { position: relative; z-index: 1; }

	.library-context {
		display: flex;
		align-items: center;
		min-height: 44px;
		padding: 0 9px;
	}

	.library-wordmark { display: block; width: 72px; height: 24px; object-fit: contain; object-position: left center; }

	.sidebar-search {
		display: flex;
		align-items: center;
		gap: 9px;
		height: 38px;
		margin-top: 20px;
		padding: 0 11px;
		border: 1px solid rgba(255, 255, 255, 0.095);
		border-radius: 10px;
		color: rgba(229, 234, 239, 0.68);
		background: rgba(8, 11, 15, 0.23);
		box-shadow: inset 0 1px rgba(255, 255, 255, 0.035);
		font-size: 0.76rem;
		font-weight: 560;
		text-decoration: none;
		transition: border-color 150ms ease, background 150ms ease, color 150ms ease;
	}

	.sidebar-search:hover,
	.sidebar-search.active { border-color: rgba(255, 255, 255, 0.16); color: var(--text-strong); background: rgba(10, 13, 17, 0.36); }
	.sidebar-search kbd { margin-left: auto; color: rgba(225, 230, 235, 0.38); font: 540 0.62rem var(--font-ui); }

	.sidebar-nav { display: grid; gap: 8px; margin-top: 17px; }
	.sidebar-nav .sidebar-link { border-radius: 14px; }
	.sidebar-link {
		display: flex;
		align-items: center;
		gap: 11px;
		height: 39px;
		padding: 0 11px;
		border: 1px solid transparent;
		border-radius: 10px;
		color: rgba(222, 227, 232, 0.68);
		font-size: 0.78rem;
		font-weight: 560;
		letter-spacing: -0.012em;
		text-decoration: none;
		transition: background 150ms ease, color 150ms ease, transform 150ms ease;
	}

	.sidebar-link:hover { color: var(--text-strong); background: rgba(255, 255, 255, 0.065); }
	.sidebar-link.active {
		border-color: rgba(255, 255, 255, 0.12);
		color: #101317;
		background: rgba(245, 247, 249, 0.88);
		box-shadow: inset 0 1px rgba(255, 255, 255, 0.74), 0 8px 22px rgba(0, 0, 0, 0.18);
	}

	.sidebar-footer { display: grid; gap: 11px; margin-top: auto; }
	.profile-row {
		display: grid;
		grid-template-columns: 31px 1fr 18px;
		align-items: center;
		gap: 9px;
		min-height: 49px;
		padding: 7px 9px;
		border: 1px solid rgba(255, 255, 255, 0.08);
		border-radius: 12px;
		color: var(--text-soft);
		background: rgba(8, 11, 15, 0.17);
		cursor: pointer;
		text-align: left;
	}

	.profile-avatar {
		display: grid;
		place-items: center;
		border: 1px solid rgba(255, 255, 255, 0.17);
		border-radius: 50%;
		color: #0b0e11;
		background: #d7e3e6;
		font-size: 0.72rem;
		font-weight: 740;
		box-shadow: inset 0 1px rgba(255, 255, 255, 0.9), 0 5px 16px rgba(0, 0, 0, 0.18);
	}
	.profile-avatar { width: 31px; height: 31px; }
	.profile-copy strong,
	.profile-copy small { display: block; }
	.profile-copy strong { color: var(--text-strong); font-size: 0.71rem; font-weight: 650; }
	.profile-copy small { color: var(--text-dim); font-size: 0.61rem; }
	.profile-row > :global(svg) { color: var(--text-dim); }

	.workspace {
		position: relative;
		display: grid;
		grid-template-rows: minmax(0, 1fr);
		min-width: 0;
		min-height: 0;
		background: rgba(5, 7, 10, 0.54);
		-webkit-backdrop-filter: blur(20px) saturate(125%);
		backdrop-filter: blur(20px) saturate(125%);
	}

	:global(html[data-runtime='desktop']) .workspace {
		background: transparent;
		-webkit-backdrop-filter: none;
		backdrop-filter: none;
	}

	.window-chrome {
		position: absolute;
		top: 0;
		right: 0;
		left: 0;
		z-index: 80;
		display: none;
		grid-template-columns: minmax(0, 1fr) auto;
		height: 44px;
		pointer-events: none;
	}
	.window-drag-region {
		min-width: 0;
		height: 44px;
		cursor: default;
		pointer-events: auto;
		user-select: none;
		-webkit-app-region: drag;
	}
	.window-controls {
		display: flex;
		align-items: center;
		gap: 2px;
		height: 36px;
		margin: 7px 11px 0 0;
		padding: 3px;
		border: 1px solid rgba(237, 243, 247, 0.14);
		border-radius: 12px;
		background: rgba(16, 20, 25, 0.72);
		box-shadow: inset 0 1px rgba(255, 255, 255, 0.075), 0 8px 24px rgba(0, 0, 0, 0.22);
		-webkit-backdrop-filter: blur(18px) saturate(125%);
		backdrop-filter: blur(18px) saturate(125%);
		pointer-events: auto;
	}
	.window-control {
		display: grid;
		width: 34px;
		height: 28px;
		place-items: center;
		border: 0;
		border-radius: 8px;
		color: rgba(241, 245, 248, 0.78);
		background: transparent;
		cursor: pointer;
		transition: background 130ms ease, color 130ms ease, transform 130ms ease;
		-webkit-app-region: no-drag;
	}
	.window-control svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
	.window-control:hover { color: #fff; background: rgba(240, 247, 250, 0.14); }
	.window-control:active { transform: scale(0.94); }
	.window-control--close:hover { color: #fff; background: rgba(184, 91, 94, 0.7); }

	:global(html[data-runtime='desktop']) .window-chrome { display: grid; }

	.content-area {
		min-width: 0;
		min-height: 0;
		overflow-x: hidden;
		overflow-y: auto;
		scrollbar-width: none;
		-ms-overflow-style: none;
		overscroll-behavior: none;
	}
	.content-area::-webkit-scrollbar { display: none; width: 0; }
	.route-content { min-height: 100%; }
	.mobile-nav { display: none; }

	@media (max-width: 760px) {
		.app-stage { padding: 0; background: var(--surface-0); }
		.app-shell { display: block; min-height: 100vh; height: auto; overflow: visible; border: 0; border-radius: 0; box-shadow: none; }
		.sidebar { display: none; }
		.workspace { display: block; min-height: 100vh; background: var(--surface-0); backdrop-filter: none; }
		.content-area { overflow: visible; }
		.mobile-nav {
			position: fixed;
			right: 10px;
			bottom: max(10px, env(safe-area-inset-bottom));
			left: 10px;
			display: grid;
			grid-template-columns: repeat(5, 1fr);
			height: 62px;
			padding: 5px 8px;
			border: 1px solid rgba(255, 255, 255, 0.13);
			border-radius: 18px;
			background: rgba(34, 39, 45, 0.73);
			box-shadow: inset 0 1px rgba(255, 255, 255, 0.08), 0 18px 46px rgba(0, 0, 0, 0.48);
			-webkit-backdrop-filter: blur(34px) saturate(150%);
			backdrop-filter: blur(34px) saturate(150%);
			z-index: 60;
		}
		.mobile-nav__link { display: grid; place-items: center; align-content: center; gap: 2px; border-radius: 12px; color: var(--text-muted); font-size: 0.56rem; font-weight: 570; text-decoration: none; }
		.mobile-nav__link.active { color: var(--text-strong); background: rgba(255, 255, 255, 0.085); }
		.route-content { padding-bottom: 80px; }
	}

	@media (prefers-reduced-motion: reduce) {
		.sidebar-link,
		.window-control { transition: none; }
	}

	@media (prefers-reduced-transparency: reduce) {
		.sidebar,
		.workspace,
		.mobile-nav { -webkit-backdrop-filter: none; backdrop-filter: none; }
		.sidebar { background: #07090c; }
		.workspace { background: #090b0e; }
		.mobile-nav { background: #22272d; }
	}
</style>
