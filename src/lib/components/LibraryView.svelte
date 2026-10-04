<script lang="ts">
	import AppSelect from '$lib/components/AppSelect.svelte';
	import { onMount, untrack } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { recoverRemoteArtwork } from '$lib/media/artwork';
	import CatalogSkeletonGrid from '$lib/components/CatalogSkeletonGrid.svelte';
	import EmptyLibraryCard from '$lib/components/EmptyLibraryCard.svelte';
	import LocalCatalogGrid from '$lib/components/LocalCatalogGrid.svelte';
	import { nativeAcrylicStatus, requestNativeAcrylic } from '$lib/platform/native-acrylic';
	import { isDesktopRuntime, peekCatalogPage, readCatalogPage, searchTmdb } from '$lib/platform/desktop';
	import type { CatalogMedia, TmdbSearchResult } from '$lib/types';
	import type { MediaKind } from '$lib/types';
	import { libraryTransfers, transferKind, transferTitle, transferCard, type LibraryCard } from '$lib/torrents/library';
	import { favorites, favoriteCard, cardMedia, isFavorite, sameFavorite } from '$lib/media/favorites';

	type Props = { heading?: string; initialType?: 'all' | MediaKind; initialQuery?: string };
	let { heading = 'Library', initialType = 'all', initialQuery = '' }: Props = $props();

	let query = $state('');
	let selectedType = $state<'all' | MediaKind>('all');
	let sort = $state('Recently added');
	let favoritesOnly = $state(false);
	let favoriteLocalItems = $state<CatalogMedia[]>([]);
	let favoriteLookupVersion = $state(0);
	let filteredFavorites = $derived($favorites.filter(({media}) =>
		(selectedType === 'all' || media.kind === selectedType)
		&& `${media.title} ${media.year} ${media.genres.join(' ')}`.toLowerCase().includes(query.trim().toLowerCase())));
	let tmdbResults = $state<TmdbSearchResult[]>([]);
	let tmdbLoading = $state(false);
	let tmdbError = $state('');
	let desktopCatalog = $state(isDesktopRuntime());
	let catalogItems = $state<CatalogMedia[]>([]);
	let catalogTotal = $state(0);
	let catalogLoading = $state(true);
	let catalogError = $state('');
	let catalogRequest = 0;
	const isGlobalSearch = $derived(heading === 'Search');
	const catalogPageSize = 48;
	let visibleTransfers = $derived($libraryTransfers.filter((item) =>
		(selectedType === 'all' || transferKind(item) === selectedType)
		&& transferTitle(item).toLowerCase().includes(query.trim().toLowerCase())
	).sort((a, b) => sort === 'Title' ? transferTitle(a).localeCompare(transferTitle(b)) : b.queuePosition - a.queuePosition));
	let displayItems = $derived.by(() => {
		const key = (item: LibraryCard) => `${item.kind}:${item.title.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().replace(/[^\p{L}\p{N}]/gu, '')}:${item.year ?? ''}`;
		const combined = new Map<string, LibraryCard>(catalogItems.map((item) => [key(item), item]));
		for (const entry of filteredFavorites) {
			const saved = favoriteCard(entry);
			const downloaded = favoriteLocalItems.find((item) => sameFavorite(cardMedia(item), entry.media));
			const card = downloaded ?? saved;
			if (!combined.has(key(card))) combined.set(key(card), card);
		}
		for (const item of visibleTransfers.map(transferCard)) {
			const existing = combined.get(key(item));
			if (!existing) combined.set(key(item), item);
			else if (item.transfer && item.transfer.progress < 1) combined.set(key(item), { ...existing, transfer: item.transfer });
		}
		return [...combined.values()].filter((item) => !favoritesOnly || isFavorite($favorites, cardMedia(item))).sort((a, b) => {
			if (sort === 'Title') return a.title.localeCompare(b.title);
			if (sort === 'Rating') return (b.voteAverage ?? 0) - (a.voteAverage ?? 0);
			return (b.modifiedAt ?? 0) - (a.modifiedAt ?? 0);
		});
	});

	$effect.pre(() => {
		query = initialQuery;
		selectedType = initialType;
	});

	$effect.pre(() => {
		if (!desktopCatalog || isGlobalSearch) return;
		const cached = peekCatalogPage(0, catalogPageSize,
			selectedType === 'all' ? undefined : selectedType, query.trim() || undefined, sort);
		untrack(() => {
			// Invalidate the previous view before its pending request can paint under a new heading.
			catalogRequest += 1;
			catalogItems = cached?.items ?? [];
			catalogTotal = cached?.total ?? 0;
			catalogLoading = !cached;
			catalogError = '';
		});
	});

	let libraryTotal = $derived(favoritesOnly ? displayItems.length : catalogTotal + displayItems.filter((item) => typeof item.id !== 'number').length);
	let catalogCountLabel = $derived(`${libraryTotal} ${libraryTotal === 1 ? 'title' : 'titles'}`);
	let filteredTmdbResults = $derived(
		tmdbResults
			.filter((item) => selectedType === 'all' || item.kind === selectedType)
			.sort((a, b) => {
				if (sort === 'Title') return a.title.localeCompare(b.title);
				if (sort === 'Rating') return (b.voteAverage ?? 0) - (a.voteAverage ?? 0);
				return (b.year ?? 0) - (a.year ?? 0);
			})
	);

	function selectType(type: 'all' | MediaKind) {
		selectedType = type;
	}

	function resetFilters() {
		query = '';
		selectedType = 'all';
		favoritesOnly = false;
	}

	$effect(() => {
		const entries = $favorites;
		void favoriteLookupVersion;
		if (!desktopCatalog || isGlobalSearch) return;
		let cancelled = false;
		// Resolve saved titles separately from pagination so a downloaded favorite
		// keeps its play action even when it is outside the loaded catalog page.
		void (async () => {
			const found: CatalogMedia[] = [];
			for (const entry of entries) {
				if (cancelled) return;
				try {
					const page = await readCatalogPage(0, 48, entry.media.kind, entry.media.title, 'Title');
					const match = page?.items.find((item) => sameFavorite(cardMedia(item), entry.media));
					if (match) found.push(match);
				} catch { /* Saved metadata remains available if the local catalog is offline. */ }
			}
			if (!cancelled) favoriteLocalItems = found;
		})();
		return () => { cancelled = true; };
	});

	async function requestCatalogPage(reset = true) {
		if (!desktopCatalog || isGlobalSearch) return;
		const request = ++catalogRequest;
		const offset = reset ? 0 : catalogItems.length;
		catalogLoading = reset ? !peekCatalogPage(0, catalogPageSize,
			selectedType === 'all' ? undefined : selectedType, query.trim() || undefined, sort) : true;
		catalogError = '';
		try {
			const page = await readCatalogPage(
				offset,
				catalogPageSize,
				selectedType === 'all' ? undefined : selectedType,
				query.trim() || undefined,
				sort
			);
			if (request !== catalogRequest) return;
			if (!page) throw new Error('The local catalog is only available in the desktop app.');
			catalogItems = reset ? page.items : [...catalogItems, ...page.items];
			catalogTotal = page.total;
		} catch (error) {
			if (request === catalogRequest) catalogError = error instanceof Error ? error.message : 'The local catalog could not be read.';
		} finally {
			if (request === catalogRequest) catalogLoading = false;
		}
	}

	$effect(() => {
		if (!desktopCatalog || isGlobalSearch) return;
		void selectedType;
		void sort;
		const term = query;
		const timer = window.setTimeout(() => void requestCatalogPage(true), term ? 160 : 0);
		return () => window.clearTimeout(timer);
	});

	$effect(() => {
		if (!isGlobalSearch) return;
		const term = query.trim();
		if (term.length < 2) {
			tmdbResults = [];
			tmdbLoading = false;
			tmdbError = '';
			return;
		}
		if (!isDesktopRuntime()) {
			tmdbResults = [];
			tmdbLoading = false;
			tmdbError = 'TMDb search is available in the desktop app.';
			return;
		}

		let cancelled = false;
		tmdbLoading = true;
		tmdbError = '';
		const timer = window.setTimeout(() => {
			void searchTmdb(term)
				.then((results) => {
					if (!cancelled) tmdbResults = results;
				})
				.catch((error) => {
					if (!cancelled) tmdbError = error instanceof Error ? error.message : 'TMDb search failed.';
				})
				.finally(() => {
					if (!cancelled) tmdbLoading = false;
				});
		}, 320);

		return () => {
			cancelled = true;
			window.clearTimeout(timer);
		};
	});

	onMount(() => {
		const acrylicRequester = Symbol('Library surface');
		desktopCatalog = isDesktopRuntime();
		requestNativeAcrylic(acrylicRequester, true);
		const update = () => { favoriteLookupVersion += 1; void requestCatalogPage(true); };
		window.addEventListener('luma-library-changed', update);
		return () => { catalogRequest += 1; window.removeEventListener('luma-library-changed', update); requestNativeAcrylic(acrylicRequester, false); };
	});
