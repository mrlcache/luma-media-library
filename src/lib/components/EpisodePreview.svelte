<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { extractOpeningThumbnail } from '$lib/media/episode-thumbnails';
	import { findTvMazeEpisodeDetails } from '$lib/media/tvmaze-episodes';
	import { localMediaUrl } from '$lib/platform/desktop';

	type Props = {
		path: string;
		label: string;
		showTitle: string;
		showYear: number;
		season: number | null;
		episode: number | null;
	};
	let { path, label, showTitle, showYear, season, episode }: Props = $props();
	let host: HTMLDivElement;
	let thumbnail = $state('');
	let checked = $state(false);
	let cancelled = false;
	let localFallbackStarted = false;

	async function loadLocalThumbnail() {
		if (localFallbackStarted || cancelled) return;
		localFallbackStarted = true;
		try {
			const source = await localMediaUrl(path);
			const result = await extractOpeningThumbnail(source, path);
			if (!cancelled) thumbnail = result ?? '';
		} catch {
			if (!cancelled) thumbnail = '';
		} finally {
			if (!cancelled) checked = true;
		}
	}

	function handleImageError() {
		if (thumbnail.startsWith('data:image/')) {
			thumbnail = '';
			checked = true;
			return;
		}
		thumbnail = '';
		checked = false;
		void loadLocalThumbnail();
	}

	onMount(() => {
		let started = false;
		const load = async () => {
			if (started) return;
			started = true;
			try {
				const remote = await findTvMazeEpisodeDetails(showTitle, showYear || null, season, episode);
				if (cancelled) return;
				if (remote?.image) {
					thumbnail = remote.image;
					checked = true;
					return;
				}
				await loadLocalThumbnail();
			} catch {
				await loadLocalThumbnail();
			} finally {
				if (!cancelled) checked = true;
			}
		};
		if (typeof IntersectionObserver === 'undefined') void load();
		else {
			const observer = new IntersectionObserver((entries) => {
				if (entries.some((entry) => entry.isIntersecting)) {
					observer.disconnect();
					void load();
				}
			}, { rootMargin: '260px 0px' });
			observer.observe(host);
			return () => {
				cancelled = true;
				observer.disconnect();
			};
		}
		return () => { cancelled = true; };
	});
</script>

<div class="episode-thumb" bind:this={host} aria-label={label}>
	{#if thumbnail}
		<img src={thumbnail} alt={`Preview for ${label}`} width="480" height="270" loading="lazy" decoding="async" onerror={handleImageError} />
	{:else if checked}
		<span class="episode-thumb__fallback" ><Icon name="film" size={20} /></span>
	{:else}
		<span class="episode-thumb__fallback episode-thumb__fallback--loading" aria-hidden="true"><Icon name="film" size={20} /></span>
	{/if}
</div>

<style>
	.episode-thumb { position: relative; overflow: hidden; aspect-ratio: 16 / 9; border: 1px solid rgba(255,255,255,0.1); border-radius: 9px; background: var(--surface-2); }
	.episode-thumb img { display: block; width: 100%; height: 100%; object-fit: cover; }
	.episode-thumb__fallback { display: grid; width: 100%; height: 100%; place-items: center; color: var(--text-dim); }
	.episode-thumb__fallback--loading { background: linear-gradient(110deg, var(--surface-2) 20%, var(--surface-3) 38%, var(--surface-2) 56%); background-size: 220% 100%; animation: episode-preview-shimmer 1.8s linear infinite; }
	@keyframes episode-preview-shimmer { to { background-position-x: -220%; } }
	@media (prefers-reduced-motion: reduce) { .episode-thumb__fallback--loading { animation: none; } }
</style>
