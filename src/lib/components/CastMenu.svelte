<script lang="ts">
	import Icon from './Icon.svelte';
	import AppSelect from './AppSelect.svelte';
	import {tick} from 'svelte';
	import {invoke} from '$lib/platform/invoke';
	import {readCatalogPage} from '$lib/platform/desktop';
	let {mediaId}: {mediaId?:number} = $props();
	let host:HTMLDivElement;
	let trigger: HTMLButtonElement;
	let panel = $state<HTMLDivElement>(null!);
	let open = $state(false);
	let busy = $state(false);
	let devices = $state<{id:string;name:string}[]>([]);
	let titles = $state<{value:number;label:string}[]>([]);
	let selected = $state(0);
	let message = $state('');
	const uid = $props.id();
	async function toggle() {
		if(open){open=false;return;}
		open=true; busy=true; message=''; selected=mediaId ?? 0;
		await tick();
		const anchor = trigger.getBoundingClientRect();
		const bounds = panel.getBoundingClientRect();
		panel.style.left = `${Math.max(12, Math.min(anchor.right - bounds.width, window.innerWidth - bounds.width - 12))}px`;
		panel.style.top = `${Math.max(12, Math.min(anchor.bottom + 8, window.innerHeight - bounds.height - 12))}px`;
		try {
			const [found, catalog] = await Promise.all([invoke<{id:string;name:string}[]>('discover_cast_devices'), readCatalogPage(0,200)]);
			devices=found; titles=(catalog?.items ?? []).filter(item=>Number(item.id)<1_000_000_000).map(item=>({value:Number(item.id),label:item.title}));
			if(!selected)selected=titles[0]?.value ?? 0;
		} catch(error){message=String(error);}
		finally{busy=false;}
	}
	async function cast(id:string) {
		if(!selected||busy)return;
		busy=true; message='';
		try{await invoke('cast_media',{deviceId:id,mediaId:selected});message='Playing on your device.';}
		catch(error){message=String(error);}
		finally{busy=false;}
	}
</script>

<svelte:window onpointerdown={(event)=>{if(open && host && !host.contains(event.target as Node) && !(event.target as HTMLElement).closest?.('.app-select__menu'))open=false;}} />
<div bind:this={host} class="detail-cast">
	<button bind:this={trigger} class="cast-trigger" type="button" aria-label="Cast to a device" aria-expanded={open} aria-controls={uid} aria-haspopup="dialog" onclick={toggle}><Icon name="cast" size={21} /></button>
	{#if open}<div bind:this={panel} id={uid} class="cast-panel" role="dialog" aria-label="Cast to a device">
		<div class="cast-heading"><strong>Cast to a device</strong><button type="button" aria-label="Close casting menu" onclick={() => open=false}><Icon name="close" size={18} /></button></div>
		{#if !mediaId && titles.length}<AppSelect bind:value={selected} options={titles} label="Media to cast" />{/if}
		{#if devices.length}{#each devices as device}<button class="cast-device" type="button" disabled={busy || !selected} onclick={()=>cast(device.id)}><Icon name="tv" size={20}/><span>{device.name}</span></button>{/each}
		{:else}<div class="cast-empty"><Icon name="tv" size={30} /><strong>{busy ? 'Looking for devices…' : 'No devices available'}</strong><p>Your devices will appear here.</p></div>{/if}
		{#if message}<p class="cast-note" role="status">{message}</p>{/if}
		<p class="cast-note">Connect your TV and phone to the same network.</p>
	</div>{/if}
</div>

<style>
	.cast-device { display:flex; align-items:center; gap:12px; width:100%; margin:8px 0; padding:12px; border:1px solid var(--line-subtle); border-radius:10px; background:transparent; color:var(--text-soft); font:inherit; font-size:.78rem; text-align:left; cursor:pointer; }
	.cast-device:disabled { opacity:.5; }
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
	:global(html[data-mobile-preview='true']) .detail-cast { position:fixed; top:calc(max(16px,env(safe-area-inset-top)) + 52px); z-index:70; }
</style>
