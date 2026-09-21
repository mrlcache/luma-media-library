<script lang="ts">
	import { onMount } from 'svelte';
	import type { Component } from 'svelte';
	import type { MediaItem } from '$lib/types';

	type Props = { media: MediaItem; onClose: () => void };
	let { media, onClose }: Props = $props();
	let PlayerComponent = $state<Component<Props> | null>(null);

	onMount(async () => {
		const module = await import('$lib/components/PlayerOverlay.svelte');
		PlayerComponent = module.default;
	});
</script>

{#if PlayerComponent}
	<PlayerComponent {media} {onClose} />
{:else}
	<div class="player-loading" role="dialog" aria-modal="true" aria-label="Loading player">
		<span class="player-loading__spinner" aria-hidden="true"></span>
		<span>Loading player</span>
	</div>
{/if}

<style>
	.player-loading { position: fixed; inset: 0; z-index: 40; display: grid; place-content: center; gap: 12px; color: var(--text-soft); font-size: 0.76rem; text-align: center; background: rgba(3, 5, 7, 0.94); }
	.player-loading__spinner { width: 20px; height: 20px; margin: 0 auto; border: 2px solid var(--line-strong); border-top-color: var(--accent); border-radius: 50%; animation: player-spin 700ms linear infinite; }
	@keyframes player-spin { to { transform: rotate(360deg); } }
	@media (prefers-reduced-motion: reduce) { .player-loading__spinner { animation: none; } }
</style>
