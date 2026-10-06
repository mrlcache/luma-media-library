<script lang="ts">
	import AppSelect from '$lib/components/AppSelect.svelte';
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import EpisodePreview from '$lib/components/EpisodePreview.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import ReleaseBrowser from '$lib/components/ReleaseBrowser.svelte';
	import EpisodeDownloadDialog from '$lib/components/EpisodeDownloadDialog.svelte';
	import EpisodeActions from '$lib/components/EpisodeActions.svelte';
	import FavoriteButton from '$lib/components/FavoriteButton.svelte';
	import { isMobilePreview } from '$lib/platform/mobile-preview';
	import { cachedDiscovery, discoveryMedia, readDiscoveryLogo, readDiscoveryTrailer } from '$lib/media/discovery';
	import { recoverRemoteArtwork } from '$lib/media/artwork';
	import { readTvMazeEpisodes, type TvMazeEpisodeDetails } from '$lib/media/tvmaze-episodes';
	import { mergeSeriesEpisodes, type SeriesEpisode } from '$lib/media/series-episodes';
	import { usePlayer } from '$lib/player-context';
	import { readTitleLogo, readTitleTrailer, readLocalTitleDetail } from '$lib/platform/desktop';
	import { nativeAcrylicStatus, requestNativeAcrylic } from '$lib/platform/native-acrylic';
	import type { LocalTitleDetail, TmdbTrailer } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const mobilePreview = isMobilePreview();
	const player = usePlayer();
	let selectedSeason = $state(1);
	let otherVersionsOpen = $state(false);
	let extrasOpen = $state(false);
	let extras = $derived(data.localDetail?.extras ?? []);
	let episodeDownload = $state<SeriesEpisode | null>(null);
	let trailer = $state<TmdbTrailer | null>(null);
	let trailerOpen = $state(false);
	let titleLogo = $state<string | null>(null);
	let titleLogoResolved = $state(false);
	let titleLogoReady = $state(false);
	let episodeMetadata = $state<TvMazeEpisodeDetails[]>([]);
	let episodesLoading = $state(false);
	let watchedBefore = $state<LocalTitleDetail['watchedBefore']>(null);
	let localEpisodes = $derived([...(data.localDetail?.files ?? [])].sort((a, b) =>
		(a.season ?? 1) - (b.season ?? 1) || (a.episode ?? Number.MAX_SAFE_INTEGER) - (b.episode ?? Number.MAX_SAFE_INTEGER) || a.fileName.localeCompare(b.fileName)
	));
	let episodes = $derived(mergeSeriesEpisodes(localEpisodes, episodeMetadata, watchedBefore));
	let seasons = $derived([...new Set(episodes.map((episode) => episode.season))].sort((a, b) => a - b));
	let visibleEpisodes = $derived(episodes.filter((episode) => episode.season === selectedSeason));
	let hasDownloadedScope = $derived(data.item.kind === 'movie' ? localEpisodes.length > 0 : visibleEpisodes.some((episode) => !!episode.file));
	const discoveryFeed = cachedDiscovery();
	let related = $derived((discoveryFeed?.sections.flatMap((section) => section.items) ?? []).filter((item) => `tmdb-${item.kind}-${item.id}` !== data.item.id && item.kind === data.item.kind).slice(0, 4).map(discoveryMedia));

	function localEpisodeLabel(file: (typeof localEpisodes)[number], index: number) {
		const season = String(file.season ?? 1).padStart(2, '0');
		const number = String(file.episode ?? index + 1).padStart(2, '0');
		return `S${season}E${number}`;
	}

	function localEpisodeTitle(file: (typeof localEpisodes)[number], index: number) {
		return episodeMetadata.find((item) => item.season === file.season && item.episode === file.episode)?.title ?? `Episode ${file.episode ?? index + 1}`;
	}

	function playEpisode(mediaId?: number) {
		if (data.transfer || data.item.installed === false) return;
		if (mediaId === undefined || !data.localDetail) { player.open(data.item); return; }
		const index = localEpisodes.findIndex((file) => file.mediaId === mediaId);
		if (index < 0) return;
		const queue = localEpisodes.slice(index).map((file, offset) => {
			const queueIndex = index + offset;
			return {
				...data.item,
				id: String(file.mediaId),
				playbackUuid: file.playbackUuid,
				episodeLabel: `${localEpisodeLabel(file, queueIndex)} · ${localEpisodeTitle(file, queueIndex)}`
			};
		});
		for (let queueIndex = 0; queueIndex < queue.length - 1; queueIndex += 1) queue[queueIndex].nextEpisode = queue[queueIndex + 1];
		if (queue[0]) player.open(queue[0]);
	}

	function playExtra(file: (typeof extras)[number]) {
		player.open({ ...data.item, id:String(file.mediaId), playbackUuid:file.playbackUuid, title:file.fileName.replace(/[._]/g,' '), kind:'movie', tmdbId:undefined, episodeLabel:undefined, nextEpisode:undefined, progress:undefined, installed:true });
	}

	onMount(() => {
		let disposed = false;
		const updateWatched = () => {
			const id = data.localDetail?.media.id;
			if (!id) return;
			void readLocalTitleDetail(id, true).then((detail) => {
				if (!disposed && data.localDetail?.media.id === id) watchedBefore = detail?.watchedBefore ?? null;
			}).catch((error) => console.warn('Series watch status unavailable', error));
		};
		window.addEventListener('media-library:playback-history-updated', updateWatched);
		const acrylicRequester = Symbol('Details surface');
		requestNativeAcrylic(acrylicRequester, true);
		return () => {
			disposed = true;
			window.removeEventListener('media-library:playback-history-updated', updateWatched);
			requestNativeAcrylic(acrylicRequester, false);
		};
	});

	$effect(() => {
		const id = data.localDetail?.media.id;
		let cancelled = false;
		trailer = null;
		trailerOpen = false;
		if (id) {
			void readTitleTrailer(id).then((result) => {
				if (!cancelled) trailer = result;
			}).catch(() => { if (!cancelled) trailer = null; });
		} else if (data.item.tmdbId) {
			void readDiscoveryTrailer(data.item).then((result) => { if (!cancelled) trailer = result; }).catch(() => {});
		}
		return () => { cancelled = true; };
	});

	$effect(() => {
		const id = data.localDetail?.media.id;
		let cancelled = false;
		titleLogo = null;
		titleLogoReady = false;
		titleLogoResolved = !id;
		if (id) {
			void readTitleLogo(id).then((url) => {
				if (cancelled) return;
				titleLogo = url;
				titleLogoResolved = true;
			}).catch(() => {
				if (!cancelled) titleLogoResolved = true;
			});
		} else if (data.item.tmdbId) {
			void readDiscoveryLogo(data.item).then((url) => { if (!cancelled) titleLogo = url; }).catch(() => {});
		}
		return () => { cancelled = true; };
	});

	$effect(() => {
		const title = data.item.title;
		const year = data.item.year;
		watchedBefore = data.localDetail?.watchedBefore ?? null;
		selectedSeason = data.localDetail?.files.find((file) => file.season !== null)?.season ?? 1;
		episodeMetadata = [];
		if (data.item.kind !== 'series') { episodesLoading = false; return; }
		let cancelled = false;
		episodesLoading = true;
		void readTvMazeEpisodes(title, year || null).then((items) => {
			if (!cancelled) episodeMetadata = items;
		}).finally(() => { if (!cancelled) episodesLoading = false; });
		return () => { cancelled = true; };
	});

	$effect(() => {
		data.item.id;
		selectedSeason;
		otherVersionsOpen = false;
		episodeDownload = null;
	});

	$effect(() => {
		if (seasons.length && !seasons.includes(selectedSeason)) selectedSeason = seasons[0];
	});
