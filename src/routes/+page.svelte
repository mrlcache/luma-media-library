<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import MediaRow from '$lib/components/MediaRow.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import { continueWatching, featured, featuredAmbient, recentlyAdded, media } from '$lib/data';
	import { usePlayer } from '$lib/player-context';

	const player = usePlayer();
	const libraryPreview = [media[2], media[3], media[4], media[6], media[7], media[8]];
</script>

<svelte:head><title>Home · Media library</title></svelte:head>

<div class="home-page" style={`--backdrop: url("${featured.backdrop}"); --ambient: url("${featuredAmbient}")`}>
	<section class="featured" aria-labelledby="featured-title">
		<div class="featured__backdrop" aria-hidden="true"></div>
		<div class="featured__content">
			<h1 id="featured-title">{featured.title}</h1>
			<div class="featured__meta">
				<span class="match">{featured.match} match</span>
				<span>{featured.year}</span>
				<span>{featured.runtime}</span>
				<span>{featured.genres.join(' · ')}</span>
			</div>
			<p class="featured__synopsis">{featured.synopsis}</p>
			<div class="featured__actions">
				<button class="button button--primary" onclick={() => player.open(featured)}>
					<Icon name="play" size={16} weight="fill" />
					Play
				</button>
				<a class="button button--secondary" href={`/title/${featured.id}`}>
					<Icon name="info" size={18} weight="bold" />
					Details
				</a>
			</div>
		</div>
	</section>

	<div class="home-content">
		<div class="home-section home-section--first">
			<MediaRow title="Continue watching" items={continueWatching} showProgress />
		</div>

		<div class="home-section home-section--library">
			<div class="section-heading">
				<h2>Your library</h2>
				<a href="/library" class="section-link">View all <Icon name="chevron-right" size={15} weight="bold" /></a>
			</div>
			<div class="poster-grid-preview">
				{#each libraryPreview as item, index (item.id)}
					<PosterCard media={item} priority={index < 2} variant="home" />
				{/each}
			</div>
		</div>

		<div class="home-section home-section--last">
			<MediaRow title="Recently added" items={recentlyAdded} />
		</div>
	</div>
</div>

<style>
	.home-page {
		min-height: 100%;
		background: #080a0d;
	}

	.featured {
		position: relative;
		min-height: clamp(410px, 49svh, 530px);
		overflow: hidden;
		isolation: isolate;
	}

	.featured__backdrop {
		position: absolute;
		inset: 0;
		background-image:
			linear-gradient(90deg, rgba(3, 5, 8, 0.96) 0%, rgba(3, 5, 8, 0.72) 34%, rgba(3, 5, 8, 0.15) 68%, rgba(3, 5, 8, 0.08) 100%),
			linear-gradient(0deg, rgba(7, 9, 12, 0.98) 0%, rgba(7, 9, 12, 0.22) 34%, rgba(7, 9, 12, 0.08) 75%),
			var(--backdrop);
		background-position: center 46%;
		background-size: cover;
		filter: saturate(0.84) contrast(1.04);
		transform: scale(1.008);
		z-index: -1;
	}

	.featured__content {
		display: grid;
		align-content: end;
		min-height: clamp(410px, 49svh, 530px);
		max-width: var(--content-width);
		margin: 0 auto;
		padding: 76px var(--content-gutter) 64px;
	}

	.featured h1 {
		max-width: 760px;
		margin: 0;
		color: var(--text-strong);
		font-family: var(--font-display);
		font-size: clamp(3.25rem, 5vw, 5.2rem);
		font-weight: 650;
		letter-spacing: -0.065em;
		line-height: 0.94;
		text-wrap: balance;
		text-shadow: 0 12px 42px rgba(0, 0, 0, 0.38);
	}

	.featured__meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 10px;
		margin-top: 17px;
		color: rgba(229, 234, 239, 0.7);
		font-size: 0.71rem;
		font-weight: 590;
		letter-spacing: -0.006em;
	}

	.featured__meta span + span::before {
		margin-right: 10px;
		color: rgba(229, 234, 239, 0.32);
		content: '·';
	}
	.featured__meta .match { color: #b9d9cd; font-weight: 700; }

	.featured__synopsis {
		max-width: 535px;
		margin: 17px 0 0;
		color: rgba(239, 242, 244, 0.78);
		font-size: 0.84rem;
		font-weight: 475;
		line-height: 1.62;
		text-wrap: pretty;
	}

	.featured__actions { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 24px; }
	.button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		min-height: 42px;
		padding: 0 17px;
		border: 1px solid transparent;
		border-radius: 11px;
		font-size: 0.76rem;
		font-weight: 710;
		letter-spacing: -0.012em;
		text-decoration: none;
		cursor: pointer;
		transition: background-color 160ms ease, box-shadow 160ms ease, transform 160ms ease;
	}
	.button:hover,
	.button:focus-visible { transform: scale(1.025); }
	.button--primary {
		color: #0a0c0f;
		background: rgba(248, 249, 250, 0.94);
		box-shadow: inset 0 1px rgba(255, 255, 255, 0.96), 0 10px 28px rgba(0, 0, 0, 0.26);
	}
	.button--primary:hover { background: #fff; box-shadow: inset 0 1px #fff, 0 13px 34px rgba(0, 0, 0, 0.32); }
	.button--secondary {
		border-color: rgba(255, 255, 255, 0.14);
		color: var(--text-strong);
		background: rgba(54, 60, 68, 0.48);
		box-shadow: inset 0 1px rgba(255, 255, 255, 0.075), 0 8px 24px rgba(0, 0, 0, 0.16);
		-webkit-backdrop-filter: blur(20px) saturate(145%);
		backdrop-filter: blur(20px) saturate(145%);
	}
	.button--secondary:hover { background: rgba(70, 77, 86, 0.61); }

	.home-content {
		position: relative;
		max-width: var(--content-width);
		margin: -20px auto 0;
		padding: 40px var(--content-gutter) 92px;
		border-top: 1px solid rgba(255, 255, 255, 0.1);
		border-radius: 22px 22px 0 0;
		background-image:
			linear-gradient(180deg, rgba(13, 16, 20, 0.91), rgba(8, 10, 13, 0.965) 390px),
			var(--ambient);
		background-position: center;
		background-size: cover;
		box-shadow: inset 0 1px rgba(255, 255, 255, 0.045), 0 -22px 55px rgba(0, 0, 0, 0.18);
		z-index: 2;
	}

	.home-section + .home-section { margin-top: 52px; }
	.home-section--last { margin-top: 58px; }
	.section-heading { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
	.section-heading h2 { margin: 0; color: var(--text-strong); font-family: var(--font-display); font-size: 1.08rem; font-weight: 690; letter-spacing: -0.034em; }
	.section-link { display: inline-flex; align-items: center; gap: 4px; flex: 0 0 auto; color: var(--text-muted); font-size: 0.71rem; font-weight: 590; text-decoration: none; transition: color 140ms ease; }
	.section-link:hover { color: var(--text-strong); }
	.poster-grid-preview {
		display: grid;
		grid-auto-columns: clamp(142px, 12.3vw, 198px);
		grid-auto-flow: column;
		gap: 17px;
		overflow-x: auto;
		padding: 3px 3px 16px;
		scrollbar-width: none;
		scroll-snap-type: x proximity;
	}
	.poster-grid-preview::-webkit-scrollbar { display: none; }
	.poster-grid-preview > :global(*) { scroll-snap-align: start; }

	@media (max-width: 760px) {
		.featured { min-height: clamp(445px, 61svh, 540px); }
		.featured__backdrop {
			background-image:
				linear-gradient(0deg, rgba(7, 9, 12, 0.97) 0%, rgba(7, 9, 12, 0.64) 35%, rgba(7, 9, 12, 0.06) 73%),
				var(--backdrop);
			background-position: 60% center;
			filter: saturate(0.83) contrast(1.04);
		}
		.featured__content { min-height: clamp(445px, 61svh, 540px); padding: 100px 18px 48px; }
		.featured h1 { max-width: 94%; font-size: clamp(2.75rem, 12vw, 3.6rem); font-weight: 650; line-height: 0.94; }
		.featured__meta { margin-top: 14px; font-size: 0.67rem; }
		.featured__meta span:nth-child(4) { display: none; }
		.featured__synopsis { display: -webkit-box; max-width: 96%; margin-top: 14px; overflow: hidden; font-size: 0.76rem; line-height: 1.52; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
		.featured__actions { margin-top: 18px; }
		.button { min-height: 40px; padding: 0 15px; }
		.button--secondary { backdrop-filter: blur(12px); }
		.home-content { margin-top: -18px; padding: 31px 18px 80px; border-radius: 18px 18px 0 0; background-attachment: scroll; }
		.home-section + .home-section { margin-top: 43px; }
		.home-section--last { margin-top: 48px; }
		.section-heading { margin-bottom: 14px; }
		.section-heading h2 { font-size: 1rem; }
		.poster-grid-preview { grid-auto-columns: 42vw; gap: 12px; margin-right: -18px; padding-right: 18px; }
	}

	@media (prefers-reduced-motion: reduce) {
		.button,
		.section-link { transition: none; }
		.button:hover,
		.button:focus-visible { transform: none; }
	}

	@media (prefers-reduced-transparency: reduce) {
		.button--secondary { background: var(--surface-3); backdrop-filter: none; }
		.home-content { background: #0c0f13; }
	}
</style>
