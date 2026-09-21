<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import VirtualPosterGrid from '$lib/components/VirtualPosterGrid.svelte';
	import { media } from '$lib/data';
	import type { MediaKind } from '$lib/types';

	type Props = { heading?: string; description?: string; initialType?: 'all' | MediaKind; initialQuery?: string };
	let { heading = 'Library', description = 'A focused view of everything in your collection.', initialType = 'all', initialQuery = '' }: Props = $props();

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

	function selectType(type: 'all' | MediaKind) {
		selectedType = type;
	}
</script>

<svelte:head><title>{heading} · Media library</title></svelte:head>

<div class="library-page">
	<div class="library-heading">
		<div>
			<p class="page-kicker">Collection</p>
			<h1>{heading}</h1>
			<p class="page-description">{description}</p>
		</div>
		<div class="library-heading__meta"><span class="meta-dot"></span>{media.length} titles indexed</div>
	</div>

	<div class="library-toolbar">
		<label class="library-search">
			<Icon name="search" size={17} />
			<span class="sr-only">Search titles</span>
			<input bind:value={query} placeholder="Search titles, genres or years" type="search" autocomplete="off" />
			{#if query}<button class="search-clear" aria-label="Clear search" onclick={() => (query = '')}><Icon name="close" size={14} /></button>{/if}
		</label>
		<div class="toolbar-controls">
			<div class="filter-group" aria-label="Filter by type">
				<button class:active={selectedType === 'all'} aria-pressed={selectedType === 'all'} onclick={() => selectType('all')}>All</button>
				<button class:active={selectedType === 'movie'} aria-pressed={selectedType === 'movie'} onclick={() => selectType('movie')}>Movies</button>
				<button class:active={selectedType === 'series'} aria-pressed={selectedType === 'series'} onclick={() => selectType('series')}>Series</button>
			</div>
			<label class="sort-control">
				<span>Sort</span>
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
		<span>{filteredMedia.length} {filteredMedia.length === 1 ? 'result' : 'results'}</span>
		{#if query}<span class="result-line__query">for “{query}”</span>{/if}
		<span class="result-line__hint"><Icon name="sliders" size={13} /> Filters update instantly</span>
	</div>

	{#if filteredMedia.length > 0}
		<VirtualPosterGrid items={filteredMedia} />
	{:else}
		<div class="empty-state">
			<div class="empty-state__icon"><Icon name="search" size={22} /></div>
			<h2>No titles found</h2>
			<p>Try a different title, genre or year.</p>
			<button class="button button--secondary" onclick={() => { query = ''; selectedType = 'all'; }}>Reset filters</button>
		</div>
	{/if}
</div>

<style>
	.library-page { max-width: 1440px; margin: 0 auto; padding: 46px 42px 70px; }
	.library-heading { display: flex; align-items: end; justify-content: space-between; gap: 28px; }
	.page-kicker { margin: 0 0 10px; color: var(--accent); font-size: 0.66rem; font-weight: 700; letter-spacing: 0.16em; text-transform: uppercase; }
	h1 { margin: 0; color: var(--text-strong); font-size: clamp(2rem, 4vw, 3.2rem); font-weight: 600; letter-spacing: -0.055em; line-height: 1; }
	.page-description { max-width: 450px; margin: 12px 0 0; color: var(--text-muted); font-size: 0.84rem; }
	.library-heading__meta { display: flex; align-items: center; gap: 8px; color: var(--text-muted); font-size: 0.72rem; white-space: nowrap; }
	.meta-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--success); }
	.library-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 20px; margin-top: 38px; padding: 13px 0; border-top: 1px solid var(--line-subtle); border-bottom: 1px solid var(--line-subtle); }
	.library-search { display: flex; align-items: center; gap: 10px; flex: 1; max-width: 420px; color: var(--text-muted); }
	.library-search input { width: 100%; min-width: 0; padding: 0; border: 0; outline: none; color: var(--text-strong); font-size: 0.82rem; background: transparent; }
	.library-search input::placeholder { color: var(--text-dim); }
	.library-search input::-webkit-search-cancel-button { display: none; }
	.search-clear { display: grid; place-items: center; width: 24px; height: 24px; padding: 0; border: 0; border-radius: 50%; color: var(--text-muted); background: transparent; cursor: pointer; }
	.search-clear:hover { color: var(--text-strong); background: var(--surface-2); }
	.toolbar-controls { display: flex; align-items: center; gap: 18px; }
	.filter-group { display: inline-flex; gap: 2px; padding: 3px; border: 1px solid var(--line-subtle); border-radius: 8px; background: var(--surface-1); }
	.filter-group button { min-height: 28px; padding: 0 9px; border: 0; border-radius: 5px; color: var(--text-muted); font-size: 0.68rem; cursor: pointer; background: transparent; }
	.filter-group button:hover { color: var(--text-soft); }
	.filter-group button.active { color: var(--text-strong); background: var(--surface-3); box-shadow: 0 1px 4px rgba(0,0,0,0.2); }
	.sort-control { display: flex; align-items: center; gap: 8px; color: var(--text-muted); font-size: 0.7rem; }
	.sort-control select { appearance: none; padding: 4px 18px 4px 0; border: 0; outline: none; color: var(--text-soft); font-size: 0.7rem; background: transparent; cursor: pointer; }
	.sort-control :global(svg) { margin-left: -17px; pointer-events: none; }
	.result-line { display: flex; align-items: center; gap: 8px; margin: 21px 0 16px; color: var(--text-soft); font-size: 0.72rem; }
	.result-line__query { color: var(--accent-soft); }
	.result-line__hint { display: inline-flex; align-items: center; gap: 5px; margin-left: auto; color: var(--text-dim); }
	.empty-state { display: grid; justify-items: center; gap: 8px; min-height: 360px; padding-top: 100px; text-align: center; }
	.empty-state__icon { display: grid; place-items: center; width: 48px; height: 48px; margin-bottom: 8px; border: 1px solid var(--line-subtle); border-radius: 50%; color: var(--text-muted); background: var(--surface-1); }
	.empty-state h2 { margin: 0; color: var(--text-strong); font-size: 1.1rem; font-weight: 600; }
	.empty-state p { margin: 0 0 12px; color: var(--text-muted); font-size: 0.78rem; }
	.button { display: inline-flex; align-items: center; justify-content: center; min-height: 38px; padding: 0 15px; border: 1px solid transparent; border-radius: 7px; font-size: 0.76rem; font-weight: 600; cursor: pointer; }
	.button--secondary { border-color: var(--line-subtle); color: var(--text-soft); background: var(--surface-2); }
	.button--secondary:hover { border-color: var(--line-strong); color: var(--text-strong); }
	@media (max-width: 880px) { .library-page { padding-right: 28px; padding-left: 28px; } .library-heading { align-items: start; flex-direction: column; gap: 14px; } .library-toolbar { align-items: stretch; flex-direction: column; } .library-search { max-width: none; } .toolbar-controls { justify-content: space-between; } }
	@media (max-width: 560px) { .library-page { padding: 32px 18px 48px; } .library-heading__meta { display: none; } .toolbar-controls { align-items: stretch; flex-direction: column; gap: 12px; } .filter-group { justify-content: space-between; } .filter-group button { flex: 1; } .sort-control { justify-content: space-between; } .result-line__hint { display: none; } }
</style>
