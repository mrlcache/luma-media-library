<script lang="ts">
	import { scale } from 'svelte/transition';
	import Icon from './Icon.svelte';
	let open = $state(false);
	let host = $state<HTMLDivElement>();
	let trigger = $state<HTMLButtonElement>();
	const uid = $props.id();
	$effect(() => {
		if (!open) return;
		const outside = (event: PointerEvent) => { if (!host?.contains(event.target as Node)) open = false; };
		const keyboard = (event: KeyboardEvent) => {
			if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); open = false; trigger?.focus({preventScroll:true}); }
		};
		window.addEventListener('pointerdown', outside, true);
		window.addEventListener('keydown', keyboard, true);
		return () => { window.removeEventListener('pointerdown', outside, true); window.removeEventListener('keydown', keyboard, true); };
	});
</script>

<div class="mobile-profile" bind:this={host}>
	<button bind:this={trigger} class="mobile-profile__trigger" type="button" aria-label="Profile options" aria-expanded={open} aria-controls={uid} onclick={() => open = !open}><Icon name="profile" size={21} /></button>
	{#if open}
		<div id={uid} class="mobile-profile__menu" transition:scale={{start:.96,duration:160}}>
			<a href="/settings" onclick={() => open = false}><Icon name="settings" size={19} /><span>Settings</span><Icon name="chevron-right" size={14} /></a>
		</div>
	{/if}
</div>

<style>
	.mobile-profile { position:fixed; top:max(16px,env(safe-area-inset-top)); right:18px; z-index:75; }
	.mobile-profile__trigger { display:grid; place-items:center; width:42px; height:42px; padding:0; border:1px solid rgba(255,255,255,.14); border-radius:50%; background:rgba(12,17,23,.55); color:var(--text-strong); -webkit-backdrop-filter:blur(12px); backdrop-filter:blur(12px); cursor:pointer; transition:background 140ms,border-color 140ms; }
	.mobile-profile__trigger:hover, .mobile-profile__trigger[aria-expanded='true'] { background:rgba(39,51,65,.8); border-color:rgba(158,198,214,.4); }
	.mobile-profile__menu { position:absolute; top:calc(100% + 10px); right:0; width:196px; padding:6px; border:1px solid var(--line-strong); border-radius:14px; background:rgba(19,27,36,.96); color:var(--text-soft); -webkit-backdrop-filter:blur(20px); backdrop-filter:blur(20px); box-shadow:0 16px 40px rgba(0,0,0,.35); transform-origin:top right; }
	.mobile-profile__menu a { display:flex; align-items:center; gap:12px; min-height:46px; padding:0 12px; border-radius:9px; color:inherit; font-size:.8rem; text-decoration:none; }
	.mobile-profile__menu a span { flex:1; }
	.mobile-profile__menu a:hover { background:rgba(158,198,214,.1); }
	@media (prefers-reduced-motion:reduce) { .mobile-profile__menu { transition:none !important; animation:none !important; } }
</style>
