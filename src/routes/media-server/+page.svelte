<script lang="ts">
	import { onMount } from 'svelte';
	import { nativeAcrylicStatus, requestNativeAcrylic } from '$lib/platform/native-acrylic';
	type ServerStatus = { running: boolean; name: string; url: string; items: number; transcoding: boolean; activeTranscodes: number; error: string | null };
	let status = $state<ServerStatus | null>(null);
	let busy = $state(false);
	let error = $state('');
	let reachable = $state(false);
	let copyFeedback = $state('');
	let copyTimer: ReturnType<typeof setTimeout> | undefined;
	async function refresh() {
		try {
			const reply = await fetch('/__media-server/status');
			if (!reply.ok) throw new Error('The media server could not be reached.');
			const result: ServerStatus = await reply.json();
			if (busy) return;
			status = result; reachable = true; error = result.error ?? '';
		} catch (failure) { reachable = false; error = failure instanceof Error ? failure.message : 'The media server could not be reached.'; }
	}
	async function toggle() {
		if (!status || !reachable || busy) return;
		busy = true;
		try {
			const reply = await fetch(`/__media-server/${status.running ? 'stop' : 'start'}`, { method: 'POST', headers: { 'X-Luma-Control': '1' } });
			const result = await reply.json();
			if (!reply.ok) throw new Error(result.error ?? 'Could not update the server.');
			status = result; error = result.error ?? '';
		} catch (failure) { error = failure instanceof Error ? failure.message : 'Could not update the server.'; }
		finally { busy = false; }
	}
	async function copyAddress() {
		if (!status || !reachable) return;
		try { await navigator.clipboard.writeText(status.url); copyFeedback = 'Copied'; }
		catch { copyFeedback = 'Select the address to copy it.'; }
		clearTimeout(copyTimer);
		copyTimer = setTimeout(() => { copyFeedback = ''; }, 3000);
	}
	onMount(() => {
		const requester = Symbol('Media server surface');
		requestNativeAcrylic(requester, true);
		void refresh();
		const timer = setInterval(() => { if (!busy) void refresh(); }, 3000);
		return () => { clearInterval(timer); clearTimeout(copyTimer); requestNativeAcrylic(requester, false); };
	});
</script>

