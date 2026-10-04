<script lang="ts">
	import Icon from './Icon.svelte';
	let trigger: HTMLButtonElement;
	let panel: HTMLDivElement;
	let open = $state(false);
	const uid = $props.id();
	function toggle() {
		if (panel.matches(':popover-open')) { panel.hidePopover(); return; }
		panel.showPopover();
		const anchor = trigger.getBoundingClientRect();
		const bounds = panel.getBoundingClientRect();
		panel.style.left = `${Math.max(12, Math.min(anchor.right - bounds.width, window.innerWidth - bounds.width - 12))}px`;
		panel.style.top = `${Math.max(12, Math.min(anchor.bottom + 8, window.innerHeight - bounds.height - 12))}px`;
	}
</script>

<div class="detail-cast">
	<button bind:this={trigger} class="cast-trigger" type="button" aria-label="Cast to a device" aria-expanded={open} aria-controls={uid} aria-haspopup="dialog" onclick={toggle}><Icon name="cast" size={21} /></button>
	<div bind:this={panel} id={uid} class="cast-panel" popover="auto" role="dialog" aria-label="Cast to a device" ontoggle={() => open = panel.matches(':popover-open')}>
		<div class="cast-heading"><strong>Cast to a device</strong><button type="button" aria-label="Close casting menu" onclick={() => panel.hidePopover()}><Icon name="close" size={18} /></button></div>
		<div class="cast-empty"><Icon name="tv" size={30} /><strong>No devices available</strong><p>Your devices will appear here.</p></div>
		<p class="cast-note">Connect your TV and phone to the same network.</p>
	</div>
</div>

<style>
	.detail-cast { position:fixed; top:58px; right:18px; z-index:70; }
	.cast-trigger { display:grid; place-items:center; width:42px; height:42px; padding:0; border:1px solid rgba(255,255,255,.14); border-radius:50%; background:rgba(12,17,23,.55); color:var(--text-strong); backdrop-filter:blur(12px); cursor:pointer; }
	.cast-trigger:hover, .cast-trigger[aria-expanded='true'] { background:rgba(39,51,65,.8); border-color:rgba(158,198,214,.4); }
	.cast-panel { position:fixed; inset:auto; margin:0; width:288px; max-width:calc(100vw - 24px); padding:16px; border:1px solid var(--line-strong); border-radius:16px; background:rgba(19,27,36,.96); color:var(--text-soft); backdrop-filter:blur(20px); box-shadow:0 16px 40px rgba(0,0,0,.35); }
	.cast-heading { display:flex; align-items:center; justify-content:space-between; gap:12px; font-size:.8rem; }
	.cast-heading button { display:grid; place-items:center; width:36px; height:36px; padding:0; border:0; border-radius:9px; color:var(--text-muted); background:transparent; cursor:pointer; }
	.cast-heading button:hover { background:rgba(158,198,214,.1); }
	.cast-empty { display:grid; justify-items:center; gap:12px; padding:28px 0; color:var(--text-muted); text-align:center; }
	.cast-empty strong { color:var(--text-soft); font-size:.78rem; font-weight:600; }
	.cast-empty p { margin:0; font-size:.7rem; }
	.cast-note { margin:0; padding-top:14px; border-top:1px solid var(--line-subtle); color:var(--text-muted); font-size:.68rem; line-height:1.5; }
	:global(html[data-mobile-preview='true']) .detail-cast { position:fixed; top:max(16px,env(safe-area-inset-top)); z-index:70; }
</style>
