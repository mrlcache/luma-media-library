<script lang="ts">
	import AppSelect from '$lib/components/AppSelect.svelte';
	import Icon from './Icon.svelte';
	import { goto } from '$app/navigation';
	import { isMobilePreview } from '$lib/platform/mobile-preview';
	import { requestRelease } from '$lib/torrents/request';
	import { prepareDownload, takePreparedDownload } from '$lib/torrents/pending-download';
	import { addMagnet, addTorrentData, isDesktopRuntime } from '$lib/platform/desktop';
	import { normalizeRelease, releaseMatches, releaseQuery, sortReleases, type ReleaseScope, type ReleaseSort, type TorrentRelease } from '$lib/torrents/releases';
	import type { MediaItem, TmdbSearchResult } from '$lib/types';

	let { media, scope }: { media: MediaItem; scope: ReleaseScope } = $props();
	let releases = $state<TorrentRelease[]>([]);
	let loading = $state(false);
	let errors = $state<string[]>([]);
	let quality = $state('');
	let codec = $state('');
	let source = $state('');
	let fileType = $state('');
	let sort = $state<ReleaseSort>('seeds');
	let visibleCount = $state(20);
	let busy = $state('');
	let feedback = $state('');
	let added = $state<string[]>([]);
	let generation = 0;
	let activeRequestSignal: AbortSignal | undefined;
	let retry = $state(0);
	let consumedRetry = 0;
	let filtered = $derived(sortReleases(releases.filter((release) => (!quality || release.quality === quality) &&
		(!codec || release.codec === codec) && (!source || release.source === source) && (!fileType || (fileType === 'unknown' ? !release.fileType : release.fileType === fileType))), sort));
	let formats = $derived([...new Set(releases.map((release) => release.fileType).filter(Boolean))].sort());
	const keyOf = (release: TorrentRelease) => release.infoHash.toLowerCase() || `${release.source}:${release.name}`;
	const normalizeTitle = (title: string) => title.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase().replace(/[^\p{L}\p{N}]/gu, '');

	async function request(params: Record<string, string>, signal?: AbortSignal) {
		return requestRelease(params,signal);
	}

	$effect(() => {
		const forceRefresh = retry !== consumedRetry;
		consumedRetry = retry;
		const currentMedia = media;
		const currentScope = scope;
		const current = ++generation;
		const controller = new AbortController();
		activeRequestSignal = controller.signal;
		releases = []; errors = []; feedback = ''; visibleCount = 20; loading = true;
		void search(currentMedia, currentScope, current, controller.signal, forceRefresh);
		return () => { controller.abort(); generation++; };
	});

	async function search(item: MediaItem, target: ReleaseScope, current: number, signal: AbortSignal, forceRefresh = false) {
		try {
			if (import.meta.env.VITE_LUMA_MOBILE_DEMO === 'true') {
				const { demoReleases } = await import('$lib/platform/mobile-demo');
				if (!signal.aborted && current === generation) releases = demoReleases(item.title, target);
				return;
			}
			let id = item.tmdbId;
			if (!id) {
				const data = await request({ action: 'titles', query: item.title }, signal);
				const matches = (data.titles as TmdbSearchResult[]).filter((title) => title.kind === item.kind && normalizeTitle(title.title) === normalizeTitle(item.title) && (!item.year || title.year === item.year));
				if (matches.length !== 1) throw new Error('Could not identify this title for release search.');
				id = matches[0].id;
			}
			const sources = item.kind === 'movie' ? ['1337x', 'YTS'] : ['1337x', 'EZTV'];
			await Promise.all(sources.map(async (provider) => {
				try {
					let page: number | null = 1;
					const seenPages = new Set<number>();
					while (page && !signal.aborted) {
						if (seenPages.has(page)) break;
						seenPages.add(page);
						const data = await request({ action: 'source', source: provider, id: String(id), kind: item.kind,
							query: releaseQuery(item.title, item.year, target), page: String(page), ...(forceRefresh ? { refresh: '1' } : {}) }, signal);
						if (signal.aborted || current !== generation) return;
						const matches = (data.results ?? []).map(normalizeRelease).filter((release: TorrentRelease) => releaseMatches(release, item, target));
						const merged = new Map(releases.map((release) => [keyOf(release), release]));
						for (const release of matches) {
							const prior = merged.get(keyOf(release));
							if (!prior || (release.qxr && !prior.qxr) || (!prior.downloadKey && release.downloadKey)) merged.set(keyOf(release), release);
						}
						releases = [...merged.values()];
						page = Number.isSafeInteger(data.nextPage) ? data.nextPage : null;
					}
				} catch (error) {
					if (!signal.aborted && current === generation) errors = [...errors, `${provider}: ${error instanceof Error ? error.message : 'Source unavailable.'}`];
				}
			}));
		} catch (error) {
			if (!signal.aborted && current === generation) errors = [error instanceof Error ? error.message : 'Search unavailable.'];
		} finally { if (!signal.aborted && current === generation) loading = false; }
	}

	function canDownload(release: TorrentRelease) {
		return !!release.magnet || /^(?:[a-f0-9]{40}|[a-z2-7]{32}|[a-f0-9]{64})$/i.test(release.infoHash) || !!release.downloadKey;
	}

	async function download(release: TorrentRelease) {
		if (busy) return;
		const current = generation;
		const key = keyOf(release);
		busy = key; feedback = '';
		try {
			if (isMobilePreview()) {
				prepareDownload(release);
				try { await goto('/torrents'); }
				catch (error) { takePreparedDownload(); throw error; }
				return;
			}
			if (!isDesktopRuntime()) throw new Error('Downloads are available in Luma desktop.');
			const hash = release.infoHash;
			if (release.magnet) {
				await addMagnet(release.magnet);
			} else if (/^(?:[a-f0-9]{40}|[a-z2-7]{32}|[a-f0-9]{64})$/i.test(hash)) {
				await addMagnet(`magnet:?xt=urn:${hash.length === 64 ? 'btmh:1220' : 'btih:'}${hash}&dn=${encodeURIComponent(release.name)}`);
			} else if (release.downloadKey) {
				const result = await request({ action: 'resolve', key: release.downloadKey }, activeRequestSignal);
				// Do not start a download if the user closed the picker or changed the target.
				if (current !== generation) return;
				if (result.magnet) await addMagnet(result.magnet);
				else if (result.torrent) await addTorrentData(result.torrent);
				else throw new Error('No download is available for this release.');
			}
			if (current === generation) { added = [...added, key]; feedback = 'Added to Torrents.'; }
		} catch (error) {
			if (current === generation) feedback = error instanceof Error ? error.message : typeof error === 'string' ? error : 'Could not add the torrent.';
		} finally { if (busy === key) busy = ''; }
	}

	function formatSize(release: TorrentRelease) {
		if (!release.sizeBytes) return release.size || 'Size unknown';
		return release.sizeBytes >= 1024 ** 3 ? `${(release.sizeBytes / 1024 ** 3).toFixed(1)} GB` : `${Math.round(release.sizeBytes / 1024 ** 2)} MB`;
	}
