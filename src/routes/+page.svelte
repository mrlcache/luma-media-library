<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import CatalogSkeletonGrid from '$lib/components/CatalogSkeletonGrid.svelte';
	import EmptyLibraryCard from '$lib/components/EmptyLibraryCard.svelte';
	import MediaRow from '$lib/components/MediaRow.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import { isDesktopRuntime, readCatalogPage, readContinueWatching, readPlaybackHistory, readTitleLogo, readTitleTrailer } from '$lib/platform/desktop';
	import { recoverRemoteArtwork, tmdbImageSize } from '$lib/media/artwork';
	import { loadYouTubeIframeApi, type YouTubePlayer } from '$lib/media/youtube-iframe';
	import { PLAYBACK_HISTORY_UPDATED_EVENT, usePlayer } from '$lib/player-context';
	import { nativeAcrylicStatus, requestNativeAcrylic } from '$lib/platform/native-acrylic';
	import { smoothHorizontalScroll } from '$lib/scroll/lenis';
	import { rightEdgeHint } from '$lib/scroll/right-edge-hint';
	import type { CatalogMedia, ContinueWatchingItem, MediaItem, PlaybackHistoryItem, TmdbTrailer } from '$lib/types';

	const player = usePlayer();
	let homeContent: HTMLDivElement;
	let heroElement = $state<HTMLElement>();
	let heroInView = $state(false);
	let heroTrailer = $state<TmdbTrailer | null>(null);
	let heroLogoUrl = $state<string | null>(null);
	let heroLogoFailed = $state(false);
	let heroLogoReady = $state(false);
	let heroLogoResolved = $state(false);
	let showHeroVideo = $state(false);
	let heroVideoReady = $state(false);
	let heroTrailerEnded = $state(false);
	let heroTrailerExpanded = $state(false);
	let heroRestartOnReturn = false;
	let heroVideoIframe = $state<HTMLIFrameElement>();
	let backgroundPlayer: YouTubePlayer | null = null;
	let desktopCatalog = $state(isDesktopRuntime());
	let catalogItems = $state<CatalogMedia[]>([]);
	let resumeItems = $state<ContinueWatchingItem[]>([]);
	let playbackHistoryItems = $state<PlaybackHistoryItem[]>([]);
	let catalogLoading = $state(true);
	let catalogError = $state('');
	let reduceMotionEnabled = $state(false);

	function toMediaItem(item: CatalogMedia | ContinueWatchingItem | PlaybackHistoryItem): MediaItem {
		const kind = item.kind === 'series' ? 'series' : 'movie';
		const rating = 'voteAverage' in item ? item.voteAverage : null;
		const duration = 'durationSeconds' in item ? item.durationSeconds : 0;
		const position = 'positionSeconds' in item ? item.positionSeconds : 0;
		const remainingMinutes = Math.max(0, Math.ceil((duration - position) / 60));
		const remaining = remainingMinutes >= 60
			? `${Math.floor(remainingMinutes / 60)}h ${remainingMinutes % 60}m left`
			: `${remainingMinutes}m left`;

		return {
			id: String(item.id),
			title: item.title,
			kind,
			year: item.year ?? 0,
			genres: [kind === 'series' ? 'Series' : 'Movie'],
			rating: rating?.toFixed(1) ?? '',
			runtime: '',
			poster: tmdbImageSize(item.posterUrl, 'w780'),
			backdrop: tmdbImageSize(item.backdropUrl ?? item.posterUrl, 'w1280'),
			synopsis: 'overview' in item ? item.overview ?? '' : '',
			match: '',
			...('durationSeconds' in item && duration > 0
				? { progress: Math.min(1, position / duration), progressLabel: `${kind === 'series' ? 'Series' : 'Movie'} · ${item.year ?? ''} · ${remaining}` }
				: {})
		};
	}

	function openHeroTrailer() {
		// The fullscreen player loads its own iframe, so it must not depend on the
		// muted background iframe firing `load` first.
		if (!showHeroVideo || !heroTrailer) return;
		heroTrailerExpanded = true;
	}

	function backgroundTrailerUrl(key: string) {
		const origin = typeof window !== 'undefined' && /^https?:$/.test(window.location.protocol)
			? `&origin=${encodeURIComponent(window.location.origin)}`
			: '';
		return `https://www.youtube-nocookie.com/embed/${encodeURIComponent(key)}?autoplay=0&mute=1&controls=0&playsinline=1&rel=0&enablejsapi=1${origin}`;
	}

	function syncBackgroundPlayback() {
		if (!backgroundPlayer) return;
		if (heroInView && !document.hidden && !heroTrailerExpanded && !heroTrailerEnded && !reduceMotionEnabled) backgroundPlayer.playVideo();
		else backgroundPlayer.pauseVideo();
	}

	let libraryItems = $derived(catalogItems.map(toMediaItem));
	let recentlyWatchedItems = $derived(playbackHistoryItems.map(toMediaItem));
	let continueWatchingItems = $derived.by(() => {
		const entries = new Map<string, { media: MediaItem; updatedAt: number }>();
		const keyFor = (item: ContinueWatchingItem | PlaybackHistoryItem) =>
			`${item.kind ?? 'movie'}:${item.year ?? 0}:${item.title.trim().toLowerCase()}`;

		for (const item of playbackHistoryItems) {
			entries.set(keyFor(item), { media: toMediaItem(item), updatedAt: item.updatedAt });
		}
		for (const item of resumeItems) {
			const key = keyFor(item);
			if (item.updatedAt >= (entries.get(key)?.updatedAt ?? 0)) {
				entries.set(key, { media: toMediaItem(item), updatedAt: item.updatedAt });
			}
		}

		return [...entries.values()]
			.sort((a, b) => b.updatedAt - a.updatedAt)
			.slice(0, 12)
			.map(({ media }) => media);
	});
	let featured = $derived(recentlyWatchedItems[0] ?? continueWatchingItems[0] ?? libraryItems.find((item) => item.backdrop || item.poster) ?? null);
	let featuredBackdrop = $derived(featured ? tmdbImageSize(featured.backdrop, 'original') : '');
	let libraryPreview = $derived(libraryItems.filter((item) => item.poster).slice(0, 8));
	let recentlyAdded = $derived(libraryItems.slice(0, 8));

	$effect(() => {
		const id = Number(featured?.id);
		if (!desktopCatalog || !Number.isSafeInteger(id) || id <= 0) return;
		let cancelled = false;
		heroTrailer = null;
		showHeroVideo = false;
		heroVideoReady = false;
		heroTrailerEnded = false;
		heroTrailerExpanded = false;
		heroRestartOnReturn = false;
		void readTitleTrailer(id).then((trailer) => {
			if (!cancelled) heroTrailer = trailer;
		}).catch(() => { if (!cancelled) heroTrailer = null; });
		return () => { cancelled = true; };
	});

	$effect(() => {
		const id = Number(featured?.id);
		heroLogoUrl = null;
		heroLogoFailed = false;
		heroLogoReady = false;
		heroLogoResolved = false;
		if (!desktopCatalog || !Number.isSafeInteger(id) || id <= 0) {
			heroLogoResolved = true;
			return;
		}
		let cancelled = false;
		void readTitleLogo(id).then((url) => {
			if (cancelled) return;
			heroLogoUrl = url;
			heroLogoResolved = true;
		}).catch(() => {
			if (cancelled) return;
			heroLogoUrl = null;
			heroLogoResolved = true;
		});
		return () => { cancelled = true; };
	});

	$effect(() => {
		const trailer = heroTrailer;
		if (!trailer || showHeroVideo || heroTrailerEnded || !heroInView || document.hidden || reduceMotionEnabled || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
		const timer = window.setTimeout(() => { showHeroVideo = true; }, 5000);
		return () => window.clearTimeout(timer);
	});

	$effect(() => {
		const iframe = heroVideoIframe;
		if (!iframe || !showHeroVideo) return;
		let disposed = false;
		let player: YouTubePlayer | null = null;
		void loadYouTubeIframeApi().then((api) => {
			if (disposed || !iframe.isConnected) return;
			player = new api.Player(iframe, {
				events: {
					onReady: ({ target }) => {
						if (disposed) return;
						backgroundPlayer = target;
						target.mute();
						syncBackgroundPlayback();
					},
					onStateChange: ({ data }) => {
						if (disposed) return;
						if (data === 1) heroVideoReady = true;
						if (data === 0) {
							heroTrailerEnded = true;
							heroVideoReady = false;
						}
					},
					onError: ({ data }) => {
						if (disposed) return;
						console.warn('Background trailer unavailable', data);
						heroTrailerEnded = true;
						heroVideoReady = false;
					}
				}
			});
		}).catch((error) => {
			if (!disposed) console.warn('Background trailer controls unavailable', error);
		});
		return () => {
			disposed = true;
			if (backgroundPlayer === player) backgroundPlayer = null;
			player?.destroy();
		};
	});

	$effect(() => {
		heroInView;
		heroTrailerExpanded;
		heroTrailerEnded;
		syncBackgroundPlayback();
	});

	$effect(() => {
		if (!heroElement) return;
		const element = heroElement;
		const updateHeroVisibility = (nextInView: boolean) => {
			if (heroInView && !nextInView && showHeroVideo && !heroTrailerEnded) heroRestartOnReturn = true;
			if (!heroInView && nextInView && heroRestartOnReturn && !heroTrailerEnded) {
				heroRestartOnReturn = false;
				backgroundPlayer?.seekTo(0, true);
			}
			heroInView = nextInView;
		};
		const heroObserver = new IntersectionObserver(([entry]) => {
			updateHeroVisibility(!!entry?.isIntersecting && entry.intersectionRatio > 0.4 && !document.hidden);
		}, { root: element.closest('.content-area'), threshold: [0, 0.4] });
		heroObserver.observe(element);
		const onVisibilityChange = () => {
			if (document.hidden) updateHeroVisibility(false);
			else {
				const bounds = element.getBoundingClientRect();
				updateHeroVisibility(bounds.bottom > 0 && bounds.top < window.innerHeight * 0.6);
			}
		};
		document.addEventListener('visibilitychange', onVisibilityChange);
		return () => {
			heroObserver.disconnect();
			document.removeEventListener('visibilitychange', onVisibilityChange);
			heroInView = false;
		};
	});

	function closeHeroTrailer() {
		heroTrailerExpanded = false;
	}

	onMount(() => {
		desktopCatalog = isDesktopRuntime();
		const syncMotionPreference = () => {
			reduceMotionEnabled = document.documentElement.dataset.reduceMotion === 'true';
			if (reduceMotionEnabled) {
				showHeroVideo = false;
				heroVideoReady = false;
			}
		};
		syncMotionPreference();
		document.addEventListener('appearance-preferences-changed', syncMotionPreference);
		const refreshPlaybackHistory = () => {
			if (!desktopCatalog || document.visibilityState !== 'visible') return;
			void Promise.all([readContinueWatching(12), readPlaybackHistory(12)])
				.then(([resume, history]) => { resumeItems = resume; playbackHistoryItems = history; })
				.catch((error) => console.warn('Playback history could not be refreshed', error));
		};
		window.addEventListener(PLAYBACK_HISTORY_UPDATED_EVENT, refreshPlaybackHistory);
		window.addEventListener('focus', refreshPlaybackHistory);
		document.addEventListener('visibilitychange', refreshPlaybackHistory);
		if (desktopCatalog) {
			void Promise.all([
				readCatalogPage(0, 48, undefined, undefined, 'Recently added'),
				readContinueWatching(12),
				readPlaybackHistory(12)
			]).then(([page, resume, history]) => {
				if (page) catalogItems = page.items;
				else catalogError = 'The local library is only available in the desktop app.';
				resumeItems = resume;
				playbackHistoryItems = history;
			}).catch((error) => {
				catalogError = error instanceof Error ? error.message : 'The local library could not be read.';
			}).finally(() => (catalogLoading = false));
		} else {
			catalogLoading = false;
		}

		if (!desktopCatalog) {
			return () => {
				document.removeEventListener('appearance-preferences-changed', syncMotionPreference);
				window.removeEventListener(PLAYBACK_HISTORY_UPDATED_EVENT, refreshPlaybackHistory);
				window.removeEventListener('focus', refreshPlaybackHistory);
				document.removeEventListener('visibilitychange', refreshPlaybackHistory);
			};
		}

		const acrylicRequester = Symbol('Home HSS');
		const contentArea = homeContent.closest('.content-area');
		const observer = new IntersectionObserver(
			(entries) => {
				const entry = entries[entries.length - 1];
				if (!entry) return;
				requestNativeAcrylic(acrylicRequester,
					entry.isIntersecting &&
					entry.intersectionRect.width > 2 &&
					entry.intersectionRect.height > 2);
			},
			{
				root: contentArea,
				// Keep the native material out of the 44px window-controls overlay.
				rootMargin: '-44px 0px 0px 0px',
				threshold: 0
			}
		);
		observer.observe(homeContent);
		return () => {
			observer.disconnect();
			window.removeEventListener(PLAYBACK_HISTORY_UPDATED_EVENT, refreshPlaybackHistory);
			window.removeEventListener('focus', refreshPlaybackHistory);
			document.removeEventListener('visibilitychange', refreshPlaybackHistory);
			document.removeEventListener('appearance-preferences-changed', syncMotionPreference);
			requestNativeAcrylic(acrylicRequester, false);
		};
	});
</script>

<svelte:head><title>Home · Luma</title></svelte:head>
<svelte:window onkeydown={(event) => { if (event.key === 'Escape') closeHeroTrailer(); }} />

<div class="home-page" class:home-page--empty={!featured && !catalogLoading && (!!catalogError || (!catalogItems.length && !continueWatchingItems.length))}>
	{#if featured}
	<section
		class="featured"
		aria-labelledby="featured-title"
		bind:this={heroElement}
	>
		<div class="featured__backdrop" aria-hidden="true">
			{#if featuredBackdrop}<img use:recoverRemoteArtwork class="featured__backdrop-image" src={featuredBackdrop} alt="" />{/if}
		</div>
		{#if showHeroVideo && heroTrailer}
			<div class="featured__video" class:featured__video--ready={heroVideoReady} aria-hidden="true">
				<iframe
					bind:this={heroVideoIframe}
					src={backgroundTrailerUrl(heroTrailer.key)}
					title={`${featured.title} trailer background`}
					allow="autoplay; encrypted-media; picture-in-picture"
					tabindex="-1"
				></iframe>
			</div>
			<div class="featured__video-veil" aria-hidden="true"></div>
			<button
				class="featured__trailer-button"
				type="button"
				aria-label="Open full-screen trailer"
				onclick={openHeroTrailer}
			><span class="featured__trailer-play" aria-hidden="true"></span><span>{heroTrailer.isTeaser ? 'Click to watch teaser' : 'Click to watch full trailer'}</span></button>
		{/if}
		<div class="featured__content">
			<h1 id="featured-title" aria-label={featured.title}>
				{#if heroLogoUrl && !heroLogoFailed}
					<span class="featured__title-pending" aria-hidden="true">{featured.title}</span>
					<img
						class="featured__logo"
						class:featured__logo--ready={heroLogoReady}
						src={heroLogoUrl}
						alt=""
						onload={() => { heroLogoReady = true; }}
						onerror={() => { heroLogoFailed = true; heroLogoUrl = null; heroLogoResolved = true; }}
					/>
				{:else if heroLogoResolved}
					{featured.title}
				{:else}
					<span class="featured__title-pending" aria-hidden="true">{featured.title}</span>
				{/if}
			</h1>
			<div class="featured__meta">
				{#if featured.rating}<span class="match">{featured.rating} rating</span>{/if}
				{#if featured.year}<span>{featured.year}</span>{/if}
				<span>{featured.kind === 'series' ? 'Series' : 'Movie'}</span>
			</div>
			{#if featured.synopsis}<p class="featured__synopsis">{featured.synopsis}</p>{/if}
			<div class="featured__actions">
				<button class="button button--primary" style="corner-shape: squircle" onclick={() => player.open(featured)}>
					<Icon name="play" size={16} weight="fill" />
					{featured.progress ? 'Resume' : 'Play'}
				</button>
				<a class="button button--secondary" style="corner-shape: squircle" href={`/title/${featured.id}`}>
					<Icon name="info" size={18} weight="bold" />
					Details
				</a>
			</div>
		</div>
	</section>
	{/if}
	{#if heroTrailerExpanded && heroTrailer && featured}
		<div class="trailer-fullscreen" role="dialog" aria-modal="true" aria-label={`${featured.title} full trailer`}>
			<iframe
				src={`https://www.youtube-nocookie.com/embed/${heroTrailer.key}?autoplay=1&controls=1&playsinline=1&rel=0`}
				title={`${featured.title} trailer`}
				allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
				allowfullscreen
			></iframe>
			<button class="trailer-fullscreen__close" aria-label="Close full trailer" onclick={closeHeroTrailer}><Icon name="close" size={20} /></button>
		</div>
	{/if}

	<div class="home-content" bind:this={homeContent} data-native-backdrop={$nativeAcrylicStatus}>
		<div class="home-content__inner">
			{#if catalogLoading}
				<section class="home-section" aria-labelledby="library-heading" aria-busy="true">
					<div class="section-heading"><h2 id="library-heading">Your library</h2></div>
					<CatalogSkeletonGrid count={8} label="Loading your library" />
				</section>
			{:else if catalogError}
				<EmptyLibraryCard title="Library unavailable" description={catalogError} role="status" fullHeight />
			{:else if catalogItems.length || continueWatchingItems.length}
				{#if continueWatchingItems.length}
					<div class="home-section home-section--first">
						<MediaRow title="Continue watching" items={continueWatchingItems} showProgress />
					</div>
				{/if}

				{#if libraryItems.length}
					<div class="home-section home-section--library">
						<div class="section-heading">
							<h2>Your library</h2>
							<a href="/library" class="section-link">View all <Icon name="chevron-right" size={15} weight="bold" /></a>
						</div>
						{#if libraryPreview.length}
							<div class="poster-grid-preview__viewport" data-more-right="false">
								<div class="poster-grid-preview" use:smoothHorizontalScroll use:rightEdgeHint>
									{#each libraryPreview as item, index (item.id)}
										<PosterCard media={item} priority={index < 2} variant="home" />
									{/each}
								</div>
							</div>
						{/if}
					</div>
				{/if}

				{#if recentlyAdded.length}
					<div class="home-section home-section--last">
						<MediaRow title="Recently added" items={recentlyAdded} />
					</div>
				{/if}
			{:else}
				<EmptyLibraryCard
					title={desktopCatalog ? 'Your library is empty' : 'Your library is in the desktop app'}
					description={desktopCatalog ? 'Add a media folder in Settings to scan your videos and find matching artwork.' : 'Open the desktop app to browse your local collection.'}
					showSettingsLink={desktopCatalog}
					fullHeight
				/>
			{/if}
		</div>
	</div>
</div>

<style>
	.home-page {
		--featured-content-bottom: 64px;
		--featured-content-top: 44px;
		min-height: 100%;
		background: #080a0d;
	}
	:global(html[data-runtime='desktop']) .home-page { background: transparent; }

	.featured {
		position: relative;
		min-height: clamp(410px, 48vh, 500px);
		overflow: hidden;
		isolation: isolate;
	}

	.featured__backdrop {
		position: absolute;
		inset: 0;
		background: #080a0d;
		z-index: -2;
	}
	.featured__backdrop::after { position: absolute; z-index: 1; inset: 0; background-image: linear-gradient(90deg, rgba(3, 5, 8, 0.96) 0%, rgba(3, 5, 8, 0.72) 34%, rgba(3, 5, 8, 0.15) 68%, rgba(3, 5, 8, 0.08) 100%), linear-gradient(0deg, rgba(7, 9, 12, 0.98) 0%, rgba(7, 9, 12, 0.22) 34%, rgba(7, 9, 12, 0.08) 75%); content: ''; pointer-events: none; }
	.featured__backdrop-image { position: absolute; z-index: 0; inset: 0; display: block; width: 100%; height: 100%; object-fit: cover; object-position: center 25%; filter: saturate(0.84) contrast(1.04); transform: scale(1.008); }
	.featured__video { position: absolute; inset: 0; z-index: -1; overflow: hidden; opacity: 0; transition: opacity 850ms ease; pointer-events: none; }
	.featured__video--ready { opacity: 1; }
	.featured__video iframe { position: absolute; top: 50%; left: 50%; width: 100%; height: max(100%, 56.25vw); min-width: 780px; transform: translate(-50%, -50%); border: 0; pointer-events: none; }
	.featured__video-veil { position: absolute; inset: 0; z-index: -1; opacity: 0; background: linear-gradient(90deg, rgba(3,5,8,.94), rgba(3,5,8,.68) 34%, rgba(3,5,8,.12) 72%), linear-gradient(0deg, rgba(7,9,12,.96), transparent 68%); transition: opacity 850ms ease; pointer-events: none; }
	.featured__video--ready + .featured__video-veil { opacity: 1; }
	.featured__trailer-button { position: absolute; z-index: 1; right: 0; bottom: 9px; left: 0; display: flex; align-items: center; justify-content: center; gap: 5px; width: fit-content; min-height: 42px; margin: auto; padding: 0 15px; border: 1px solid rgba(255,255,255,.16); border-radius: 999px; color: rgba(245,247,250,.82); background: rgba(10,13,18,.34); backdrop-filter: blur(8px); font-family: inherit; font-size: .66rem; font-weight: 600; line-height: 1; letter-spacing: .015em; text-shadow: 0 1px 8px #000; cursor: pointer; }
	.featured__trailer-play { display: block; flex: 0 0 9px; width: 9px; height: 9px; background: currentColor; clip-path: polygon(0 0, 100% 50%, 0 100%); }
	.trailer-fullscreen { position: fixed; z-index: 120; inset: 0; display: grid; place-items: center; overflow: hidden; background: #000; }
	.trailer-fullscreen iframe { width: 100%; height: 100%; border: 0; }
	.trailer-fullscreen__close { position: absolute; top: max(16px, env(safe-area-inset-top)); right: max(16px, env(safe-area-inset-right)); display: grid; place-items: center; width: 42px; height: 42px; border: 1px solid rgba(255,255,255,.24); border-radius: 50%; color: white; background: rgba(16,20,25,.68); backdrop-filter: blur(14px); cursor: pointer; }

	.featured__content {
		display: grid;
		align-content: end;
		min-height: clamp(410px, 48vh, 500px);
		max-width: var(--content-width);
		margin: 0 auto;
		padding: var(--featured-content-top) var(--content-gutter) var(--featured-content-bottom);
	}

	.featured h1 {
		display: grid;
		max-width: 760px;
		margin: 0;
		color: var(--text-strong);
		font-family: var(--font-display);
		font-size: clamp(3.25rem, 5vw, 5.2rem);
		font-weight: 650;
		letter-spacing: -0.04em;
		line-height: 0.94;
		text-wrap: balance;
		text-shadow: 0 12px 42px rgba(0, 0, 0, 0.38);
	}
	.featured__title-pending, .featured__logo { grid-area: 1 / 1; }
	.featured__title-pending { visibility: hidden; }
	.featured__logo { display: block; width: auto; height: auto; max-width: min(480px, 100%); max-height: 104px; object-fit: contain; object-position: left center; filter: drop-shadow(0 12px 38px rgba(0, 0, 0, 0.4)); opacity: 0; transition: opacity 140ms ease; }
	.featured__logo--ready { opacity: 1; }

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

	.featured__actions { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 17px; }
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
		width: 100%;
		background: #101419;
		z-index: 2;
	}
	.home-content__inner {
		max-width: var(--content-width);
		margin: 0 auto;
		padding: 40px var(--content-gutter) 92px;
	}
	/* An empty Home must fill the viewport; percentage heights cannot resolve through min-height-only ancestors. */
	.home-page--empty { display: grid; min-height: 100dvh; }
	.home-page--empty .home-content { display: grid; }
	.home-page--empty .home-content__inner { display: grid; width: 100%; padding: 32px var(--content-gutter); }
	/* DWM supplies desktop Acrylic; keep this tint translucent so it shows through. */
	:global(html[data-runtime='desktop']) .home-content {
		background: var(--acrylic-content-tint);
		-webkit-backdrop-filter: none;
		backdrop-filter: none;
	}
	:global(html[data-runtime='desktop'] .home-content[data-native-backdrop='unavailable']) {
		background: var(--acrylic-content-fallback);
		-webkit-backdrop-filter: none;
		backdrop-filter: none;
	}
	.home-section + .home-section { margin-top: 52px; }
	.home-section--last { margin-top: 58px; }
	.section-heading { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
	.section-heading h2 { margin: 0; color: var(--text-strong); font-family: var(--font-display); font-size: 1.08rem; font-weight: 690; letter-spacing: -0.034em; }
	.section-link { display: inline-flex; align-items: center; gap: 4px; flex: 0 0 auto; color: var(--text-muted); font-size: 0.71rem; font-weight: 590; text-decoration: none; transition: color 140ms ease; }
	.section-link:hover { color: var(--text-strong); }
	.poster-grid-preview {
		--hover-clearance: 14px;
		--hover-inline-clearance: 14px;
		display: grid;
		grid-auto-columns: clamp(142px, 12.3vw, 198px);
		grid-auto-flow: column;
		gap: 17px;
		overflow-x: auto;
		overflow-y: hidden;
		overscroll-behavior-x: none;
		padding: calc(3px + var(--hover-clearance)) calc(3px + var(--hover-inline-clearance)) calc(16px + var(--hover-clearance));
		scroll-padding-inline: calc(3px + var(--hover-inline-clearance));
		scrollbar-width: none;
	}
	.poster-grid-preview__viewport { --hover-clearance: 14px; margin: calc(-1 * var(--hover-clearance)) -14px; }
	:global(.poster-grid-preview__viewport[data-more-right="true"] .poster-grid-preview) { -webkit-mask-image: linear-gradient(90deg, #000 calc(100% - 26px), transparent calc(100% - 12px)); mask-image: linear-gradient(90deg, #000 calc(100% - 26px), transparent calc(100% - 12px)); }
	.poster-grid-preview::-webkit-scrollbar { display: none; }
	@media (min-width: 761px) {
		:global(.home-content .media-row__viewport) {
			margin-right: -42px;
		}
		:global(.home-content .media-row__track) {
			padding-right: 45px;
			scroll-padding-right: 45px;
		}
		.poster-grid-preview__viewport {
			margin-right: -42px;
		}
		.poster-grid-preview {
			padding-right: 45px;
			scroll-padding-right: 45px;
		}
	}

	@media (max-width: 760px) {
		.home-page { --featured-content-bottom: 48px; --featured-content-top: 30px; }
		.featured { min-height: 360px; }
		.featured__backdrop::after { background-image: linear-gradient(0deg, rgba(7, 9, 12, 0.97) 0%, rgba(7, 9, 12, 0.64) 35%, rgba(7, 9, 12, 0.06) 73%); }
		.featured__backdrop-image { object-position: 60% center; filter: saturate(0.83) contrast(1.04); }
		.featured__content { min-height: 360px; padding: var(--featured-content-top) 18px var(--featured-content-bottom); }
		.featured__trailer-button { bottom: 8px; }
		.featured h1 { max-width: 94%; font-size: clamp(2.75rem, 12vw, 3.6rem); font-weight: 650; line-height: 0.94; }
		.featured__logo { max-width: 94%; max-height: 92px; }
		.featured__meta { margin-top: 17px; font-size: 0.67rem; }
		.featured__meta span:nth-child(4) { display: none; }
		.featured__synopsis { display: -webkit-box; max-width: 96%; margin-top: 17px; overflow: hidden; font-size: 0.76rem; line-height: 1.52; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
		.featured__actions { margin-top: 17px; }
		.button { min-height: 40px; padding: 0 15px; }
		.button--secondary { backdrop-filter: blur(12px); }
		.home-content__inner { padding: 31px 18px 80px; }
		.home-section + .home-section { margin-top: 43px; }
		.home-section--last { margin-top: 48px; }
		.section-heading { margin-bottom: 14px; }
		.section-heading h2 { font-size: 1rem; }
		.poster-grid-preview__viewport { margin-right: -18px; }
		.poster-grid-preview { grid-auto-columns: 42vw; gap: 12px; padding-right: 18px; }
	}

	@media (prefers-reduced-motion: reduce) {
		.button,
		.section-link { transition: none; }
		.button:hover,
		.button:focus-visible { transform: none; }
		.featured__video, .featured__video-veil { transition: none; }
	}

	@media (prefers-reduced-transparency: reduce) {
		.button--secondary { background: var(--surface-3); backdrop-filter: none; }
		.home-content { background: #0c0f13; }
		:global(html[data-runtime='desktop']) .home-content {
			background: #0c0f13;
			-webkit-backdrop-filter: none;
			backdrop-filter: none;
		}
	}
</style>
