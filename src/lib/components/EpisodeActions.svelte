<script module lang="ts">
	let closeActiveMenu: (() => void) | undefined;
</script>

<script lang="ts">
	import { tick } from 'svelte';
	import Icon from './Icon.svelte';
	let { label, onOtherVersions }: { label: string; onOtherVersions: () => void } = $props();
	let host: HTMLDivElement;
	let trigger: HTMLButtonElement;
	let menu: HTMLDivElement;
	let action: HTMLButtonElement;
	let open = $state(false);
	const uid = $props.id();

	function close() {
		open = false;
		if (menu?.matches(':popover-open')) menu.hidePopover();
		if (closeActiveMenu === close) closeActiveMenu = undefined;
	}

	async function toggle() {
		if (open) { close(); return; }
		closeActiveMenu?.();
		closeActiveMenu = close;
		open = true;
		await tick();
		if (!open || !menu?.isConnected) return;
		menu.showPopover();
		const anchor = trigger.getBoundingClientRect();
		const bounds = menu.getBoundingClientRect();
		menu.style.left = `${Math.max(8, Math.min(anchor.right - bounds.width, window.innerWidth - bounds.width - 8))}px`;
		menu.style.top = `${Math.max(8, anchor.top - bounds.height - 6 >= 8 ? anchor.top - bounds.height - 6 : Math.min(anchor.bottom + 6, window.innerHeight - bounds.height - 8))}px`;
		action.focus({ preventScroll: true });
	}

	function choose() {
		close();
		onOtherVersions();
	}

	function keyboard(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault(); event.stopPropagation(); close(); trigger.focus({ preventScroll: true });
		} else if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
			event.preventDefault(); event.stopPropagation();
			if (!open) void toggle(); else action.focus({ preventScroll: true });
		}
	}

	$effect(() => {
		if (!open) return;
		const outside = (event: PointerEvent) => { if (!host.contains(event.target as Node)) close(); };
		const scroll = (event: Event) => { if (!(event.target instanceof Node) || !menu.contains(event.target)) close(); };
		window.addEventListener('pointerdown', outside, true);
		window.addEventListener('scroll', scroll, true);
		window.addEventListener('resize', close);
		return () => {
			window.removeEventListener('pointerdown', outside, true);
			window.removeEventListener('scroll', scroll, true);
			window.removeEventListener('resize', close);
			close();
		};
	});
</script>

<div bind:this={host} class="episode-actions" class:episode-actions--open={open}>
	<button bind:this={trigger} class="episode-actions__trigger" type="button" aria-label={`Options for ${label}`} aria-haspopup="menu" aria-expanded={open} aria-controls={uid} onclick={toggle} onkeydown={keyboard}><Icon name="more" size={20} /></button>
	<div bind:this={menu} id={uid} class="episode-actions__menu" popover="manual" role="menu" aria-label={`Options for ${label}`} onkeydown={keyboard} tabindex="-1" data-lenis-prevent>
		<button bind:this={action} type="button" role="menuitem" tabindex="-1" onclick={choose}><Icon name="download" size={14} /><span>Other versions</span></button>
	</div>
</div>

<style>
	.episode-actions { position:absolute; z-index:2; right:12px; top:50%; transform:translateY(-50%); opacity:0; pointer-events:none; transition:opacity 140ms ease; }
	:global(.episode-row:hover) .episode-actions, :global(.episode-row:focus-within) .episode-actions, .episode-actions--open { opacity:1; pointer-events:auto; }
	.episode-actions__trigger { display:grid; place-items:center; width:32px; height:32px; padding:0; border:1px solid transparent; border-radius:var(--radius-sm); background:transparent; color:var(--text-muted); cursor:pointer; }
	.episode-actions__trigger:hover, .episode-actions--open .episode-actions__trigger { border-color:var(--line-subtle); background:rgba(158,198,214,.08); color:var(--text-soft); }
	.episode-actions__menu { position:fixed; inset:auto; margin:0; width:176px; max-width:calc(100vw - 16px); padding:5px; border:1px solid var(--line-strong); border-radius:9px; background:#151d24; color:var(--text-soft); box-shadow:0 12px 32px rgba(0,0,0,.35); font-size:.72rem; }
	.episode-actions__menu button { display:flex; align-items:center; gap:10px; width:100%; min-height:34px; padding:7px 10px; border:0; border-radius:5px; background:transparent; color:inherit; font:inherit; text-align:left; cursor:pointer; }
	.episode-actions__menu button:hover, .episode-actions__menu button:focus { background:rgba(158,198,214,.1); }
	@media (hover:none) { .episode-actions { opacity:1; pointer-events:auto; } }
	@media (prefers-reduced-motion:reduce) { .episode-actions { transition:none; } }
</style>
