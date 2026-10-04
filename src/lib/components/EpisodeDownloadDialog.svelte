<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from './Icon.svelte';
	import ReleaseBrowser from './ReleaseBrowser.svelte';
	import type { MediaItem } from '$lib/types';
	let { media, season, episode, title, otherVersions = false, onClose }: { media: MediaItem; season: number; episode: number; title: string; otherVersions?: boolean; onClose: () => void } = $props();
	let dialog: HTMLDialogElement;
	onMount(() => { dialog.showModal(); return () => dialog.close(); });
</script>

<dialog bind:this={dialog} class="episode-download-dialog" aria-labelledby="episode-download-title" onclose={onClose} onclick={(event) => { if (event.target === dialog) onClose(); }} onkeydown={(event) => { if (event.key === 'Escape') onClose(); }}>
	<header><div><p>{media.title} · S{String(season).padStart(2, '0')}E{String(episode).padStart(2, '0')}</p><h2 id="episode-download-title">{title}</h2><span>{otherVersions ? 'Other versions' : 'Download this episode'}</span></div><button type="button" aria-label="Close episode downloads" onclick={onClose}><Icon name="close" size={19} /></button></header>
	<ReleaseBrowser {media} scope={{ type: 'episode', season, episode }} />
</dialog>

<style>
	.episode-download-dialog { width:min(860px,calc(100vw - 48px)); max-height:calc(100dvh - 64px); overflow-y:auto; box-sizing:border-box; padding:28px; border:1px solid var(--line-strong); border-radius:18px; background:#12191f; color:var(--text-soft); box-shadow:0 24px 80px rgba(0,0,0,.4); }
	.episode-download-dialog::backdrop { background:rgba(0,0,0,.55); }
	header { display:flex; align-items:flex-start; justify-content:space-between; gap:20px; margin-bottom:26px; }
	header p { margin:0 0 8px; color:var(--text-muted); font-size:.7rem; }
	header h2 { margin:0 0 9px; font-size:1.3rem; font-weight:650; letter-spacing:-.02em; }
	header span { color:var(--text-muted); font-size:.73rem; }
	header button { display:grid; place-items:center; flex-shrink:0; width:34px; height:34px; padding:0; border:1px solid var(--line-subtle); border-radius:var(--radius-sm); background:transparent; color:var(--text-soft); cursor:pointer; }
	header button:focus-visible { outline:2px solid var(--accent); outline-offset:3px; }
	@media(max-width:600px) { .episode-download-dialog { width:calc(100vw - 24px); padding:20px; } }
</style>
