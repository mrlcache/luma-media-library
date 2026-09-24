<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import PosterGrid from '$lib/components/PosterGrid.svelte';
	import { media } from '$lib/data';
	import type { MediaKind } from '$lib/types';

	type Props = { heading?: string; initialType?: 'all' | MediaKind; initialQuery?: string };
	let { heading = 'Library', initialType = 'all', initialQuery = '' }: Props = $props();

	let query = $state('');
	let selectedType = $state<'all' | MediaKind>('all');
	let sort = $state('Recently added');

	$effect.pre(() => {
		query = initialQuery;
		selectedType = initialType;
	});

	let filteredMedia = $derived(
		media
			.filter((item) => selectedType === 'all' || item.kind === selectedType)
			.filter((item) => {
				const normalized = query.trim().toLowerCase();
				return !normalized || [item.title, item.genres.join(' '), item.year].join(' ').toLowerCase().includes(normalized);
			})
			.sort((a, b) => {
				if (sort === 'Title') return a.title.localeCompare(b.title);
				if (sort === 'Rating') return Number(b.rating) - Number(a.rating);
				return b.year - a.year;
			})
	);
	let collectionCount = $derived(selectedType === 'all' ? media.length : media.filter((item) => item.kind === selectedType).length);

	function selectType(type: 'all' | MediaKind) {
		selectedType = type;
	}

	function resetFilters() {
		query = '';
		selectedType = 'all';
	}
</script>

<svelte:head><title>{heading} · Media library</title></svelte:head>

<div class="library-page">
	<header class="library-heading">
		<h1>{heading}</h1>
		<p class="library-heading__meta">{collectionCount} titles</p>
	</header>

	<div class="library-controls">
		<label class="library-search">
			<Icon name="search" size={17} />
			<span class="sr-only">Search titles</span>
			<input bind:value={query} placeholder="Search titles, genres or years" type="search" autocomplete="off" />
			{#if query}<button class="search-clear" type="button" aria-label="Clear search" onclick={() => (query = '')}><Icon name="close" size={14} /></button>{/if}
		</label>
		<div class="library-controls__right">
			<div class="filter-group" role="group" aria-label="Filter by type">
				<button type="button" class:active={selectedType === 'all'} aria-pressed={selectedType === 'all'} onclick={() => selectType('all')}>All</button>
				<button type="button" class:active={selectedType === 'movie'} aria-pressed={selectedType === 'movie'} onclick={() => selectType('movie')}>Movies</button>
				<button type="button" class:active={selectedType === 'series'} aria-pressed={selectedType === 'series'} onclick={() => selectType('series')}>Series</button>
			</div>
			<label class="sort-control">
				<span>Sort by</span>
				<select bind:value={sort} aria-label="Sort library">
					<option>Recently added</option>
					<option>Title</option>
					<option>Rating</option>
				</select>
				<Icon name="chevron-down" size={14} />
			</label>
	</div>
	</div>

	<div class="result-line" aria-live="polite">
		<strong>{filteredMedia.length} {filteredMedia.length === 1 ? 'result' : 'results'}</strong>
		{#if query}<span class="result-line__query">for “{query}”</span>{/if}
	</div>

	{#if filteredMedia.length > 0}
		<PosterGrid items={filteredMedia} />
	{:else}
		<div class="empty-state">
			<Icon name="search" size={21} />
			<h2>No titles found</h2>
			<p>Try a different title, genre or year.</p>
			<button class="button button--secondary" type="button" onclick={resetFilters}>Reset filters</button>
		</div>
	{/if}
</div>

<style>
	.library-page { max-width: var(--content-width); margin: 0 auto; padding: 42px var(--content-gutter) 76px; }
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
	.library-controls__right { display: flex; align-items: center; gap: 25px; }
	.filter-group { display: inline-flex; align-items: center; gap: 19px; }
	.filter-group button { min-height: 30px; padding: 0 0 3px; border: 0; border-bottom: 2px solid transparent; color: var(--text-muted); font-size: 0.7rem; font-weight: 590; cursor: pointer; background: transparent; transition: color 140ms ease, border-color 140ms ease; }
	.filter-group button:hover { color: var(--text-soft); }
	.filter-group button.active { border-bottom-color: var(--accent); color: var(--text-strong); }
	.sort-control { display: flex; align-items: center; gap: 8px; color: var(--text-muted); font-size: 0.7rem; }
	.sort-control select { appearance: none; padding: 4px 18px 4px 0; border: 0; outline: none; color: var(--text-soft); font-size: 0.7rem; background: transparent; cursor: pointer; }
	.sort-control :global(svg) { margin-left: -17px; pointer-events: none; }
	.result-line { display: flex; align-items: center; gap: 8px; margin: 19px 0 17px; color: var(--text-soft); font-size: 0.72rem; }
	.result-line strong { color: var(--text-strong); font-weight: 620; }
	.result-line__query { color: var(--accent-soft); }
	.empty-state { display: grid; justify-items: center; gap: 8px; min-height: 360px; padding-top: 105px; color: var(--text-muted); text-align: center; }
	.empty-state h2 { margin: 2px 0 0; color: var(--text-strong); font-family: var(--font-display); font-size: 1.16rem; font-weight: 680; }
	.empty-state p { margin: 0 0 12px; color: var(--text-muted); font-size: 0.78rem; }
	.button { display: inline-flex; align-items: center; justify-content: center; min-height: 38px; padding: 0 15px; border: 1px solid transparent; border-radius: 9px; font-size: 0.76rem; font-weight: 620; cursor: pointer; transition: color 140ms ease, border-color 140ms ease, background-color 140ms ease; }
	.button--secondary { border-color: var(--line-subtle); color: var(--text-soft); background: var(--surface-2); }
	.button--secondary:hover { border-color: var(--line-strong); color: var(--text-strong); background: var(--surface-3); }
	@media (max-width: 880px) { .library-page { padding-right: 28px; padding-left: 28px; } .library-heading { align-items: start; flex-direction: column; gap: 9px; } .library-controls { align-items: stretch; flex-direction: column; gap: 17px; } .library-search { max-width: none; } .library-controls__right { justify-content: space-between; } }
	@media (max-width: 560px) { .library-page { padding: 32px 18px 48px; } .library-heading__meta { margin: 0; } .library-controls { margin-top: 26px; padding-top: 13px; } .library-controls__right { align-items: stretch; flex-direction: column; gap: 14px; } .filter-group { justify-content: space-between; gap: 15px; } .filter-group button { flex: 1; } .sort-control { justify-content: space-between; } }
	@media (prefers-reduced-motion: reduce) { .filter-group button, .button { transition: none; } }
</style>
