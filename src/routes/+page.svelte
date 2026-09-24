<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import CatalogSkeletonGrid from '$lib/components/CatalogSkeletonGrid.svelte';
	import EmptyLibraryCard from '$lib/components/EmptyLibraryCard.svelte';
	import LocalCatalogGrid from '$lib/components/LocalCatalogGrid.svelte';
	import { isDesktopRuntime, readCatalogPage } from '$lib/platform/desktop';
	import { nativeAcrylicStatus, requestNativeAcrylic } from '$lib/platform/native-acrylic';
	import type { CatalogMedia } from '$lib/types';

	let homeContent: HTMLDivElement;
	let desktopCatalog = $state(false);
	let catalogItems = $state<CatalogMedia[]>([]);
	let catalogTotal = $state(0);
	let catalogLoading = $state(true);
	let catalogError = $state('');

	onMount(() => {
		desktopCatalog = isDesktopRuntime();
		if (desktopCatalog) {
			catalogLoading = true;
			void readCatalogPage(0, 48, undefined, undefined, 'Recently added')
				.then((page) => {
					if (!page) throw new Error('The local catalog is only available in the desktop app.');
					catalogItems = page.items;
					catalogTotal = page.total;
				})
				.catch((error) => {
					catalogError = error instanceof Error ? error.message : 'The local library could not be read.';
				})
				.finally(() => (catalogLoading = false));
		} else {
			catalogLoading = false;
		}

		if (!desktopCatalog) return;

		const acrylicRequester = Symbol('Home HSS');
		const contentArea = homeContent.closest('.content-area');
		const observer = new IntersectionObserver(
			(entries) => {
				const entry = entries[entries.length - 1];
				if (!entry) return;
				requestNativeAcrylic(
					acrylicRequester,
					entry.isIntersecting && entry.intersectionRect.width > 2 && entry.intersectionRect.height > 2
				);
			},
			{
				root: contentArea,
				// Keep the native material out of the 44px window-controls overlay.
				rootMargin: '-44px 0px 0px 0px',
				threshold: 0
			}
		);
		observer.observe(homeContent);
		return () => {
			observer.disconnect();
			requestNativeAcrylic(acrylicRequester, false);
		};
	});
</script>

<svelte:head><title>Home · Media library</title></svelte:head>

<div class="home-page">
	<div class="home-content" bind:this={homeContent} data-native-backdrop={$nativeAcrylicStatus}>
		<div class="home-content__inner">
			{#if catalogLoading}
				<section class="home-section" aria-labelledby="library-heading" aria-busy="true">
					<div class="section-heading"><h1 id="library-heading">Your library</h1></div>
					<CatalogSkeletonGrid count={8} label="Loading your library" />
				</section>
			{:else if catalogError}
				<EmptyLibraryCard title="Library unavailable" description={catalogError} role="status" fullHeight />
			{:else if catalogTotal > 0}
				<section class="home-section" aria-labelledby="library-heading">
					<div class="section-heading">
						<div>
							<h1 id="library-heading">Your library</h1>
							<p>{catalogTotal} {catalogTotal === 1 ? 'title' : 'titles'}</p>
						</div>
						<a href="/library" class="section-link">View all <Icon name="chevron-right" size={15} weight="bold" /></a>
					</div>
					<LocalCatalogGrid items={catalogItems} />
				</section>
			{:else}
				<EmptyLibraryCard
				title={desktopCatalog ? 'Your library is empty' : 'Your library is in the desktop app'}
				description={desktopCatalog ? 'Add a media folder in Settings to scan your videos and find matching artwork.' : 'Open the desktop app to scan a local media folder.'}
				showSettingsLink={desktopCatalog}
				fullHeight
			/>
			{/if}
		</div>
	</div>
</div>

<style>
	.home-page { min-height: 100vh; min-height: 100dvh; background: #080a0d; }
	:global(html[data-runtime='desktop']) .home-page { background: transparent; }

	.home-content {
		position: relative;
		width: 100%;
		min-height: 100vh;
		min-height: 100dvh;
		background: #101419;
		z-index: 2;
	}
	/* DWM supplies desktop Acrylic; keep this tint translucent so it shows through. */
	:global(html[data-runtime='desktop']) .home-content {
		background: var(--acrylic-content-tint);
		-webkit-backdrop-filter: none;
		backdrop-filter: none;
	}
	:global(html[data-runtime='desktop'] .home-content[data-native-backdrop='unavailable']) {
		background: var(--acrylic-content-fallback);
		-webkit-backdrop-filter: none;
		backdrop-filter: none;
	}

	.home-content__inner {
		box-sizing: border-box;
		display: grid;
		grid-template-rows: minmax(0, 1fr);
		max-width: var(--content-width);
		min-height: 100vh;
		min-height: 100dvh;
		margin: 0 auto;
		padding: 42px var(--content-gutter);
	}
	.home-section { min-width: 0; }
	.section-heading { display: flex; align-items: end; justify-content: space-between; gap: 18px; margin-bottom: 20px; }
	.section-heading h1 { margin: 0; color: var(--text-strong); font-family: var(--font-display); font-size: 1.24rem; font-weight: 690; letter-spacing: -0.038em; }
	.section-heading p { margin: 6px 0 0; color: var(--text-muted); font-size: 0.73rem; }
	.section-link { display: inline-flex; align-items: center; gap: 4px; flex: 0 0 auto; color: var(--text-muted); font-size: 0.71rem; font-weight: 590; text-decoration: none; transition: color 140ms ease; }
	.section-link:hover { color: var(--text-strong); }

	@media (max-width: 760px) {
		.home-content__inner { padding: 30px 18px; }
		.section-heading { align-items: center; margin-bottom: 16px; }
		.section-heading h1 { font-size: 1.08rem; }
	}
	@media (prefers-reduced-motion: reduce) {
		.section-link { transition: none; }
	}
	@media (prefers-reduced-transparency: reduce) {
		.home-content { background: #0c0f13; }
		:global(html[data-runtime='desktop']) .home-content { background: #0c0f13; -webkit-backdrop-filter: none; backdrop-filter: none; }
	}
</style>