<svelte:head><title>Media server · Luma</title></svelte:head>
<div class="server-surface" data-native-backdrop={$nativeAcrylicStatus}>
	<div class="server-page">
		<header class="server-heading"><h1>Media server</h1><p class="page-description">Your library, on your home network.</p></header>
		<section aria-labelledby="sharing-heading">
			<div class="section-heading"><h2 id="sharing-heading">Sharing</h2><p>Make your library available to other devices.</p></div>
			<div class="detail-list">
				<div class="detail-row">
					<div class="detail-copy"><strong>Share library</strong><small aria-live="polite">{busy ? 'Updating…' : !reachable ? error ? 'Server unavailable' : 'Connecting…' : status?.running ? `${status.items} media files available` : 'Hidden from other devices'}</small></div>
					<button class="toggle" role="switch" aria-label="Share library" aria-checked={reachable && (status?.running ?? false)} onclick={toggle} disabled={!reachable || busy}></button>
				</div>
				{#if error}<p class="feedback" role="alert">{error}</p>{/if}
			</div>
		</section>
		<section aria-labelledby="connection-heading">
			<div class="section-heading"><h2 id="connection-heading">Connection</h2><p>Find Luma in your device’s media library.</p></div>
			<div class="detail-list">
				<div class="detail-row"><strong>Library name</strong><span class="detail-value selectable">{reachable ? status?.name ?? '—' : '—'}</span></div>
				<div class="detail-row address-row"><strong>Local address</strong><div class="address-value"><span class="detail-value selectable">{reachable ? status?.url ?? '—' : '—'}</span><button class="copy-button" onclick={copyAddress} disabled={!reachable || !status}>Copy</button></div></div>
				{#if copyFeedback}<p class="feedback" role="status">{copyFeedback}</p>{/if}
			</div>
		</section>
		<section aria-labelledby="playback-heading">
			<div class="section-heading"><h2 id="playback-heading">Playback</h2><p>Choose the playback resource supported by your device.</p></div>
			<div class="detail-list">
				<div class="detail-row"><div class="detail-copy"><strong>Original playback</strong><small>Streams the original file without conversion.</small></div><span class="detail-value">Original file</span></div>
				<div class="detail-row"><div class="detail-copy"><strong>Compatibility playback</strong><small>Offers a converted stream for supported files.</small></div><span class="detail-value">{reachable && status ? status.transcoding ? 'Available' : 'Unavailable' : '—'}</span></div>
				{#if reachable && status?.activeTranscodes}<p class="feedback">{status.activeTranscodes} active conversion{status.activeTranscodes === 1 ? '' : 's'}.</p>{/if}
			</div>
		</section>
		<details class="connection-help"><summary>Connect a device</summary><p>Connect your device to the same network as this PC. Open a player that supports UPnP or DLNA and select {status?.name ?? 'Luma'} from its media servers. Keep library sharing on while you watch.</p></details>
	</div>
</div>

<style>
	.server-surface { min-height:100vh; background:#101419; }
	:global(html[data-runtime='desktop']) .server-surface { background:var(--acrylic-content-tint); }
	:global(html[data-runtime='desktop']) .server-surface[data-native-backdrop='unavailable'] { background:var(--acrylic-content-fallback); }
	.server-page { max-width:1080px; min-height:100dvh; margin:0 auto; padding:clamp(48px,7vh,82px) clamp(22px,4vw,62px) 84px; color:var(--text-soft); }
	h1 { margin:0; color:var(--text-strong); font-family:var(--font-display); font-size:clamp(2rem,4vw,3rem); font-weight:600; letter-spacing:-.06em; line-height:1; }
	.page-description { margin:14px 0 32px; color:var(--text-muted); font-size:.78rem; line-height:1.5; }
	section { display:grid; grid-template-columns:minmax(200px,1.05fr) minmax(0,1.95fr); gap:clamp(24px,3vw,44px); padding:28px 0; border-top:1px solid var(--line-subtle); }
	h2 { margin:0; color:var(--text-strong); font-size:.9rem; font-weight:600; letter-spacing:-.025em; }
	.section-heading p { max-width:220px; margin:9px 0 0; color:var(--text-muted); font-size:.72rem; line-height:1.55; }
	.detail-list { min-width:0; }
	.detail-row { display:flex; align-items:center; justify-content:space-between; gap:24px; min-height:74px; padding:16px 0; border-bottom:1px solid var(--line-subtle); }
	.detail-row:first-child { padding-top:0; min-height:58px; }
	.detail-row:last-child { border-bottom:0; }
	.detail-copy { display:grid; gap:6px; min-width:0; }
	.detail-row strong { color:var(--text-soft); font-size:.78rem; font-weight:600; }
	.detail-copy small { color:var(--text-muted); font-size:.68rem; line-height:1.4; }
	.detail-value { color:var(--text-muted); font-size:.7rem; text-align:right; }
	.selectable { overflow-wrap:anywhere; user-select:text; -webkit-user-select:text; }
	.address-value { display:flex; align-items:center; justify-content:flex-end; gap:12px; min-width:0; }
	.copy-button { flex-shrink:0; padding:7px 10px; border:1px solid var(--line-subtle); border-radius:var(--radius-sm); background:transparent; color:var(--text-soft); font:inherit; font-size:.68rem; cursor:pointer; }
	.copy-button:hover:not(:disabled) { border-color:var(--line-strong); background:var(--surface-3); }
	.copy-button:disabled { opacity:.5; cursor:default; }
	.toggle { position:relative; flex:0 0 40px; width:40px; height:34px; padding:0; border:0; background:none; cursor:pointer; }
	.toggle::before { content:''; position:absolute; top:6px; left:2px; width:36px; height:21px; box-sizing:border-box; border:1px solid var(--line-strong); border-radius:999px; background:var(--surface-3); transition:background 160ms ease,border-color 160ms ease; }
	.toggle::after { content:''; position:absolute; top:10px; left:6px; width:13px; height:13px; border-radius:50%; background:var(--text-muted); transition:transform 160ms ease,background 160ms ease; }
	.toggle[aria-checked='true']::before { border-color:var(--accent); background:rgba(158,198,214,.24); }
	.toggle[aria-checked='true']::after { background:var(--accent-soft); transform:translateX(15px); }
	.toggle:disabled { opacity:.5; cursor:default; }
	.toggle:focus-visible,.copy-button:focus-visible,summary:focus-visible { outline:2px solid var(--accent); outline-offset:3px; border-radius:var(--radius-sm); }
	.feedback { margin:12px 0 0; color:var(--text-muted); font-size:.68rem; line-height:1.5; }
	.feedback[role='alert'] { color:var(--danger); }
	.connection-help { border-top:1px solid var(--line-subtle); padding-top:14px; color:var(--text-muted); font-size:.72rem; }
	summary { width:fit-content; padding:10px 0; cursor:pointer; }
	summary:hover { color:var(--text-soft); }
	.connection-help p { max-width:480px; margin:4px 0 0; line-height:1.6; }
	@media(max-width:760px) { section { grid-template-columns:1fr; gap:20px; }.section-heading p { max-width:none; } }
	@media(max-width:480px) { .address-row { align-items:start; flex-direction:column; gap:12px; }.address-value { width:100%; justify-content:space-between; }.detail-row { gap:16px; } }
	:global(html:not([data-mobile-preview='true'])) .server-heading { position:absolute; width:1px; height:1px; overflow:hidden; clip-path:inset(50%); white-space:nowrap; }
	:global(html:not([data-mobile-preview='true'])) .server-heading + section { padding-top:0; border-top:0; }
	@media(prefers-reduced-motion:reduce) { .toggle::before,.toggle::after { transition:none; } }
	:global(html[data-reduce-transparency='true']) .server-surface { background:#0c0f13; }
	@media(prefers-reduced-transparency:reduce) { :global(html[data-runtime='desktop']) .server-surface { background:#0c0f13; }.toggle[aria-checked='true']::before { background:#263740; } }
</style>
