<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { nativeMobile, readMobileConnection, saveMobileConnection, testMobileConnection, clearMobileConnection, setTorrentTarget } from '$lib/platform/mobile-connection';
	import { homeSnapshot } from '$lib/media/home-state';
	import { invalidateCatalogPageCache, isDesktopRuntime } from '$lib/platform/desktop';
	import { resetDiscovery } from '$lib/media/discovery';
	type DesktopBridgeInfo = { enabled: boolean; baseUrl: string | null; token: string | null; port: number };
	let url = $state('');
	let port = $state(47631);
	let token = $state('');
	let paired = $state(false);
	let busy = $state(false);
	let message = $state('');
	let desktopInfo = $state<DesktopBridgeInfo | null>(null);
	onMount(() => {
		if (!isDesktopRuntime()) return;
		const refresh = () => { void readMobileConnection().then(value => { url = value.url ?? ''; paired = value.paired; }).catch(error => message = String(error)); };
		if (nativeMobile) { refresh(); window.addEventListener('luma-connection-changed',refresh); }
		else void invoke<DesktopBridgeInfo>('get_mobile_bridge_info').then(value => { desktopInfo = value; port = value.port; }).catch(() => { message = 'Restart the updated Luma desktop to enable mobile connections.'; });
		return () => window.removeEventListener('luma-connection-changed',refresh);
	});
	async function connect() {
		busy = true; message = '';
		try {
			await saveMobileConnection(url.trim(),token.trim());
			await testMobileConnection();
			paired = true; token = '';
			invalidateCatalogPageCache(); resetDiscovery();
			message = 'Connected to your computer.';
			window.dispatchEvent(new Event('luma-connection-changed'));
		} catch (error) { message = String(error); }
		finally { busy = false; }
	}
	async function enable() {
		busy = true; message = '';
		try { const info = await invoke<DesktopBridgeInfo>('enable_mobile_bridge', {enabled:true}); desktopInfo = info; port = info.port; }
		catch (error) { message = String(error); }
		finally { busy = false; }
	}
	async function savePort() {
		if (!Number.isInteger(port) || port < 1024 || port > 65535) { message = 'Choose a port between 1024 and 65535.'; return; }
		busy = true; message = '';
		try {
			const info = await invoke<DesktopBridgeInfo>('set_mobile_bridge_port', {port});
			desktopInfo = info; port = info.port;
			message = info.enabled ? 'Port updated. Reconnect phones using the new address.' : 'Port saved.';
		} catch (error) { message = String(error); }
		finally { busy = false; }
	}
	async function disconnect() {
		busy = true; message = '';
		try {
			await clearMobileConnection();
			paired = false; url = ''; token = ''; setTorrentTarget('phone');
			invalidateCatalogPageCache(); resetDiscovery();
			homeSnapshot.ready = false; homeSnapshot.catalog = []; homeSnapshot.resume = []; homeSnapshot.history = [];
			window.dispatchEvent(new Event('luma-library-changed'));
			window.dispatchEvent(new Event('luma-connection-changed'));
			message = 'Disconnected from your computer.';
		} catch (error) { message = String(error); }
		finally { busy = false; }
	}
</script>

<section class="connection-section" aria-labelledby="mobile-connection-heading">
	<div><h2 id="mobile-connection-heading">{nativeMobile ? 'Computer connection' : 'Mobile connection'}</h2><p>{nativeMobile ? 'Connect to Luma on your computer to access its library and downloads.' : 'Connect your phone while both devices are on the same network.'}</p></div>
	<div class="connection-fields">
		{#if nativeMobile}
			<button type="button" onclick={()=>window.dispatchEvent(new Event('luma-pair-computer'))}>Find and pair a computer</button>
			<label>Computer address<input bind:value={url} type="url" placeholder="http://192.168.1.10:47631" autocomplete="off" /></label>
			<label>Connection key<input bind:value={token} type="password" placeholder={paired ? 'Saved key · leave blank to keep it' : 'Key from your computer'} autocomplete="off" /></label>
			<button type="button" disabled={busy || !url.trim()} onclick={connect}>{busy ? 'Connecting…' : paired ? 'Save and test' : 'Connect'}</button>
			{#if paired}<button class="disconnect-button" type="button" disabled={busy} onclick={disconnect}>Disconnect</button>{/if}
		{:else}
			{#if desktopInfo?.enabled}
				<label>Computer address<input value={desktopInfo.baseUrl ?? ''} readonly /></label>
				<label>Connection key<input value={desktopInfo.token ?? ''} readonly type="password" /></label>
				<button type="button" onclick={() => { if (desktopInfo) void navigator.clipboard.writeText(desktopInfo.token ?? '').then(() => message = 'Key copied.').catch(() => message = 'Select the connection key to copy it.'); }}>Copy connection key</button>
			{/if}
			<label>Computer port<input bind:value={port} type="number" min="1024" max="65535" step="1" /></label>
			<button type="button" disabled={busy} onclick={savePort}>Save port</button>
			{#if !desktopInfo?.enabled}<button type="button" disabled={busy} onclick={enable}>{busy ? 'Starting…' : 'Enable mobile connection'}</button>{/if}
		{/if}
		{#if message}<p class="connection-feedback" role="status">{message}</p>{/if}
	</div>
</section>

<style>
	.connection-section { display:grid; grid-template-columns:minmax(245px,1.05fr) minmax(0,1.95fr); gap:clamp(24px,3.3vw,44px); padding:28px 0; border-top:1px solid var(--line-subtle); }
	h2 { margin:0; color:var(--text-strong); font-size:.91rem; font-weight:650; }
	p { margin:7px 0 0; color:var(--text-muted); font-size:.7rem; line-height:1.5; }
	.connection-fields { display:grid; gap:14px; }
	label { display:grid; gap:7px; color:var(--text-muted); font-size:.7rem; }
	input { min-width:0; width:100%; height:40px; padding:0 12px; border:1px solid var(--line-subtle); border-radius:9px; color:var(--text-soft); background:var(--surface-1); font:inherit; }
	button { justify-self:start; min-height:40px; padding:0 14px; border:1px solid var(--line-subtle); border-radius:9px; color:var(--text-soft); background:var(--surface-2); font:inherit; font-size:.75rem; cursor:pointer; }
	.connection-feedback { overflow-wrap:anywhere; }
	.disconnect-button { color:#ee929a; border-color:rgba(224,91,107,.3); background:rgba(160,43,59,.12); }
	.disconnect-button:hover:not(:disabled) { background:rgba(160,43,59,.22); }
	:global(html[data-mobile-preview='true']) .connection-section { grid-template-columns:1fr; gap:18px; }
</style>

