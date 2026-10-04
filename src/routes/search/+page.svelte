<script lang="ts">
	import { onMount } from 'svelte';
	import { replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import Icon from '$lib/components/Icon.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import { isMobilePreview } from '$lib/platform/mobile-preview';
	import { searchTitles } from '$lib/media/search';
	import {readGenreBrowse} from '$lib/media/genre-browse';
	import { nativeAcrylicStatus } from '$lib/platform/native-acrylic';
	import type { MediaItem } from '$lib/types';
	const mobilePreview = isMobilePreview();
	let input = $state<HTMLInputElement>();
	let query = $state(page.url.searchParams.get('q') ?? '');
	let items = $state<MediaItem[]>([]);
	let loading = $state(false);
	let error = $state('');
	let retry = $state(0);
	let genres=$state<{title:string;items:MediaItem[]}[]>([]);
	let genresLoading=$state(false),genresError=$state('');
	async function browse(){
		genresLoading=true;genresError='';
		try{genres=await readGenreBrowse();}catch{genresError='Connect to your computer to browse movies and series.';}
		finally{genresLoading=false;}
	}
	const normalized = $derived(query.trim());
	onMount(() => {
		if(mobilePreview)void browse();else input?.focus({ preventScroll: true });
	});
	$effect(() => {
		const text = normalized;
		void retry;
		let current = true;
		items = []; error = '';
		loading = text.length >= 2;
		const timer = setTimeout(async () => {
			const url = new URL(window.location.href);
			if (text) url.searchParams.set('q', text); else url.searchParams.delete('q');
			replaceState(url, page.state);
			if (text.length < 2) return;
			try {
				const result = await searchTitles(text);
				if (current) { items = result.items; error = result.warning; }
			} catch { if (current) error = 'Search could not be loaded. Try again.'; }
			finally { if (current) loading = false; }
		}, 280);
		return () => { current = false; clearTimeout(timer); };
	});
	function keyboard(event: KeyboardEvent) {
		if (event.key === 'Escape' || event.key === 'Enter') { event.preventDefault(); input?.blur(); }
	}
</script>

	<div class="search-surface" class:search-surface--mobile={mobilePreview} data-native-backdrop={$nativeAcrylicStatus}>
	<section class="search-page" aria-label="Search movies and series">
		<div class="search-field" role="search">
			<div class="search-input">
				<Icon name="search" size={19} />
				<input bind:this={input} bind:value={query} type="search" placeholder="Search movies and series" aria-label="Search movies and series" autocomplete="off" spellcheck="false" aria-controls="search-results" onkeydown={keyboard} />
				{#if query}<button type="button" aria-label="Clear search" onclick={() => { query = ''; input?.focus(); }}><Icon name="close" size={19} /></button>{/if}
			</div>
		</div>
		<div id="search-results" class="search-results" aria-busy={loading}>
		{#if normalized.length >= 2}
			<span class="search-status" role="status">{loading ? 'Searching…' : `${items.length} ${items.length === 1 ? 'title' : 'titles'} found`}</span>
			{#if error}<div class="search-error" role="alert"><span>{error}</span><button type="button" onclick={() => retry++}>Try again</button></div>{/if}
			{#if loading}
				<div class="search-grid" aria-label="Loading search results" aria-busy="true">{#each Array(12) as _}<div class="search-skeleton"><div></div><span></span></div>{/each}</div>
			{:else if items.length}
				<div class="search-grid">{#each items as item (item.id)}<PosterCard media={item} variant="catalog" />{/each}</div>
			{:else if !error}<div class="search-empty"><strong>No titles found</strong><p>Try another title or check the spelling.</p></div>{/if}
		{:else if mobilePreview}
			{#if genresLoading}<div class="search-grid" aria-label="Loading genres">{#each Array(6) as _}<div class="search-skeleton"><div></div><span></span></div>{/each}</div>
			{:else if genresError}<div class="search-error"><span>{genresError}</span><button type="button" onclick={browse}>Try again</button></div>
			{:else}{#each genres as section}<section class="genre-section"><h2>{section.title}</h2><div class="genre-row">{#each section.items as item (item.id)}<PosterCard media={item} variant="catalog" />{/each}</div></section>{/each}{/if}
		{/if}
		</div>
	</section>
	</div>

<style>
	.search-surface { min-height: 100%; background: #101419; }
	:global(html[data-runtime='desktop']) .search-surface { background: var(--acrylic-content-tint); }
	:global(html[data-runtime='desktop']) .search-surface[data-native-backdrop='unavailable'] { background: var(--acrylic-content-fallback); }
	.search-page { max-width: var(--content-width); min-height: 100vh; box-sizing: border-box; margin: 0 auto; padding: 42px var(--content-gutter, 56px) 72px; }
	.search-field { max-width: 440px; }
	.search-input { display: flex; align-items: center; gap: 12px; min-height: 42px; padding: 0 14px; border: 1px solid var(--line-subtle); border-radius: 9px; color: var(--text-muted); background: var(--material-control); transition: border-color 140ms; }
	.search-input:focus-within { border-color: rgba(218,231,241,.55); }
	.search-input input { flex: 1; min-width: 0; height: 40px; padding: 0; border: 0; outline: none; color: var(--text-strong); background: transparent; font: inherit; font-size: .82rem; }
	.search-input input::placeholder { color: var(--text-muted); }
	.search-input input::-webkit-search-cancel-button { display: none; }
	.search-input button { display: grid; place-items: center; width: 32px; height: 32px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--text-muted); cursor: pointer; }
	.search-input button:hover { color: var(--text-strong); background: rgba(255,255,255,.08); }
	.search-results { margin-top: 32px; }
	.search-status { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
	.search-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(145px, 1fr)); gap: 28px 20px; }
	.search-empty { display: grid; justify-items: center; align-content: center; gap: 16px; min-height: 340px; padding: 40px 16px; color: var(--text-muted); text-align: center; }
	.search-empty strong { color: var(--text-soft); font-size: 1.15rem; font-weight: 600; }
	.search-empty p { margin: 0; font-size: .82rem; line-height: 1.6; }
	.search-error { display: flex; align-items: center; gap: 16px; margin-bottom: 20px; color: var(--text-muted); font-size: .8rem; }
	.search-error button { padding: 8px 12px; border: 1px solid var(--line-subtle); border-radius: 7px; color: var(--text-soft); background: var(--surface-1); cursor: pointer; font: inherit; }
	.search-skeleton > div { aspect-ratio: 2/3; border-radius: 12px; background: rgba(168,192,210,.065); }
	.search-skeleton > span { display: block; width: 70%; height: 10px; margin-top: 14px; border-radius: 3px; background: rgba(168,192,210,.065); }
	@media (max-width: 1100px) { .search-page { padding: 42px 32px 72px; } .search-field { max-width: min(440px,calc(100% - 44px)); } .search-grid { gap: 24px 16px; grid-template-columns: repeat(auto-fill,minmax(125px,1fr)); } }
	@media (prefers-reduced-transparency: reduce) { .search-surface, :global(html[data-runtime='desktop']) .search-surface { background: #0c0f13; } }
	.search-surface--mobile .search-page { min-height:calc(100dvh - 88px); padding: max(18px,env(safe-area-inset-top)) 18px 32px; }
	.search-surface--mobile .search-field { max-width:none; }
	.search-surface--mobile .search-input { min-height:46px; }
	.search-surface--mobile .search-input input { height:44px; font-size:.85rem; }
	.search-surface--mobile .search-results { margin-top:24px; }
	.search-surface--mobile .search-grid { grid-template-columns:repeat(3,minmax(0,1fr)); gap:20px 10px; }
	.genre-section{margin-bottom:26px;}.genre-section h2{margin:0 0 14px;font-size:.95rem;font-weight:650;color:var(--text-strong);}
	.genre-row{display:grid;grid-auto-flow:column;grid-auto-columns:32%;gap:12px;overflow-x:auto;padding-bottom:8px;scrollbar-width:none;}.genre-row::-webkit-scrollbar{display:none;}
</style>