</script>

<svelte:head><title>{data.item.title} · Luma</title></svelte:head>
<svelte:window onkeydown={(event) => { if (event.key === 'Escape') trailerOpen = false; }} />


<div class="detail-surface" data-native-backdrop={$nativeAcrylicStatus}>
	<div class="detail-page">
	{#if !mobilePreview}<a class="back-link" href={data.item.kind === 'movie' ? '/library?type=movie' : '/library?type=series'}><Icon name="arrow-left" size={15} />Back to {data.item.kind === 'movie' ? 'movies' : 'series'}</a>{/if}

	<section class="detail-hero">
		{#if mobilePreview}<a class="detail-back" href="/library" aria-label="Go back" onclick={(event) => { if (window.history.length > 1) { event.preventDefault(); window.history.back(); } }}><Icon name="arrow-left" size={20} /></a>{/if}
		<div class="detail-hero__backdrop" aria-hidden="true">{#if data.item.backdrop || data.item.poster}<img use:recoverRemoteArtwork src={data.item.backdrop || data.item.poster} alt="" loading="eager" decoding="async" fetchpriority="high" />{/if}</div>
		<div class="detail-hero__veil"></div>
		<div class="detail-hero__content">
			{#if !mobilePreview}<div class="detail-poster">{#if data.item.poster}<img use:recoverRemoteArtwork src={data.item.poster} alt={`Poster for ${data.item.title}`} width="520" height="780" />{/if}</div>{/if}
			<div class="detail-copy">
				<h1 class="detail-title" aria-label={data.item.title}>
					{#if titleLogo}
						{#if !titleLogoReady}<span>{data.item.title}</span>{/if}
						<img class="detail-title__logo" class:detail-title__logo--ready={titleLogoReady} src={titleLogo} alt="" loading="eager" decoding="async" fetchpriority="high" onload={() => { titleLogoReady = true; }} onerror={() => { titleLogo = null; titleLogoResolved = true; }} />
					{:else if titleLogoResolved}
						{data.item.title}
					{:else}
						<span>{data.item.title}</span>
					{/if}
				</h1>
				<div class="detail-meta">
					{#if data.localDetail}
						<span>{data.localDetail.files.length} {data.item.kind === 'series' ? 'episodes' : 'local file'}</span>
					{:else if data.item.kind === 'series' && episodes.length}
						<span>{episodes.length} episodes</span>
					{:else if data.item.kind === 'movie' && data.item.runtime}
						<span>{data.item.runtime}</span>
					{/if}
					{#if data.item.rating}<span>{data.item.rating} rating</span>{/if}
					{#if data.item.year}<span>{data.item.year}</span>{/if}
					<span>{data.item.kind === 'series' ? 'Series' : 'Movie'}</span>
				</div>
				<p class="detail-synopsis">{data.item.synopsis}</p>
				<div class="detail-actions">
					{#if data.item.installed !== false}<button class="button button--primary" disabled={!!data.transfer} onclick={() => playEpisode(localEpisodes[0]?.mediaId)}><Icon name="play" size={15} />{data.item.progress ? 'Resume' : 'Play now'}</button>{/if}
				{#if trailer}<button class="button button--ghost" onclick={() => { trailerOpen = true; }}><Icon name="play" size={15} />{trailer.isTeaser ? 'Teaser' : 'Trailer'}</button>{/if}
					<FavoriteButton media={data.item} />
				</div>
			</div>
		</div>
	</section>

		<section id="downloads" class="downloads-section" class:downloads-section--optional={hasDownloadedScope || mobilePreview} class:downloads-section--collapsed={(hasDownloadedScope || mobilePreview) && !otherVersionsOpen} aria-label={hasDownloadedScope ? 'Other versions' : 'Downloads'}>
			{#if hasDownloadedScope || mobilePreview}
				<button class="other-versions-toggle" type="button" aria-expanded={otherVersionsOpen} aria-controls="download-versions" onclick={() => otherVersionsOpen = !otherVersionsOpen}>{hasDownloadedScope ? 'Other versions' : 'Downloads'}<Icon name={otherVersionsOpen ? 'chevron-down' : 'chevron-right'} size={13} /></button>
			{/if}
			<div id="download-versions">
				{#if (!hasDownloadedScope && !mobilePreview) || otherVersionsOpen}
					<div class="section-heading"><h2>{data.item.kind === 'series' ? 'Season downloads' : 'Downloads'}</h2>
						{#if data.item.kind === 'series'}<div class="season-select"><span class="sr-only">Season to download</span><AppSelect bind:value={selectedSeason} label="Season" variant="plain" options={(seasons.length ? seasons : [1]).map(season => ({value:season,label:season === 0 ? 'Specials' : `Season ${season}`}))} /></div>{/if}
					</div>
					<ReleaseBrowser media={data.item} scope={data.item.kind === 'series' ? { type: 'season', season: selectedSeason } : { type: 'movie' }} />
				{/if}
			</div>
		</section>

	{#if data.item.kind === 'series'}
		<section class="episodes-section" aria-labelledby="episodes-heading">
			<div class="section-heading">
				<h2 id="episodes-heading">Episodes</h2>
				{#if seasons.length}<div class="season-select"><span class="sr-only">Season</span><AppSelect bind:value={selectedSeason} label="Season" variant="plain" options={(seasons.length ? seasons : [1]).map(season => ({value:season,label:season === 0 ? 'Specials' : `Season ${season}`}))} /></div>{/if}
			</div>
			<div class="episode-list">
				{#each visibleEpisodes as episode (episode.id)}
					{@const label = episode.episode === null ? 'Episode' : `S${String(episode.season).padStart(2, '0')}E${String(episode.episode).padStart(2, '0')}`}
					<article class="episode-row" class:episode-row--unavailable={!episode.file || !!data.transfer}>
						<div class="episode-art">
							{#if episode.image}<div class="episode-thumb"><img use:recoverRemoteArtwork src={episode.image} alt="" width="520" height="292" loading="lazy" /></div>
							{:else if episode.file}<EpisodePreview path={episode.file.path} label={`${label} ${episode.title}`} showTitle={data.item.title} showYear={data.item.year} season={episode.file.season} episode={episode.file.episode} />
							{:else}<div class="episode-thumb episode-thumb--empty"><Icon name="tv" size={22} /></div>{/if}
						</div>
						<div class="episode-copy"><div class="episode-title"><span class="episode-number">{label}</span><h3>{episode.title}</h3>{#if episode.runtime}<span>{episode.runtime}m</span>{/if}</div>
							<p>{episode.summary ?? 'Episode summary unavailable.'}</p>
							{#if !episode.file || data.transfer}<span class="episode-status"><Icon name="download" size={12} />Not downloaded</span>{/if}
							{#if episode.watched}<span class="episode-status"><Icon name="check" size={12} />Watched</span>{/if}
						</div>
						{#if episode.file && !data.transfer}<button class="episode-select" type="button" aria-label={`Play ${label}, ${episode.title}`} onclick={() => playEpisode(episode.file!.mediaId)}></button>
						{:else if episode.episode !== null}<button class="episode-select" type="button" aria-label={`Download ${label}, ${episode.title}`} onclick={() => episodeDownload = episode}></button>{/if}
						{#if episode.episode !== null}
							<EpisodeActions label={`${label}, ${episode.title}`} onOtherVersions={() => episodeDownload = episode} />
						{/if}
					</article>
				{/each}
				{#if !visibleEpisodes.length}<p class="episode-feedback" role="status">{episodesLoading ? 'Loading episodes…' : 'Episode information is unavailable.'}</p>{/if}
			</div>
				{#if episodeMetadata.length}<p class="episode-credit">Episode data: <a href="https://www.tvmaze.com" target="_blank" rel="noreferrer">TVmaze</a></p>{/if}
		</section>
	{/if}

	{#if extras.length}
		<section class="extras-section" aria-label="Extras">
			<button class="extras-toggle" type="button" aria-expanded={extrasOpen} onclick={() => extrasOpen = !extrasOpen}>Extras<Icon name={extrasOpen ? 'chevron-down' : 'chevron-right'} size={15}/></button>
			{#if extrasOpen}<div class="extras-list">{#each extras as file (file.mediaId)}<button type="button" class="extras-file" onclick={() => playExtra(file)}><Icon name="film" size={18}/><span>{file.fileName.replace(/[._]/g,' ')}</span><Icon name="play" size={16}/></button>{/each}</div>{/if}
		</section>
	{/if}
	{#if related.length}<section class="related-section" aria-labelledby="related-heading">
		<div class="section-heading"><h2 id="related-heading">More like this</h2></div>
		<div class="related-grid">{#each related as item (item.id)}<PosterCard media={item} variant="catalog" />{/each}</div>
	</section>{/if}
	</div>
</div>

{#if episodeDownload && episodeDownload.episode !== null}
	{@const selection = episodeDownload}
	<EpisodeDownloadDialog media={data.item} season={selection.season} episode={selection.episode!} title={selection.title} otherVersions={!!selection.file} onClose={() => { if (episodeDownload === selection) episodeDownload = null; }} />
{/if}

{#if trailerOpen && trailer}
	<div class="trailer-modal" role="dialog" aria-modal="true" aria-label={`${data.item.title} trailer`}>
		<button class="trailer-modal__dismiss" aria-label="Close trailer" onclick={() => { trailerOpen = false; }}></button>
		<div class="trailer-modal__content">
			<button class="trailer-modal__close" aria-label="Close trailer" onclick={() => { trailerOpen = false; }}><Icon name="close" size={19} /></button>
			<iframe src={`https://www.youtube-nocookie.com/embed/${trailer.key}?autoplay=1&rel=0`} title={trailer.name} allow="autoplay; encrypted-media; picture-in-picture; fullscreen" allowfullscreen></iframe>
		</div>
	</div>
{/if}

<style>
	.extras-section { max-width:var(--content-width); margin:28px auto 0; }
	.extras-toggle { display:flex; align-items:center; gap:8px; padding:0; border:0; color:var(--text-muted); background:transparent; font:inherit; font-size:.85rem; cursor:pointer; }
	.extras-list { display:grid; gap:8px; margin-top:14px; }
	.extras-file { display:flex; align-items:center; gap:12px; min-height:48px; padding:12px 14px; border:1px solid var(--line-subtle); border-radius:12px; color:var(--text-soft); background:var(--surface-1); font:inherit; font-size:.82rem; text-align:left; cursor:pointer; }
	.extras-file span { flex:1; min-width:0; overflow-wrap:anywhere; }
	.detail-back { position:absolute; z-index:4; top:16px; left:18px; display:grid; place-items:center; width:42px; height:42px; border:1px solid rgba(255,255,255,.16); border-radius:50%; color:var(--text-strong); background:rgba(8,12,17,.65); backdrop-filter:blur(8px); text-decoration:none; }
	.episode-status { display: inline-flex; align-items: center; gap: 5px; margin: 8px 12px 0 0; color: var(--text-muted); font-size: .65rem; }
	.episode-thumb--empty { display: grid; place-items: center; color: var(--text-dim); }
	.downloads-section { max-width:var(--content-width); margin:52px auto 0; scroll-margin-top:24px; }
	.downloads-section--optional { margin-top:20px; }
	.other-versions-toggle { display:inline-flex; align-items:center; gap:8px; min-height:34px; padding:0; border:0; color:var(--text-muted); background:transparent; font:inherit; font-size:.72rem; cursor:pointer; }
	.other-versions-toggle:hover { color:var(--text-soft); }
	.other-versions-toggle[aria-expanded='true'] { margin-bottom:18px; }
	.episode-feedback { padding: 18px 0; color: var(--text-muted); font-size: .73rem; }
	.trailer-modal { position: fixed; z-index: 300; inset: 0; display: grid; place-items: center; padding: 42px; background: rgba(0,0,0,.82); }
	.trailer-modal__dismiss { position: absolute; inset: 0; width: 100%; border: 0; background: none; cursor: pointer; }
	.trailer-modal__content { position: relative; width: min(100%, 1100px); aspect-ratio: 16 / 9; background: #000; box-shadow: 0 24px 90px rgba(0,0,0,.65); }
	.trailer-modal__content iframe { display: block; width: 100%; height: 100%; border: 0; }
	.trailer-modal__close { position: absolute; z-index: 1; top: -42px; right: 0; display: grid; place-items: center; width: 36px; height: 36px; border: 1px solid rgba(255,255,255,.28); border-radius: 10px; color: #fff; background: #222831; cursor: pointer; }
	.detail-surface { min-height: 100vh; background: #101419; }
	:global(html[data-runtime='desktop']) .detail-surface { background: var(--acrylic-content-tint); }
	:global(html[data-runtime='desktop'] .detail-surface[data-native-backdrop='unavailable']) { background: var(--acrylic-content-fallback); }
	.detail-page { max-width: 1440px; margin: 0 auto; padding: 26px 42px 74px; }
	.back-link { position: relative; z-index: 81; display: inline-flex; align-items: center; gap: 7px; min-height: 44px; margin-left: -12px; padding: 0 12px; border: 1px solid transparent; border-radius: 13px; color: var(--text-muted); font-size: 0.74rem; text-decoration: none; transition: color 140ms ease, background-color 140ms ease; }
	.back-link:hover { color: var(--text-strong); background: rgba(255,255,255,0.06); }
	.detail-hero { position: relative; min-height: 470px; margin-top: 22px; overflow: hidden; border: 1px solid var(--line-subtle); border-radius: var(--radius-lg); background: var(--surface-1); }
	.detail-hero__backdrop { position: absolute; inset: 0 0 0 32%; overflow: hidden; background: var(--surface-1); }
	.detail-hero__backdrop img { display: block; width: 100%; height: 100%; object-fit: cover; object-position: center; filter: saturate(0.72); }
	.detail-hero__veil { position: absolute; inset: 0; background: linear-gradient(90deg, var(--surface-1) 0%, rgba(14,17,21,0.95) 26%, rgba(14,17,21,0.58) 66%, rgba(14,17,21,0.25) 100%), linear-gradient(0deg, rgba(14,17,21,0.52), transparent 38%); }
	.detail-hero__content { position: relative; display: flex; align-items: center; gap: 38px; min-height: 470px; padding: 50px; }
	.detail-poster { flex: 0 0 190px; overflow: hidden; aspect-ratio: 2 / 3; border-radius: 10px; box-shadow: 0 18px 38px rgba(0,0,0,0.32); }
	.detail-poster img { display: block; width: 100%; height: 100%; object-fit: cover; }
	.detail-copy { width: min(520px, 55%); }
	.detail-copy h1 { margin: 0; color: var(--text-strong); font-size: clamp(2.4rem, 5vw, 4.6rem); font-weight: 600; letter-spacing: -0.07em; line-height: 0.94; }
	.detail-meta { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 19px; color: var(--text-muted); font-size: 0.7rem; }
	.detail-title { display: grid; align-items: center; min-height: 104px; }
	.detail-title__logo { display: block; width: auto; height: auto; max-width: min(480px, 100%); max-height: 104px; object-fit: contain; object-position: left center; filter: drop-shadow(0 12px 38px rgba(0,0,0,0.4)); opacity: 0; }
	.detail-title__logo--ready { opacity: 1; }
	.detail-meta span:not(:first-child)::before { content: '·'; margin-right: 10px; color: var(--text-dim); }
	.detail-synopsis { max-width: 490px; margin: 18px 0 0; color: var(--text-soft); font-size: 0.8rem; line-height: 1.7; }
	.detail-actions { display: flex; flex-wrap: wrap; gap: 9px; margin-top: 24px; }
	.button { display: inline-flex; align-items: center; gap: 8px; min-height: 38px; padding: 0 15px; border: 1px solid transparent; border-radius: 7px; font-size: 0.74rem; font-weight: 650; text-decoration: none; cursor: pointer; }
	.button--primary { color: var(--surface-0); background: var(--accent-soft); }
	.button--primary:hover { background: #efd6a4; }
	.button--ghost { border-color: rgba(255,255,255,0.15); color: var(--text-soft); background: rgba(255,255,255,0.06); }
	.button--ghost:hover { border-color: rgba(201,174,124,0.4); color: var(--accent-soft); background: rgba(201,174,124,0.1); }
	.episodes-section, .related-section { margin-top: 52px; }
	.downloads-section--collapsed + .episodes-section { margin-top: 16px; }
	.section-heading { display: flex; align-items: end; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
	.section-heading h2 { margin: 0; color: var(--text-strong); font-size: 1.04rem; font-weight: 650; letter-spacing: -0.025em; }
	.season-select { position: relative; display: flex; align-items: center; gap: 5px; color: var(--text-muted); font-size: 0.72rem; }
	.episode-list { display: grid; border-top: 1px solid var(--line-subtle); }
	.episode-credit { margin: 12px 0 0; color: var(--text-dim); font-size: 0.62rem; }
	.episode-credit a { color: var(--text-muted); text-decoration: underline; text-underline-offset: 2px; }
	.episode-row { position: relative; display: grid; grid-template-columns: 180px minmax(0, 1fr); align-items: center; gap: 20px; margin: 0 -12px; padding: 17px 12px; border-bottom: 1px solid var(--line-subtle); border-radius: 10px; transition: background-color 140ms ease; }
	.episode-row:hover, .episode-row:has(.episode-select:focus-visible) { background: rgba(158,198,214,0.07); }
	.episode-row:hover .episode-title h3, .episode-row:has(.episode-select:focus-visible) .episode-title h3 { color: var(--accent-soft); }
	.episode-art { position: relative; min-width: 0; }
	.episode-thumb { position: relative; overflow: hidden; aspect-ratio: 16 / 9; border-radius: 7px; background: var(--surface-2); }
	.episode-thumb img { display: block; width: 100%; height: 100%; object-fit: cover; }
	.episode-number { position: absolute; top: 8px; left: 8px; color: rgba(255,255,255,0.84); font-size: 0.62rem; font-weight: 700; letter-spacing: 0.1em; }
	.episode-copy { min-width: 0; padding-right: 42px; }
	.episode-title { display: flex; align-items: baseline; gap: 10px; }
	.episode-title h3 { overflow: hidden; margin: 0; color: var(--text-soft); font-size: 0.84rem; font-weight: 620; text-overflow: ellipsis; white-space: nowrap; }
	.episode-title span { flex: 0 0 auto; color: var(--text-muted); font-size: 0.68rem; }
	.episode-copy p { max-width: 600px; margin: 7px 0 0; color: var(--text-muted); font-size: 0.73rem; line-height: 1.55; }
	.episode-select { position: absolute; z-index: 1; inset: 0; width: 100%; height: 100%; padding: 0; border: 0; border-radius: inherit; background: transparent; cursor: pointer; }
	.related-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 20px; }
	@media (max-width: 900px) { .detail-page { padding-right: 28px; padding-left: 28px; } .detail-hero__content { padding: 38px; } .detail-poster { flex-basis: 150px; } .detail-copy { width: min(500px, 64%); } .related-grid { grid-template-columns: repeat(3, minmax(0, 1fr)); } :global(.related-grid .poster-card:nth-child(4)) { display: none; } }
	@media (max-width: 680px) { .detail-page { padding: 20px 18px 56px; } .detail-hero { min-height: 650px; margin-top: 18px; } .detail-hero__backdrop { inset: 0 0 48% 0; } .detail-hero__veil { background: linear-gradient(0deg, var(--surface-1) 0%, rgba(14,17,21,0.96) 43%, rgba(14,17,21,0.12) 73%), linear-gradient(90deg, rgba(14,17,21,0.24), transparent); } .detail-hero__content { align-items: end; flex-direction: column; justify-content: end; gap: 24px; min-height: 650px; padding: 28px 24px; } .detail-poster { align-self: flex-start; flex-basis: auto; width: 120px; } .detail-copy { width: 100%; } .detail-copy h1 { font-size: clamp(2.4rem, 12vw, 3.8rem); } .detail-synopsis { font-size: 0.75rem; } .episode-row { grid-template-columns: 122px minmax(0, 1fr) 32px; gap: 13px; } .episode-copy p { display: -webkit-box; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; } .related-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; } }

	/* Details use the Home's canvas and material grammar, without a page-sized card. */
	.detail-page { max-width: var(--content-width); margin: 0 auto; padding: 28px var(--content-gutter) 84px; }
	.back-link { color: var(--text-muted); font-weight: 560; }
	.detail-hero { min-height: clamp(410px, 49svh, 530px); margin-top: 18px; overflow: hidden; border: 1px solid rgba(255,255,255,0.1); border-radius: 28px; corner-shape: squircle; background: transparent; }
	.detail-hero__backdrop { inset: 0; background-image: linear-gradient(90deg, rgba(5, 7, 10, 0.97) 0%, rgba(5, 7, 10, 0.82) 31%, rgba(5, 7, 10, 0.34) 66%, rgba(5, 7, 10, 0.14) 100%), linear-gradient(0deg, rgba(7, 9, 12, 0.98), rgba(7, 9, 12, 0.12) 64%), var(--backdrop); background-position: center; background-size: cover; filter: saturate(0.78) contrast(1.02); }
	.detail-hero__veil { display: none; }
	:global(html:not([data-mobile-preview='true'])) .detail-hero__veil {
		display: block;
		pointer-events: none;
		background: linear-gradient(90deg, rgba(9, 14, 21, 0.9) 0%, rgba(9, 14, 21, 0.82) 30%, rgba(9, 14, 21, 0.64) 55%, rgba(9, 14, 21, 0.18) 80%, transparent 100%);
	}
	.detail-hero__content { align-items: center; max-width: var(--content-width); min-height: clamp(410px, 49svh, 530px); margin: 0 auto; padding: 58px var(--content-gutter); gap: 32px; }
	.detail-poster { flex: 0 0 166px; width: 166px; border: 1px solid rgba(255,255,255,0.12); border-radius: 14px; corner-shape: squircle; box-shadow: 0 20px 42px rgba(0,0,0,0.34); }
	.detail-copy { width: min(630px, 66%); }
	.detail-copy h1 { font-family: var(--font-display); font-size: clamp(3rem, 5.2vw, 5.2rem); font-weight: 650; letter-spacing: -0.072em; line-height: 0.94; }
	.detail-meta { gap: 10px; margin-top: 16px; font-weight: 560; }
	.detail-synopsis { max-width: 540px; margin-top: 16px; font-size: 0.82rem; line-height: 1.62; }
	.detail-actions { gap: 10px; margin-top: 21px; }
	:global(html:not([data-mobile-preview='true'])) .detail-actions :global(.favorite-button) { width: 42px; height: auto; align-self: stretch; border-radius: 11px; }
	.button { min-height: 42px; padding: 0 17px; border-radius: 11px; font-size: 0.76rem; font-weight: 710; transition: background-color 160ms ease, border-color 160ms ease, color 160ms ease, transform 160ms ease; }
	.button:hover, .button:focus-visible { transform: scale(1.02); }
	.button--primary { color: #0a0c0f; background: rgba(248,249,250,0.94); box-shadow: inset 0 1px rgba(255,255,255,0.96), 0 10px 28px rgba(0,0,0,0.24); }
	.button--primary:hover { background: #fff; }
	.button--ghost { border-color: rgba(255,255,255,0.14); color: var(--text-strong); background: rgba(54,60,68,0.48); box-shadow: inset 0 1px rgba(255,255,255,0.075), 0 8px 24px rgba(0,0,0,0.16); backdrop-filter: blur(16px) saturate(135%); }
	.button--ghost:hover { border-color: rgba(158,198,214,0.38); color: var(--accent-soft); background: rgba(64,78,86,0.62); }
	.episodes-section, .related-section { max-width: var(--content-width); margin: 52px auto 0; }
	.section-heading { align-items: center; margin-bottom: 17px; }
	.section-heading h2 { font-family: var(--font-display); font-size: 1.08rem; font-weight: 690; letter-spacing: -0.034em; }
	.episode-list { border-top-color: var(--line-subtle); }
	.episode-row { grid-template-columns: 170px minmax(0, 1fr); gap: 20px; padding: 16px 12px; border-bottom-color: var(--line-subtle); }
	.episode-thumb { border: 1px solid rgba(255,255,255,0.1); border-radius: 9px; }
	.episode-number { position: static; color: var(--accent); font-size: 0.68rem; font-weight: 700; letter-spacing: 0.08em; }
	.episode-title { gap: 9px; }
	.episode-title h3 { color: var(--text-strong); font-weight: 650; }
	.episode-copy p { max-width: 680px; margin-top: 6px; }
	.related-grid { grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 18px; }
	@media (max-width: 900px) { .detail-hero__content { padding-right: 28px; padding-left: 28px; } }
	@media (max-width: 680px) {
		.detail-page { padding: 28px 18px 72px; }
		.detail-hero { min-height: clamp(500px, 72svh, 620px); margin: 18px -18px 0; }
		.detail-hero__backdrop { inset: 0; background-image: linear-gradient(0deg, rgba(7,9,12,0.98) 0%, rgba(7,9,12,0.72) 40%, rgba(7,9,12,0.08) 76%), var(--backdrop); background-position: 60% center; }
		.detail-hero__content { align-items: start; flex-direction: column; justify-content: end; min-height: clamp(500px, 72svh, 620px); padding: 54px 18px 34px; gap: 18px; }
		.detail-poster { flex: 0 0 auto; width: 104px; border-radius: 10px; }
		.detail-copy { width: 100%; }
		.detail-copy h1 { max-width: 96%; font-size: clamp(2.75rem, 12vw, 3.7rem); }
		.detail-meta { margin-top: 13px; font-size: 0.67rem; }
		.detail-synopsis { display: -webkit-box; max-width: 96%; margin-top: 13px; overflow: hidden; font-size: 0.76rem; line-height: 1.52; -webkit-box-orient: vertical; -webkit-line-clamp: 3; line-clamp: 3; }
		.detail-actions { margin-top: 18px; }
		.button { min-height: 40px; padding: 0 15px; }
		.episodes-section, .related-section { margin-top: 43px; }
		.episode-row { grid-template-columns: 116px minmax(0, 1fr); gap: 12px; padding: 14px 12px; }
		.episode-copy p { display: -webkit-box; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
		.related-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
	}
	@media (prefers-reduced-motion: reduce) { .button, .episode-row { transition: none; } .button:hover, .button:focus-visible { transform: none; } }
	@media (prefers-reduced-transparency: reduce) {
		.button--ghost { background: var(--surface-3); backdrop-filter: none; }
		:global(html[data-runtime='desktop']) .detail-surface { background: #0c0f13; }
	}
</style>