</script>

<svelte:head><title>{heading} · Luma</title></svelte:head>

<div class="library-surface" data-native-backdrop={$nativeAcrylicStatus}>
	<div class="library-page" class:library-page--collection={!isGlobalSearch}>
	<header class="library-heading" class:library-heading--collection={!isGlobalSearch}>
		<h1>{heading}</h1>
		<p class="library-heading__meta">{isGlobalSearch ? 'Movies and series · TMDb' : desktopCatalog ? catalogCountLabel : 'Local collection'}</p>
	</header>

	<div class="library-controls">
		<label class="library-search">
			<Icon name="search" size={17} />
			<span class="sr-only">Search titles</span>
			<input bind:value={query} placeholder={isGlobalSearch ? 'Search movies and series' : 'Search titles, genres or years'} type="search" autocomplete="off" />
			{#if query}<button class="search-clear" type="button" aria-label="Clear search" onclick={() => (query = '')}><Icon name="close" size={14} /></button>{/if}
		</label>
		<div class="library-controls__right">
			<div class="filter-group" role="group" aria-label="Filter by type">
				<button type="button" class:active={selectedType === 'all'} aria-pressed={selectedType === 'all'} onclick={() => selectType('all')}>All</button>
				<button type="button" class:active={selectedType === 'movie'} aria-pressed={selectedType === 'movie'} onclick={() => selectType('movie')}>Movies</button>
				<button type="button" class:active={selectedType === 'series'} aria-pressed={selectedType === 'series'} onclick={() => selectType('series')}>Series</button>
			</div>
			<div class="sort-control">
				<span>Sort by</span>
				<AppSelect bind:value={sort} label="Sort library" variant="plain" options={[{value:"Recently added",label:"Recently added"},{value:"Title",label:"Title"},{value:"Rating",label:"Rating"}]} />
			</div>
			{#if !isGlobalSearch}<button type="button" class="favorites-filter" class:active={favoritesOnly} aria-pressed={favoritesOnly} onclick={() => favoritesOnly = !favoritesOnly}><Icon name="heart" size={14} weight={favoritesOnly ? 'fill' : 'regular'} />Favorites</button>{/if}
	</div>
	</div>

	<div class="result-line" aria-live="polite">
		{#if isGlobalSearch}
			<strong>{query.trim().length < 2 ? 'Search TMDb' : tmdbLoading ? 'Searching…' : `${filteredTmdbResults.length} ${filteredTmdbResults.length === 1 ? 'result' : 'results'}`}</strong>
		{:else if desktopCatalog && !(catalogLoading && catalogItems.length === 0)}
			<strong>{`${libraryTotal} ${libraryTotal === 1 ? 'result' : 'results'}`}</strong>
		{:else if desktopCatalog}
			<strong>Loading your library…</strong>
		{:else}
			<strong>Open the desktop app</strong>
		{/if}
		{#if query}<span class="result-line__query">for “{query}”</span>{/if}
	</div>

	{#if isGlobalSearch}
		{#if query.trim().length < 2}
			<div class="empty-state"><Icon name="search" size={21} /><h2>Search movies and series</h2><p>Enter at least two characters to look up metadata.</p></div>
		{:else if tmdbLoading}
			<div class="empty-state" role="status"><Icon name="search" size={21} /><h2>Searching The Movie Database</h2><p>Looking for matching titles.</p></div>
		{:else if tmdbError}
			<div class="empty-state" role="status"><Icon name="search" size={21} /><h2>Search is unavailable</h2><p>{tmdbError}</p></div>
		{:else if filteredTmdbResults.length > 0}
			<div class="tmdb-results" role="list" aria-label="The Movie Database search results">
				{#each filteredTmdbResults as item (item.kind + item.id)}
					<article class="tmdb-result" role="listitem">
						{#if item.posterUrl}
							<img use:recoverRemoteArtwork class="tmdb-result__poster" src={item.posterUrl} alt={`Poster for ${item.title}`} width="342" height="513" loading="lazy" decoding="async" />
						{:else}
							<div class="tmdb-result__poster tmdb-result__poster--empty" aria-label="Poster unavailable"></div>
						{/if}
						<div class="tmdb-result__copy">
							<p class="tmdb-result__meta">{item.kind === 'series' ? 'Series' : 'Movie'}{item.year ? ` · ${item.year}` : ''}{item.voteAverage ? ` · ${item.voteAverage.toFixed(1)}` : ''}</p>
							<h2>{item.title}</h2>
							<p class="tmdb-result__overview">{item.overview || 'No overview available.'}</p>
						</div>
					</article>
				{/each}
			</div>
		{:else}
			<div class="empty-state"><Icon name="search" size={21} /><h2>No titles found</h2><p>Try a different title.</p></div>
		{/if}
	{:else if desktopCatalog}
		{#if favoritesOnly && displayItems.length === 0}
			<EmptyLibraryCard title={query ? 'No favorites found' : 'No favorites yet'} description={query ? 'Try a different search or type filter.' : 'Use the heart on a title to save it here, including titles you have not downloaded.'} centered />
		{:else if catalogError && displayItems.length === 0}
			<EmptyLibraryCard title="Library unavailable" description={catalogError} role="status" centered />
		{:else if catalogLoading && displayItems.length === 0}
			<CatalogSkeletonGrid count={12} label="Loading your library" />
		{:else if catalogTotal === 0 && displayItems.length === 0 && !query.trim()}
			<EmptyLibraryCard
				title={selectedType === 'all' ? 'Your library is empty' : `No ${selectedType === 'movie' ? 'movies' : 'series'} in your library`}
				description={selectedType === 'all'
					? 'Add a media folder in Settings to scan your videos and find matching artwork.'
					: `No ${selectedType === 'movie' ? 'movies' : 'series'} were found in this view. Review your folders in Settings.`}
				showSettingsLink
				centered
			/>
		{:else if displayItems.length > 0}
			<div aria-busy={catalogLoading}>
				<LocalCatalogGrid items={displayItems} />
			</div>
			{#if !favoritesOnly && catalogItems.length < catalogTotal}
				<div class="catalog-more"><button class="button button--secondary" type="button" disabled={catalogLoading} onclick={() => void requestCatalogPage(false)}>{catalogLoading ? 'Loading…' : 'Load more titles'}</button></div>
			{/if}
		{:else}
			<div class="empty-state"><Icon name="search" size={21} /><h2>No titles found</h2><p>Try a different title, genre or year.</p><button class="button button--secondary" type="button" onclick={resetFilters}>Reset filters</button></div>
		{/if}
	{:else}
		<EmptyLibraryCard title="Local library is in the desktop app" description="Open the desktop app to scan and browse your media folders." centered />
	{/if}
	</div>
</div>

<style>
	.favorites-filter { display:inline-flex; align-items:center; gap:6px; min-height:30px; padding:0 0 3px; border:0; border-bottom:2px solid transparent; background:transparent; color:var(--text-muted); font-size:.7rem; font-weight:590; cursor:pointer; }
	.favorites-filter:hover { color:var(--text-soft); }
	.favorites-filter.active { border-bottom-color:var(--accent); color:var(--text-strong); }
	.library-surface { min-height: 100vh; background: #101419; }
	:global(html[data-runtime='desktop']) .library-surface { background: var(--acrylic-content-tint); }
	:global(html[data-runtime='desktop'] .library-surface[data-native-backdrop='unavailable']) { background: var(--acrylic-content-fallback); }
	.library-page { position: relative; max-width: var(--content-width); min-height: 100vh; min-height: 100dvh; margin: 0 auto; padding: 42px var(--content-gutter) 76px; }
	.library-heading { display: flex; align-items: end; justify-content: space-between; gap: 24px; }
	.library-heading h1 { margin: 0; color: var(--text-strong); font-family: var(--font-display); font-size: clamp(2.35rem, 4vw, 3.7rem); font-weight: 650; letter-spacing: -0.065em; line-height: 0.98; }
	.library-heading__meta { margin: 0 0 4px; color: var(--text-muted); font-size: 0.72rem; font-weight: 560; }
	.library-controls { display: flex; align-items: center; justify-content: space-between; gap: 24px; margin-top: 32px; padding: 12px 0 13px; border-top: 1px solid var(--line-subtle); border-bottom: 1px solid var(--line-subtle); }
	.library-search { display: flex; align-items: center; gap: 10px; flex: 1; max-width: 440px; color: var(--text-muted); }
	.library-search input { width: 100%; min-width: 0; padding: 0; border: 0; outline: none; color: var(--text-strong); font-size: 0.82rem; background: transparent; }
	.library-search input::placeholder { color: var(--text-dim); }
	.library-search input::-webkit-search-cancel-button { display: none; }
	.search-clear { display: grid; place-items: center; width: 24px; height: 24px; padding: 0; border: 0; border-radius: 50%; color: var(--text-muted); background: transparent; cursor: pointer; }
	.search-clear:hover { color: var(--text-strong); background: var(--surface-2); }
	.library-controls__right { display: flex; flex-shrink: 0; align-items: center; gap: 25px; }
	.filter-group { display: inline-flex; align-items: center; gap: 19px; }
	.filter-group button { min-height: 30px; padding: 0 0 3px; border: 0; border-bottom: 2px solid transparent; color: var(--text-muted); font-size: 0.7rem; font-weight: 590; cursor: pointer; background: transparent; transition: color 140ms ease, border-color 140ms ease; }
	.filter-group button:hover { color: var(--text-soft); }
	.filter-group button.active { border-bottom-color: var(--accent); color: var(--text-strong); }
	.sort-control { display: flex; align-items: center; gap: 8px; color: var(--text-muted); font-size: 0.7rem; }
	.result-line { display: flex; align-items: center; gap: 8px; min-height: 1em; margin: 19px 0 17px; color: var(--text-soft); font-size: 0.72rem; }
	.result-line strong { color: var(--text-strong); font-weight: 620; }
	.result-line__query { color: var(--accent-soft); }
	.empty-state { display: grid; justify-items: center; align-content: center; gap: 8px; min-height: 360px; padding: 40px 24px; color: var(--text-muted); text-align: center; }
	.empty-state h2 { margin: 2px 0 0; color: var(--text-strong); font-family: var(--font-display); font-size: 1.16rem; font-weight: 680; }
	.empty-state p { margin: 0 0 12px; color: var(--text-muted); font-size: 0.78rem; }
	.tmdb-results { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 150px), 1fr)); gap: 22px 18px; margin: 0 -14px; padding: 0 14px 20px; }
	.tmdb-result { min-width: 0; }
	.tmdb-result__poster { display: block; width: 100%; aspect-ratio: 2 / 3; border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; object-fit: cover; background: var(--surface-2); }
	.tmdb-result__poster--empty { background: linear-gradient(145deg, var(--surface-2), var(--surface-1)); }
	.tmdb-result__copy { padding: 10px 2px 0; }
	.tmdb-result__meta { margin: 0 0 4px; color: var(--text-muted); font-size: 0.64rem; }
	.tmdb-result__copy h2 { overflow: hidden; margin: 0; color: var(--text-soft); font-size: 0.82rem; font-weight: 620; text-overflow: ellipsis; white-space: nowrap; }
	.tmdb-result__overview { display: -webkit-box; overflow: hidden; margin: 6px 0 0; color: var(--text-muted); font-size: 0.68rem; line-height: 1.5; line-clamp: 3; -webkit-box-orient: vertical; -webkit-line-clamp: 3; }
	.button { display: inline-flex; align-items: center; justify-content: center; min-height: 38px; padding: 0 15px; border: 1px solid transparent; border-radius: 9px; font-size: 0.76rem; font-weight: 620; cursor: pointer; transition: color 140ms ease, border-color 140ms ease, background-color 140ms ease; }
	.button--secondary { border-color: var(--line-subtle); color: var(--text-soft); background: var(--surface-2); }
	.button--secondary:hover { border-color: var(--line-strong); color: var(--text-strong); background: var(--surface-3); }
	.catalog-more { display: flex; justify-content: center; padding: 0 0 34px; }
	/* Break against the space left by the sidebar, before controls begin to compete. */
	@media (min-width: 761px) and (max-width: 1280px) {
		.library-page { padding-inline: 32px; }
		.library-controls { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; margin-top: 26px; }
		.library-search { max-width: none; min-height: 34px; }
		.library-controls__right { justify-content: space-between; gap: 16px; min-width: 0; }
		.filter-group { gap: 20px; }
	}
	@media (max-width: 880px) { .library-page { padding-right: 28px; padding-left: 28px; } .library-heading { gap: 16px; } .library-controls { align-items: stretch; flex-direction: column; gap: 12px; } .library-search { max-width: none; } .library-controls__right { justify-content: space-between; gap: 16px; } }
	@media (max-width: 560px) { .library-page { min-height: 100vh; min-height: 100dvh; padding: 32px 18px 48px; } .library-heading__meta { margin: 0; } .library-controls { margin-top: 26px; padding-top: 13px; } .library-controls__right { align-items: stretch; flex-direction: column; gap: 14px; } .filter-group { justify-content: space-between; gap: 15px; } .filter-group button { flex: 1; } .sort-control { justify-content: space-between; } }
	@media (prefers-reduced-motion: reduce) { .filter-group button, .button { transition: none; } }
	:global(html:not([data-mobile-preview='true'])) .library-heading--collection { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
	:global(html:not([data-mobile-preview='true'])) .library-page--collection .library-controls { margin-top: 0; border-top: 0; padding-top: 0; }
	@media (prefers-reduced-transparency: reduce) {
		:global(html[data-runtime='desktop']) .library-surface { background: #0c0f13; }
	}
</style>
