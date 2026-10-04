<script lang="ts">
	import { dev } from '$app/environment';
	import { goto } from '$app/navigation';
	import PlayerOverlay from '$lib/components/PlayerOverlay.svelte';
	import type { MediaItem } from '$lib/types';
	const media: MediaItem = { id: 'preview', title: 'MobLand', kind: 'series', year: 2025, genres: ['Series'], rating: '8.4', runtime: '52m', synopsis: '', match: '', poster: '', backdrop: '/images/subtitle-preview-mobland.jpg', episodeLabel: 'S01 · E03' };
</script>

{#if dev}
	<div class="preview-background"></div>
	<PlayerOverlay {media} visualPreview onClose={() => goto('/?mobile=1')} />
{:else}
	<p>Not found</p>
{/if}

<style>
	.preview-background { position: fixed; inset: 0; z-index: 79; background: #080d13; }
	:global(.player-overlay--preview) { max-width: 412px; margin: 0 auto; box-shadow: 0 0 60px #0008; }
</style>
