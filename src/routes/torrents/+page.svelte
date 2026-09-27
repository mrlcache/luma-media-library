<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { nativeAcrylicStatus, requestNativeAcrylic } from '$lib/platform/native-acrylic';
	import ArrowDownIcon from 'phosphor-svelte/lib/ArrowDownIcon';
	import ArrowUpIcon from 'phosphor-svelte/lib/ArrowUpIcon';
	import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
	import { addMagnet, addTorrentFile, chooseTorrentFile, moveTorrentQueue, readTorrentSnapshot, removeTorrent, setTorrentLimits, setTorrentPaused, type TorrentTransfer } from '$lib/platform/desktop';

	let transfers = $state<TorrentTransfer[]>([]);
	let loadError = $state('');
	let busy = $state(false);
	let downloadDirectory = $state('');
	const filters = ['All', 'Downloading', 'Seeding', 'Paused', 'Queued'] as const;
	let activeFilter = $state<(typeof filters)[number]>('All');
	let query = $state('');
	let selectedId = $state('');
	let detailsOpen = $state(false);
	let queueOpen = $state(false);
	let limitsOpen = $state(false);
	let addOpen = $state(false);
	let torrentInput = $state('');
	let downloadLimit = $state('Unlimited');
	let uploadLimit = $state('Unlimited');
	let visibleTransfers = $derived(transfers.filter((transfer) =>
		(activeFilter === 'All' || transfer.status === activeFilter)
		&& transfer.name.toLowerCase().includes(query.trim().toLowerCase())
	));
	let selected = $derived(transfers.find((transfer) => transfer.infoHash === selectedId));
	let activeDownloads = $derived(transfers.filter((transfer) => transfer.status === 'Downloading').length);
	let seedingCount = $derived(transfers.filter((transfer) => transfer.status === 'Seeding').length);
	let totalDown = $derived(transfers.reduce((sum, item) => sum + item.downloadRate, 0));
	let totalUp = $derived(transfers.reduce((sum, item) => sum + item.uploadRate, 0));

	function formatBytes(bytes: number) {
		if (bytes <= 0) return '—';
		const units = ['B', 'KB', 'MB', 'GB', 'TB'];
		const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
		return `${(bytes / 1024 ** index).toFixed(index < 2 ? 0 : 1)} ${units[index]}`;
	}
	function formatRate(bytes: number) { return bytes > 0 ? `${formatBytes(bytes)}/s` : '—'; }
	function formatEta(seconds: number | null) {
		if (seconds === null) return '—';
		if (seconds < 3600) return `${Math.max(1, Math.ceil(seconds / 60))} min`;
		return `${Math.ceil(seconds / 3600)} h`;
	}
	function percent(item: TorrentTransfer) { return Math.round(item.progress * 100); }
	function errorMessage(error: unknown) { return error instanceof Error ? error.message : String(error); }

	async function refresh() {
		try {
			const snapshot = await readTorrentSnapshot();
			transfers = snapshot.transfers.sort((a, b) => a.queuePosition - b.queuePosition);
			downloadDirectory = snapshot.downloadDirectory;
			if (!transfers.some((item) => item.infoHash === selectedId)) { selectedId = ''; detailsOpen = false; }
			loadError = '';
		} catch (error) { loadError = errorMessage(error); }
	}
	async function act(operation: () => Promise<unknown>) {
		if (busy) return;
		busy = true;
		try { await operation(); await refresh(); }
		catch (error) { loadError = errorMessage(error); }
		finally { busy = false; }
	}

	onMount(() => {
		const requester = Symbol('Torrents surface');
		requestNativeAcrylic(requester, true);
		void refresh();
		const timer = window.setInterval(() => { if (!busy) void refresh(); }, 1500);
		return () => { window.clearInterval(timer); requestNativeAcrylic(requester, false); };
	});

	function setStatus(item: TorrentTransfer) {
		void act(() => setTorrentPaused(item.infoHash, item.status !== 'Paused'));
	}

	function setAll(paused: boolean) {
		void act(async () => { for (const item of transfers) await setTorrentPaused(item.infoHash, paused); });
		queueOpen = false;
	}

	function moveSelected(direction: -1 | 1) {
		if (selected) void act(() => moveTorrentQueue(selected.infoHash, direction));
		queueOpen = false;
	}

	function applyLimits() {
		const values: Record<string, number> = { Unlimited: 0, '20 MB/s': 20 * 1024 * 1024, '10 MB/s': 10 * 1024 * 1024, '5 MB/s': 5 * 1024 * 1024, '2 MB/s': 2 * 1024 * 1024, '1 MB/s': 1024 * 1024 };
		void act(() => setTorrentLimits(values[downloadLimit], values[uploadLimit]));
	}

	function addTorrent() {
		const input = torrentInput.trim();
		if (!input) return;
		void act(async () => { selectedId = await addMagnet(input); torrentInput = ''; addOpen = false; });
	}

	async function addFromFile() {
		try {
			const path = await chooseTorrentFile();
			if (path) await act(async () => { selectedId = await addTorrentFile(path); addOpen = false; });
		} catch (error) { loadError = errorMessage(error); }
	}

	function removeSelected() {
		if (!selected) return;
		if (window.confirm(`Remove “${selected.name}” from the queue? Downloaded files will be kept.`))
			void act(() => removeTorrent(selected.infoHash));
	}