</script>

<div class="release-browser" aria-busy={loading}>
	<div class="release-filters">
		<div>Quality<AppSelect bind:value={quality} label="Quality" options={[{value:"",label:"All qualities"},{value:"2160p",label:"2160p"},{value:"1080p",label:"1080p"},{value:"720p",label:"720p"},{value:"480p",label:"480p"}]} /></div>
		<div>Codec<AppSelect bind:value={codec} label="Codec" options={[{value:"",label:"All codecs"},{value:"HEVC",label:"HEVC"},{value:"H.264",label:"H.264"},{value:"AV1",label:"AV1"}]} /></div>
		<div>Source<AppSelect bind:value={source} label="Source" options={[{value:'',label:'All sources'},{value:'1337x',label:'1337x'},... (media.kind === 'movie' ? [{value:'YTS',label:'YTS'}] : [{value:'EZTV',label:'EZTV'}])]} /></div>
		<div>File type<AppSelect bind:value={fileType} label="File type" options={[{value:'',label:'All formats'},...formats.map(format => ({value:format,label:format})),{value:'unknown',label:'Unknown'}]} /></div>
		<div>Sort by<AppSelect bind:value={sort} label="Sort by" options={[{value:"seeds",label:"Seeders"},{value:"size",label:"Size"},{value:"name",label:"Name"}]} /></div>
	</div>
	<div class="release-summary"><span>{filtered.length} {filtered.length === 1 ? 'release' : 'releases'}{loading ? ' · Searching…' : ''}</span><button class="text-button" onclick={() => retry++} disabled={loading || !!busy} aria-label="Refresh releases"><Icon name="refresh" size={14} />Refresh</button></div>
	<div class="release-list">
		{#each filtered.slice(0, visibleCount) as release (keyOf(release))}
			{@const key = keyOf(release)}
			<article class="release-row">
				<div class="release-copy"><h3>{release.name}</h3><p>{[release.source, release.uploader || release.group, release.quality, release.codec, release.fileType, formatSize(release)].filter(Boolean).join(' · ')}</p></div>
				<span class="release-seeds" >{release.seeds}<small>seeds</small></span>
				<button class="release-download" type="button" disabled={!!busy || added.includes(key) || !canDownload(release)} onclick={() => download(release)} aria-label={added.includes(key) ? `Added ${release.name}` : `Download ${release.name}`} ><Icon name={added.includes(key) ? 'check' : 'download'} size={17} />{#if busy === key}<span>Adding…</span>{/if}</button>
			</article>
		{/each}
		{#if !filtered.length}<p class="release-empty" role="status">{loading ? 'Finding releases…' : releases.length ? 'No releases match these filters.' : scope.type === 'season' ? 'No full-season releases found.' : 'No releases found.'}</p>{/if}
	</div>
	{#if filtered.length > visibleCount}<button class="more-button" onclick={() => visibleCount += 20}>Show more</button>{/if}
	{#if feedback}<p class="release-feedback" role="status">{feedback}</p>{/if}
	{#each errors as error}<p class="release-feedback" role="status">{error}</p>{/each}
</div>

<style>
	.release-filters { display:grid; grid-template-columns:repeat(5,minmax(0,1fr)); gap:12px; }
	.release-filters > div { display:grid; gap:7px; min-width:0; color:var(--text-muted); font-size:.65rem; }
	.release-summary { display:flex; align-items:center; justify-content:space-between; gap:16px; margin:22px 0 10px; color:var(--text-muted); font-size:.68rem; }
	.text-button { display:inline-flex; align-items:center; gap:6px; min-height:34px; padding:0; border:0; background:transparent; color:var(--text-soft); font:inherit; cursor:pointer; }
	.release-list { border-top:1px solid var(--line-subtle); }
	.release-row { display:grid; grid-template-columns:minmax(0,1fr) 55px auto; align-items:center; gap:20px; padding:17px 0; border-bottom:1px solid var(--line-subtle); }
	.release-copy { min-width:0; }
	.release-copy h3 { margin:0; color:var(--text-soft); font-size:.78rem; font-weight:600; overflow-wrap:anywhere; }
	.release-copy p { margin:7px 0 0; color:var(--text-muted); font-size:.66rem; line-height:1.5; }
	.release-seeds { text-align:right; color:var(--text-soft); font-size:.75rem; font-variant-numeric:tabular-nums; }
	.release-seeds small { display:block; margin-top:4px; color:var(--text-muted); font-size:.6rem; }
	.release-download { display:flex; align-items:center; justify-content:center; gap:6px; min-width:36px; min-height:36px; padding:8px; border:1px solid var(--line-subtle); border-radius:var(--radius-sm); background:transparent; color:var(--text-soft); font:inherit; font-size:.65rem; cursor:pointer; }
	.release-download:hover { background:var(--surface-2); }
	button:disabled { opacity:.45; cursor:default; }
	.release-empty, .release-feedback { margin:16px 0; color:var(--text-muted); font-size:.72rem; line-height:1.6; overflow-wrap:anywhere; }
	.more-button { display:block; margin:16px auto 0; padding:9px 16px; border:1px solid var(--line-subtle); border-radius:var(--radius-sm); background:transparent; color:var(--text-soft); font:inherit; font-size:.72rem; cursor:pointer; }
	@media(max-width:720px) { .release-filters { grid-template-columns:repeat(2,minmax(0,1fr)); } .release-row { gap:12px; } }
</style>
