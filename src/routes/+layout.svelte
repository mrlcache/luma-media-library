<script lang="ts">
	import { onMount, setContext } from 'svelte';
	import { page } from '$app/state';
	import Icon from '$lib/components/Icon.svelte';
	import PlayerHost from '$lib/components/PlayerHost.svelte';
	import { featuredAmbient } from '$lib/data';
	import { PLAYER_CONTEXT } from '$lib/player-context';
	import { readDesktopBootstrap } from '$lib/platform/desktop';
	import type { IconName } from '$lib/components/Icon.svelte';
	import type { MediaItem } from '$lib/types';
	import '../app.css';

	let { children } = $props();
	let activeMedia = $state<MediaItem | null>(null);

	function openPlayer(media: MediaItem) {
		activeMedia = media;
	}

	function closePlayer() {
		activeMedia = null;
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
		}
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
		void readDesktopBootstrap()
			.then((bootstrap) => {
				if (!bootstrap) return;
				document.documentElement.dataset.runtime = bootstrap.platform;
				document.documentElement.dataset.appVersion = bootstrap.version;
			})
			.catch((error) => console.warn('Desktop bootstrap unavailable', error));
	});

</script>

<svelte:head>
	<title>Media library</title>
	<meta name="description" content="A focused media library for browsing and playback." />
</svelte:head>

<div class="app-stage" data-tauri-drag-region style={`--shell-backdrop: url("${featuredAmbient}")`}>
	<div class="app-shell">
		<aside class="sidebar" aria-label="Application navigation">
			<div class="library-context">
				<div class="library-mark" aria-hidden="true"><Icon name="play" size={13} weight="fill" /></div>
				<div>
					<strong>Media Library</strong>
					<span>Personal collection</span>
				</div>
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
			<main class="content-area"><div class="route-content">{@render children()}</div></main>
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
	</div>
</div>

{#if activeMedia}
	<PlayerHost media={activeMedia} onClose={closePlayer} />
{/if}

<style>
	.app-stage {
		min-height: 100vh;
		padding: 12px;
		background-image:
			linear-gradient(135deg, rgba(18, 26, 35, 0.3), rgba(3, 5, 8, 0.72)),
			var(--shell-backdrop);
		background-position: center;
		background-size: cover;
		background-attachment: fixed;
	}

	.app-shell {
		display: grid;
		grid-template-columns: 224px minmax(0, 1fr);
		height: calc(100vh - 24px);
		overflow: hidden;
		border: 1px solid rgba(255, 255, 255, 0.17);
		border-radius: 20px;
		background: rgba(7, 9, 12, 0.48);
		box-shadow:
			inset 0 1px rgba(255, 255, 255, 0.1),
			0 28px 90px rgba(0, 0, 0, 0.58);
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
			linear-gradient(158deg, rgba(18, 23, 29, 0.98) 0%, rgba(4, 6, 9, 0.985) 50%, rgba(9, 12, 16, 0.99) 100%);
		box-shadow:
			inset -1px 0 rgba(0, 0, 0, 0.44),
			inset 1px 0 rgba(255, 255, 255, 0.06),
			inset 0 1px rgba(255, 255, 255, 0.09);
		-webkit-backdrop-filter: blur(44px) saturate(135%);
		backdrop-filter: blur(44px) saturate(135%);
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
		gap: 11px;
		min-height: 44px;
		padding: 0 9px;
	}

	.library-mark {
		display: grid;
		width: 31px;
		height: 31px;
		place-items: center;
		border: 1px solid rgba(255, 255, 255, 0.18);
		border-radius: 9px;
		color: #0a0c0f;
		background: rgba(245, 247, 248, 0.92);
		box-shadow: inset 0 1px rgba(255, 255, 255, 0.9), 0 7px 18px rgba(0, 0, 0, 0.19);
	}

	.library-context strong,
	.library-context span { display: block; }
	.library-context strong { color: var(--text-strong); font-size: 0.82rem; font-weight: 690; letter-spacing: -0.025em; }
	.library-context span { margin-top: 1px; color: rgba(221, 226, 232, 0.54); font-size: 0.66rem; font-weight: 520; }

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
		display: grid;
		grid-template-rows: minmax(0, 1fr);
		min-width: 0;
		min-height: 0;
		background: rgba(5, 7, 10, 0.54);
		-webkit-backdrop-filter: blur(20px) saturate(125%);
		backdrop-filter: blur(20px) saturate(125%);
	}

	.content-area { min-width: 0; min-height: 0; overflow-x: hidden; overflow-y: auto; }
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
		.sidebar-link { transition: none; }
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