</script>

<svelte:head><title>Torrents · Luma</title></svelte:head>

<div class="torrent-surface" data-native-backdrop={$nativeAcrylicStatus}>
	<div class="torrent-page">
		<header class="page-header">
			<h1>Torrents</h1>
			<div class="header-actions">
				<div class="menu-anchor">
					<button class="quiet-button" type="button" aria-expanded={queueOpen} onclick={() => { queueOpen = !queueOpen; limitsOpen = false; }}><Icon name="library" size={17} /> Queue <Icon name="chevron-down" size={13} /></button>
					{#if queueOpen}
						<div class="action-menu" role="group" aria-label="Queue controls">
							<button type="button" onclick={() => setAll(true)}><Icon name="pause" size={16} /> Pause all</button>
							<button type="button" onclick={() => setAll(false)}><Icon name="play" size={16} /> Resume all</button>
							<div class="menu-divider"></div>
							<button type="button" disabled={!selected} onclick={() => moveSelected(-1)}><ArrowUpIcon size={16} /> Move selected up</button>
							<button type="button" disabled={!selected} onclick={() => moveSelected(1)}><ArrowDownIcon size={16} /> Move selected down</button>
						</div>
					{/if}
				</div>
				<div class="menu-anchor">
					<button class="quiet-button" type="button" aria-expanded={limitsOpen} onclick={() => { limitsOpen = !limitsOpen; queueOpen = false; }}><Icon name="settings" size={16} /> Limits</button>
					{#if limitsOpen}
						<div class="action-menu limits-menu" role="group" aria-label="Speed limits">
							<label>Download <select bind:value={downloadLimit} onchange={applyLimits}><option>Unlimited</option><option>20 MB/s</option><option>10 MB/s</option><option>5 MB/s</option></select></label>
							<label>Upload <select bind:value={uploadLimit} onchange={applyLimits}><option>Unlimited</option><option>5 MB/s</option><option>2 MB/s</option><option>1 MB/s</option></select></label>
						</div>
					{/if}
				</div>
				<button class="add-button" type="button" onclick={() => (addOpen = true)}><PlusIcon size={17} weight="bold" /> Add torrent</button>
			</div>
		</header>

		<section class="overview" aria-label="Transfer overview">
			<div class="summary-rate"><ArrowDownIcon size={17} /><span>Download</span><strong>{formatRate(totalDown)}</strong></div>
			<div class="summary-rate"><ArrowUpIcon size={17} /><span>Upload</span><strong>{formatRate(totalUp)}</strong></div>
			<span class="summary-activity">{activeDownloads} downloading <span>·</span> {seedingCount} seeding</span>
		</section>
		{#if loadError}<div class="engine-error" role="alert">Torrent engine: {loadError} <button type="button" onclick={() => void refresh()}>Retry</button></div>{/if}

		<div class="content-grid">
			<section class="transfers-panel" aria-label="Torrents">
				<div class="toolbar">
					<div class="filters" aria-label="Transfer status">
						{#each filters as filter}
							<button class:chosen={activeFilter === filter} type="button" aria-pressed={activeFilter === filter} onclick={() => (activeFilter = filter)}>{filter}</button>
						{/each}
					</div>
					<label class="search"><Icon name="search" size={16} /><input bind:value={query} aria-label="Search transfers" placeholder="Search transfers" /></label>
				</div>
				{#if selected}
					<div class="selection-bar">
						<strong title={selected.name}>{selected.name}</strong>
						<div class="selection-actions">
							<button type="button" disabled={busy} onclick={() => setStatus(selected)}><Icon name={selected.status === 'Paused' ? 'play' : 'pause'} size={15} />{selected.status === 'Paused' ? 'Resume' : 'Pause'}</button>
							<button type="button" aria-expanded={detailsOpen} onclick={() => (detailsOpen = !detailsOpen)}>Details <Icon name="chevron-down" size={13} /></button>
							<button type="button" disabled={busy} onclick={removeSelected}>Remove</button>
							<button type="button" aria-label="Clear selection" onclick={() => { selectedId = ''; detailsOpen = false; }}><Icon name="close" size={15} /></button>
						</div>
					</div>
					{#if detailsOpen}
						<div class="transfer-details" aria-label="Selected torrent details">
							<dl>
								<div><dt>Peers / seeds</dt><dd>{selected.peers} / {selected.seeds}</dd></div>
								<div><dt>Downloaded</dt><dd>{formatBytes(selected.downloadedBytes)}</dd></div>
								<div><dt>Uploaded</dt><dd>{formatBytes(selected.uploadedBytes)}</dd></div>
								<div><dt>Ratio</dt><dd>{selected.downloadedBytes > 0 ? (selected.uploadedBytes / selected.downloadedBytes).toFixed(2) : '—'}</dd></div>
							</dl>
							{#if downloadDirectory}<p title={downloadDirectory}>Save location <span>{downloadDirectory}</span></p>{/if}
						</div>
					{/if}
				{/if}
				<div class="list-scroll">
					<div class="table-head"><span>Name</span><span>Size</span><span>Progress</span><span>Down</span><span>Up</span><span>ETA</span></div>
					{#each visibleTransfers as transfer (transfer.infoHash)}
						<button class="transfer-row" class:selected={selectedId === transfer.infoHash} type="button" onclick={() => (selectedId = transfer.infoHash)} aria-label={`View ${transfer.name} details`}>
							<span class="file-cell"><span class="file-icon"><Icon name="download" size={18} /></span><span class="file-copy"><strong>{transfer.name}</strong><small><span class="status-dot" class:downloading={transfer.status === 'Downloading'} class:seeding={transfer.status === 'Seeding'} class:paused={transfer.status === 'Paused'}></span>{transfer.error || transfer.status}</small></span></span>
							<span class="muted-cell">{formatBytes(transfer.sizeBytes)}</span>
							<span class="progress-cell"><span>{percent(transfer)}%</span><span class="progress-track"><span class:complete={percent(transfer) === 100} style={`width: ${percent(transfer)}%`}></span></span></span>
							<span class="muted-cell speed">{formatRate(transfer.downloadRate)}</span><span class="muted-cell speed">{formatRate(transfer.uploadRate)}</span><span class="muted-cell">{formatEta(transfer.etaSeconds)}</span>
						</button>
					{:else}
						<div class="no-results">
							{#if transfers.length === 0}<Icon name="download" size={25} /><h2>No torrents yet</h2><p>Add a magnet link or a .torrent file to start.</p><button class="quiet-button" type="button" onclick={() => (addOpen = true)}><PlusIcon size={16} /> Add torrent</button>
							{:else}<p>No transfers in this view.</p>{/if}
						</div>
					{/each}
				</div>
			</section>

		</div>
	</div>
</div>

{#if addOpen}
	<div class="modal-backdrop" role="presentation" onclick={(event) => { if (event.target === event.currentTarget) addOpen = false; }}>
		<form class="add-modal" aria-label="Add torrent" onsubmit={(event) => { event.preventDefault(); addTorrent(); }}>
			<header><h2>Add torrent</h2><button type="button" aria-label="Close" onclick={() => (addOpen = false)}><Icon name="close" size={18} /></button></header>
			<label>Magnet link<input bind:value={torrentInput} placeholder="Paste a magnet link" /></label>
			<div class="modal-actions"><button type="button" onclick={() => void addFromFile()}>Choose .torrent file</button><button type="button" onclick={() => (addOpen = false)}>Cancel</button><button type="submit" disabled={!torrentInput.trim() || busy}>Add to queue</button></div>
		</form>
	</div>
{/if}

<style>
	.torrent-surface { min-height: 100vh; min-height: 100dvh; background: #101419; }
	:global(html[data-runtime='desktop']) .torrent-surface { background: var(--acrylic-content-tint); }
	:global(html[data-runtime='desktop'] .torrent-surface[data-native-backdrop='unavailable']) { background: var(--acrylic-content-fallback); }
	.torrent-page { max-width: var(--content-width); min-height: 100vh; min-height: 100dvh; margin: 0 auto; padding: 42px var(--content-gutter) 76px; color: var(--text-soft); }
	.page-header { display: flex; align-items: end; justify-content: space-between; gap: 28px; min-height: 58px; }
	h1 { margin: 0; color: var(--text-strong); font-family: var(--font-display); font-size: clamp(2.35rem, 4vw, 3.7rem); font-weight: 650; letter-spacing: -0.065em; line-height: .98; }
	.header-actions { display: flex; align-items: center; gap: 9px; padding-bottom: 3px; }
	.header-actions button { cursor: pointer; white-space: nowrap; }
	.quiet-button, .add-button { display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-height: 38px; padding: 0 13px; border: 1px solid var(--line-strong); border-radius: 10px; font-size: .71rem; font-weight: 680; }
	.quiet-button { color: var(--text-soft); background: rgba(23, 29, 36, .55); }
	.quiet-button:hover { color: var(--text-strong); background: rgba(54, 65, 76, .6); }
	.add-button { color: #11171c; background: #e8eef1; }
	.add-button:hover { background: #fff; }
	.menu-anchor { position: relative; }
	.action-menu { position: absolute; top: calc(100% + 8px); right: 0; z-index: 20; display: grid; gap: 2px; width: 196px; padding: 7px; border: 1px solid var(--line-strong); border-radius: 12px; background: #1a222a; box-shadow: 0 18px 48px rgba(0,0,0,.42); }
	.action-menu button { display: flex; align-items: center; gap: 9px; min-height: 33px; padding: 0 8px; border: 0; border-radius: 7px; background: transparent; text-align: left; font-size: .7rem; }
	.action-menu button:hover { background: rgba(255,255,255,.08); }
	.action-menu button:disabled { opacity: .35; cursor: default; }
	.menu-divider { height: 1px; margin: 3px 0; background: var(--line-subtle); }
	.limits-menu { width: 220px; padding: 12px; gap: 11px; }
	.limits-menu label { display: grid; gap: 5px; color: var(--text-muted); font-size: .68rem; }
	.limits-menu select { min-height: 32px; padding: 0 8px; border: 1px solid var(--line-strong); border-radius: 7px; color: var(--text-soft); background: #202931; }
	.overview { display: flex; align-items: center; flex-wrap: wrap; gap: 16px 30px; margin: 29px 0 22px; padding: 16px 0; border-top: 1px solid var(--line-subtle); border-bottom: 1px solid var(--line-subtle); }
	.summary-rate { display: flex; align-items: center; gap: 8px; color: var(--text-muted); font-size: .72rem; }
	.summary-rate strong { margin-left: 5px; color: var(--text-strong); font-size: .8rem; font-weight: 650; font-variant-numeric: tabular-nums; }
	.summary-activity { margin-left: auto; color: var(--text-muted); font-size: .68rem; }
	.summary-activity span { margin: 0 8px; color: var(--text-dim); }
	.engine-error { margin: -4px 0 18px; padding: 12px 15px; border: 1px solid rgba(228, 156, 156, .4); border-radius: 10px; color: #ffcbcb; background: rgba(115, 33, 33, .16); font-size: .72rem; }
	.engine-error button { margin-left: 10px; border: 0; color: #fff; background: transparent; text-decoration: underline; cursor: pointer; }
	.content-grid { min-width: 0; }
	.transfers-panel { overflow: hidden; border: 1px solid var(--material-border); border-radius: 17px; corner-shape: squircle; background: linear-gradient(150deg, rgba(30, 39, 48, .67), rgba(15, 21, 28, .74)); box-shadow: inset 0 1px rgba(255,255,255,.035); }
	.toolbar { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 18px 20px; }
	.selection-bar { display: flex; align-items: center; justify-content: space-between; gap: 18px; padding: 11px 20px; border-top: 1px solid var(--line-subtle); background: rgba(158,198,214,.045); }
	.selection-bar > strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: .72rem; font-weight: 600; }
	.selection-actions { display: flex; flex-shrink: 0; gap: 7px; }
	.selection-actions button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 32px; padding: 0 10px; border: 1px solid var(--line-subtle); border-radius: 8px; color: var(--text-soft); background: rgba(255,255,255,.04); font-size: .66rem; cursor: pointer; }
	.selection-actions button:hover { background: rgba(255,255,255,.1); }
	.selection-actions button:disabled { opacity: .4; cursor: default; }
	.transfer-details { padding: 18px 20px; border-top: 1px solid var(--line-subtle); }
	.transfer-details dl { display: grid; grid-template-columns: repeat(4, minmax(0,1fr)); gap: 18px; margin: 0; }
	.transfer-details dt { color: var(--text-muted); font-size: .65rem; }
	.transfer-details dd { margin: 6px 0 0; color: var(--text-strong); font-size: .75rem; font-variant-numeric: tabular-nums; }
	.transfer-details p { margin: 18px 0 0; color: var(--text-muted); font-size: .65rem; overflow-wrap: anywhere; }
	.transfer-details p span { margin-left: 12px; color: var(--text-soft); }
	.filters { display: flex; gap: 3px; overflow-x: auto; }
	.filters button { min-height: 29px; padding: 0 9px; border: 0; border-radius: 7px; color: var(--text-muted); background: transparent; font-size: .65rem; font-weight: 650; cursor: pointer; }
	.filters button.chosen { color: var(--text-strong); background: rgba(230,239,245,.12); }
	.search { display: flex; align-items: center; gap: 7px; width: 175px; min-height: 31px; padding: 0 9px; border: 1px solid var(--line-subtle); border-radius: 8px; color: var(--text-muted); background: rgba(6, 10, 15, .22); }
	.search input { width: 100%; min-width: 0; border: 0; outline: 0; color: var(--text-strong); background: none; font-size: .65rem; }
	.search input::placeholder { color: var(--text-dim); }
	.list-scroll { min-width: 0; overflow-x: auto; }
	.table-head, .transfer-row { display: grid; grid-template-columns: minmax(230px, 2.1fr) minmax(60px,.55fr) minmax(108px,.8fr) minmax(66px,.64fr) minmax(66px,.64fr) minmax(45px,.4fr); align-items: center; gap: 10px; min-width: 670px; }
	.table-head { padding: 10px 20px; border-top: 1px solid var(--line-subtle); border-bottom: 1px solid var(--line-subtle); color: var(--text-dim); background: rgba(6, 10, 15, .16); font-size: .57rem; font-weight: 730; letter-spacing: .075em; text-transform: uppercase; }
	.transfer-row { width: 100%; min-height: 73px; padding: 10px 20px; border: 0; border-bottom: 1px solid var(--line-subtle); color: var(--text-soft); background: transparent; text-align: left; cursor: pointer; }
	.transfer-row:last-child { border-bottom: 0; }
	.transfer-row:hover, .transfer-row.selected { background: rgba(158, 198, 214, .08); }
	.transfer-row.selected { box-shadow: inset 2px 0 var(--accent); }
	.file-cell { display: flex; align-items: center; gap: 10px; min-width: 0; }
	.file-icon { display: grid; flex: none; place-items: center; width: 35px; height: 35px; border: 1px solid rgba(198,221,231,.15); border-radius: 9px; color: #c6d9e2; background: rgba(180,207,219,.08); }
	.file-copy { display: grid; min-width: 0; gap: 3px; }
	.file-copy strong { overflow: hidden; color: var(--text-strong); font-size: .68rem; font-weight: 680; white-space: nowrap; text-overflow: ellipsis; }
	.file-copy small { display: flex; align-items: center; gap: 5px; color: var(--text-muted); font-size: .58rem; }
	.status-dot { display: inline-block; width: 5px; height: 5px; border-radius: 50%; background: #8c9ca6; }
	.status-dot.downloading { background: var(--accent); }
	.status-dot.seeding { background: #a4c9b2; }
	.status-dot.paused { background: #9ca4a9; }
	.muted-cell { color: var(--text-muted); font-size: .65rem; white-space: nowrap; }
	.speed { font-variant-numeric: tabular-nums; }
	.progress-cell { display: grid; gap: 7px; color: var(--text-strong); font-size: .64rem; font-weight: 670; }
	.progress-track { display: block; height: 4px; overflow: hidden; border-radius: 20px; background: rgba(198,216,225,.13); }
	.progress-track > span { display: block; height: 100%; border-radius: inherit; background: linear-gradient(90deg,#86b4c9,#c3e0eb); }
	.progress-track > span.complete { background: #9cbeaa; }
	.no-results { display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 270px; padding: 40px 20px; color: var(--text-muted); font-size: .74rem; text-align: center; }
	.no-results h2 { margin: 15px 0 0; color: var(--text-strong); font-size: 1rem; font-weight: 600; letter-spacing: -.03em; }
	.no-results p { margin: 8px 0 20px; }
	.modal-backdrop { position: fixed; inset: 0; z-index: 100; display: grid; place-items: center; padding: 20px; background: rgba(0, 3, 6, .7); backdrop-filter: blur(10px); }
	.add-modal { width: min(420px, 100%); padding: 22px; border: 1px solid var(--line-strong); border-radius: 18px; background: #1b242d; box-shadow: 0 28px 90px rgba(0,0,0,.5); }
	.add-modal header { display: flex; align-items: center; justify-content: space-between; }
	.add-modal h2 { margin: 0; color: var(--text-strong); font-size: 1.15rem; }
	.add-modal header button { display: grid; place-items: center; width: 30px; height: 30px; border: 0; border-radius: 8px; background: transparent; cursor: pointer; }
	.add-modal label { display: grid; gap: 8px; margin-top: 22px; color: var(--text-muted); font-size: .72rem; }
	.add-modal input { min-height: 40px; padding: 0 11px; border: 1px solid var(--line-strong); border-radius: 9px; color: var(--text-strong); background: #101820; outline: 0; }
	.modal-actions { display: flex; justify-content: end; gap: 8px; margin-top: 20px; }
	.modal-actions button { min-height: 36px; padding: 0 12px; border: 1px solid var(--line-strong); border-radius: 9px; background: rgba(255,255,255,.07); cursor: pointer; font-size: .7rem; }
	.modal-actions button:last-child { color: #11171c; background: #e8eef1; }
	.modal-actions button:disabled { opacity: .4; cursor: default; }
	@media (max-width: 850px) { .page-header { align-items: start; flex-direction: column; gap: 18px; } .header-actions { flex-wrap: wrap; } .overview { margin-top: 24px; } .toolbar { align-items: stretch; flex-direction: column; } .search { width: 100%; } }
	@media (max-width: 560px) { .torrent-page { padding: 32px 18px 48px; } .summary-activity { width: 100%; margin-left: 0; } .selection-bar { align-items: start; flex-direction: column; gap: 10px; } .selection-bar > strong { max-width: 100%; } .selection-actions { flex-wrap: wrap; } .transfer-details dl { grid-template-columns: repeat(2,minmax(0,1fr)); } .filters { max-width: 100%; } }
	@media (prefers-reduced-transparency: reduce) { .torrent-surface { background: #0c0f13; } }
</style>
