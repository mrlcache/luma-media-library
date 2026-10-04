<script lang="ts">
	import Icon from './Icon.svelte';
	import { favorites, isFavorite, toggleFavorite } from '$lib/media/favorites';
	import type { MediaItem } from '$lib/types';
	let { media, compact = false }: { media: MediaItem; compact?: boolean } = $props();
	let selected = $derived(isFavorite($favorites, media));
	let error = $state('');
	function toggle() {
		try { toggleFavorite(media); error = ''; }
		catch { error = 'Could not save favorites. Please try again.'; }
	}
</script>

<button class="favorite-button" class:favorite-button--compact={compact} class:favorite-button--selected={selected} type="button" aria-label={`${selected ? 'Remove' : 'Add'} ${media.title} ${selected ? 'from' : 'to'} favorites`} aria-pressed={selected}  onclick={toggle}>
	<Icon name="heart" size={compact ? 16 : 18} weight={selected ? 'fill' : 'regular'} />
</button>
{#if error}<span class="favorite-error" role="status">{error}</span>{/if}

<style>
	.favorite-button { display:grid; place-items:center; flex-shrink:0; width:38px; height:38px; padding:0; border:1px solid var(--line-subtle); border-radius:7px; background:transparent; color:var(--text-muted); cursor:pointer; }
	.favorite-button:hover, .favorite-button--selected { color:var(--accent-soft); border-color:var(--line-strong); }
	.favorite-button--compact { width:28px; height:28px; border-color:rgba(255,255,255,.14); border-radius:50%; background:rgba(8,12,17,.7); color:rgba(255,255,255,.85); backdrop-filter:blur(8px); }
	.favorite-button--compact.favorite-button--selected { color:var(--accent-soft); }
	.favorite-error { position:absolute; z-index:5; padding:6px; border-radius:5px; background:#151d24; color:var(--text-soft); font-size:.68rem; }
</style>
