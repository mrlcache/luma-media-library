<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import { media } from '$lib/data';
	import { usePlayer } from '$lib/player-context';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const player = usePlayer();
	let selectedSeason = $state(1);
	let saved = $state(false);
	let episodes = $derived(data.item.episodes ?? []);
	let related = $derived(media.filter((item) => item.id !== data.item.id && item.kind === data.item.kind).slice(0, 4));

	function playEpisode() {
		player.open(data.item);
	}
</script>

<svelte:head><title>{data.item.title} · Media library</title></svelte:head>

<div class="detail-page">
	<a class="back-link" href={data.item.kind === 'movie' ? '/library?type=movie' : '/library?type=series'}><Icon name="arrow-left" size={15} />Back to {data.item.kind === 'movie' ? 'movies' : 'series'}</a>

	<section class="detail-hero" style={`--backdrop: url("${data.item.backdrop}")`}>
		<div class="detail-hero__backdrop"></div>
		<div class="detail-hero__veil"></div>
		<div class="detail-hero__content">
			<div class="detail-poster"><img src={data.item.poster} alt={`Poster for ${data.item.title}`} width="520" height="780" /></div>
			<div class="detail-copy">
				<p class="detail-kicker">{data.item.kind === 'series' ? 'Series' : 'Movie'} <span>·</span> {data.item.year}</p>
				<h1>{data.item.title}</h1>
				<div class="detail-meta"><span class="match">{data.item.match} match</span><span>{data.item.rating} rating</span><span>{data.item.runtime}</span><span>{data.item.genres.join(' · ')}</span></div>
				<p class="detail-synopsis">{data.item.synopsis}</p>
				<div class="detail-actions">
					<button class="button button--primary" onclick={playEpisode}><Icon name="play" size={15} />{data.item.progress ? 'Resume' : 'Play now'}</button>
					<button class="button button--ghost" class:saved aria-pressed={saved} onclick={() => (saved = !saved)}><Icon name={saved ? 'check' : 'bookmark'} size={15} />{saved ? 'In library' : 'Add to library'}</button>
				</div>
				<p class="detail-note"><Icon name="info" size={14} />{data.item.kind === 'series' ? `${data.item.seasons} seasons available` : 'Available for direct play'}</p>
			</div>
		</div>
	</section>

	{#if data.item.kind === 'series'}
		<section class="episodes-section" aria-labelledby="episodes-heading">
			<div class="section-heading">
				<div><h2 id="episodes-heading">Episodes</h2><p>Continue with season {selectedSeason}.</p></div>
				<label class="season-select"><span class="sr-only">Season</span><select bind:value={selectedSeason}><option value={1}>Season 1</option><option value={2}>Season 2</option><option value={3}>Season 3</option></select><Icon name="chevron-down" size={14} /></label>
			</div>
			<div class="episode-list">
				{#each episodes as episode, index (episode.id)}
					<article class="episode-row">
						<div class="episode-thumb"><img src={episode.thumbnail} alt="" width="520" height="292" loading={index === 0 ? 'eager' : 'lazy'} /><span class="episode-number">{String(episode.number).padStart(2, '0')}</span>{#if episode.progress}<span class="episode-progress" style={`--progress: ${episode.progress * 100}%`}></span>{/if}</div>
						<div class="episode-copy"><div class="episode-title"><h3>{episode.title}</h3><span>{episode.duration}</span></div><p>{episode.summary}</p></div>
						<button class="episode-play" aria-label={`Play episode ${episode.number}, ${episode.title}`} onclick={playEpisode}><Icon name="play" size={15} /></button>
					</article>
				{/each}
			</div>
		</section>
	{/if}

	<section class="related-section" aria-labelledby="related-heading">
		<div class="section-heading"><div><h2 id="related-heading">You may also like</h2><p>More from your library.</p></div></div>
		<div class="related-grid">{#each related as item (item.id)}<PosterCard media={item} />{/each}</div>
	</section>
</div>

<style>
	.detail-page { max-width: 1440px; margin: 0 auto; padding: 26px 42px 74px; }
	.back-link { display: inline-flex; align-items: center; gap: 7px; color: var(--text-muted); font-size: 0.74rem; text-decoration: none; }
	.back-link:hover { color: var(--text-strong); }
	.detail-hero { position: relative; min-height: 470px; margin-top: 22px; overflow: hidden; border: 1px solid var(--line-subtle); border-radius: var(--radius-lg); background: var(--surface-1); }
	.detail-hero__backdrop { position: absolute; inset: 0 0 0 32%; background-image: var(--backdrop); background-position: center; background-size: cover; filter: saturate(0.72); }
	.detail-hero__veil { position: absolute; inset: 0; background: linear-gradient(90deg, var(--surface-1) 0%, rgba(14,17,21,0.95) 26%, rgba(14,17,21,0.58) 66%, rgba(14,17,21,0.25) 100%), linear-gradient(0deg, rgba(14,17,21,0.52), transparent 38%); }
	.detail-hero__content { position: relative; display: flex; align-items: center; gap: 38px; min-height: 470px; padding: 50px; }
	.detail-poster { flex: 0 0 190px; overflow: hidden; aspect-ratio: 2 / 3; border-radius: 10px; box-shadow: 0 18px 38px rgba(0,0,0,0.32); }
	.detail-poster img { display: block; width: 100%; height: 100%; object-fit: cover; }
	.detail-copy { width: min(520px, 55%); }
	.detail-kicker { margin: 0 0 13px; color: var(--accent-soft); font-size: 0.66rem; font-weight: 700; letter-spacing: 0.14em; text-transform: uppercase; }
	.detail-kicker span { margin: 0 5px; color: var(--text-dim); }
	.detail-copy h1 { margin: 0; color: var(--text-strong); font-size: clamp(2.4rem, 5vw, 4.6rem); font-weight: 600; letter-spacing: -0.07em; line-height: 0.94; }
	.detail-meta { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 19px; color: var(--text-muted); font-size: 0.7rem; }
	.detail-meta span:not(:first-child)::before { content: '·'; margin-right: 10px; color: var(--text-dim); }
	.detail-meta .match { color: var(--success); font-weight: 700; }
	.detail-meta .match::before { display: none; }
	.detail-synopsis { max-width: 490px; margin: 18px 0 0; color: var(--text-soft); font-size: 0.8rem; line-height: 1.7; }
	.detail-actions { display: flex; flex-wrap: wrap; gap: 9px; margin-top: 24px; }
	.button { display: inline-flex; align-items: center; gap: 8px; min-height: 38px; padding: 0 15px; border: 1px solid transparent; border-radius: 7px; font-size: 0.74rem; font-weight: 650; text-decoration: none; cursor: pointer; }
	.button--primary { color: var(--surface-0); background: var(--accent-soft); }
	.button--primary:hover { background: #efd6a4; }
	.button--ghost { border-color: rgba(255,255,255,0.15); color: var(--text-soft); background: rgba(255,255,255,0.06); }
	.button--ghost:hover, .button--ghost.saved { border-color: rgba(201,174,124,0.4); color: var(--accent-soft); background: rgba(201,174,124,0.1); }
	.detail-note { display: flex; align-items: center; gap: 7px; margin: 22px 0 0; color: var(--text-muted); font-size: 0.68rem; }
	.episodes-section, .related-section { margin-top: 52px; }
	.section-heading { display: flex; align-items: end; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
	.section-heading h2 { margin: 0; color: var(--text-strong); font-size: 1.04rem; font-weight: 650; letter-spacing: -0.025em; }
	.section-heading p { margin: 4px 0 0; color: var(--text-muted); font-size: 0.73rem; }
	.season-select { position: relative; display: flex; align-items: center; gap: 5px; color: var(--text-muted); font-size: 0.72rem; }
	.season-select select { appearance: none; padding: 5px 20px 5px 8px; border: 1px solid var(--line-subtle); border-radius: 6px; outline: none; color: var(--text-soft); font-size: 0.72rem; background: var(--surface-1); cursor: pointer; }
	.season-select :global(svg) { position: absolute; right: 5px; pointer-events: none; }
	.episode-list { display: grid; border-top: 1px solid var(--line-subtle); }
	.episode-row { display: grid; grid-template-columns: 180px minmax(0, 1fr) 36px; align-items: center; gap: 20px; padding: 17px 0; border-bottom: 1px solid var(--line-subtle); }
	.episode-thumb { position: relative; overflow: hidden; aspect-ratio: 16 / 9; border-radius: 7px; background: var(--surface-2); }
	.episode-thumb img { display: block; width: 100%; height: 100%; object-fit: cover; }
	.episode-number { position: absolute; top: 8px; left: 8px; color: rgba(255,255,255,0.84); font-size: 0.62rem; font-weight: 700; letter-spacing: 0.1em; }
	.episode-progress { position: absolute; right: 8px; bottom: 8px; left: 8px; height: 3px; border-radius: 2px; background: linear-gradient(90deg, var(--accent) var(--progress), rgba(255,255,255,0.28) var(--progress)); }
	.episode-copy { min-width: 0; }
	.episode-title { display: flex; align-items: baseline; gap: 10px; }
	.episode-title h3 { overflow: hidden; margin: 0; color: var(--text-soft); font-size: 0.84rem; font-weight: 620; text-overflow: ellipsis; white-space: nowrap; }
	.episode-title span { flex: 0 0 auto; color: var(--text-muted); font-size: 0.68rem; }
	.episode-copy p { max-width: 600px; margin: 7px 0 0; color: var(--text-muted); font-size: 0.73rem; line-height: 1.55; }
	.episode-play { display: grid; place-items: center; width: 33px; height: 33px; padding: 0; border: 1px solid var(--line-subtle); border-radius: 50%; color: var(--text-soft); background: var(--surface-2); cursor: pointer; }
	.episode-play:hover { border-color: var(--accent); color: var(--accent-soft); }
	.related-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 20px; }
	@media (max-width: 900px) { .detail-page { padding-right: 28px; padding-left: 28px; } .detail-hero__content { padding: 38px; } .detail-poster { flex-basis: 150px; } .detail-copy { width: min(500px, 64%); } .related-grid { grid-template-columns: repeat(3, minmax(0, 1fr)); } :global(.related-grid .poster-card:nth-child(4)) { display: none; } }
	@media (max-width: 680px) { .detail-page { padding: 20px 18px 56px; } .detail-hero { min-height: 650px; margin-top: 18px; } .detail-hero__backdrop { inset: 0 0 48% 0; } .detail-hero__veil { background: linear-gradient(0deg, var(--surface-1) 0%, rgba(14,17,21,0.96) 43%, rgba(14,17,21,0.12) 73%), linear-gradient(90deg, rgba(14,17,21,0.24), transparent); } .detail-hero__content { align-items: end; flex-direction: column; justify-content: end; gap: 24px; min-height: 650px; padding: 28px 24px; } .detail-poster { align-self: flex-start; flex-basis: auto; width: 120px; } .detail-copy { width: 100%; } .detail-copy h1 { font-size: clamp(2.4rem, 12vw, 3.8rem); } .detail-synopsis { font-size: 0.75rem; } .episode-row { grid-template-columns: 122px minmax(0, 1fr) 32px; gap: 13px; } .episode-copy p { display: -webkit-box; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; } .related-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; } }
</style>
