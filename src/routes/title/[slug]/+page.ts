import { error } from '@sveltejs/kit';
import { getMedia } from '$lib/data';
import { tmdbImageSize } from '$lib/media/artwork';
import { isDesktopRuntime, readContinueWatching, readLocalTitleDetail } from '$lib/platform/desktop';
import type { MediaItem } from '$lib/types';
import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { libraryTransfers, transferCard } from '$lib/torrents/library';
import { readDiscoveryTitle } from '$lib/media/discovery';
import { savedFavorite } from '$lib/media/favorites';

export const load: PageLoad = async ({ params }) => {
	if (params.slug.startsWith('tmdb-')) {
		const item = await readDiscoveryTitle(params.slug).catch(() => null) ?? savedFavorite(params.slug);
		if (!item) error(404, 'Title not found');
		return { item, localDetail: null };
	}
	if (params.slug.startsWith('download-')) {
		const transfer = get(libraryTransfers).find((item) => `download-${item.infoHash}` === params.slug);
		if (!transfer) error(404, 'Title not found');
		const record = transferCard(transfer);
		const item: MediaItem = { id: String(record.id), title: record.title, kind: record.kind === 'series' ? 'series' : 'movie', year: record.year ?? 0,
			genres: [], rating: record.voteAverage?.toFixed(1) ?? '', runtime: '', poster: tmdbImageSize(record.posterUrl, 'w780'),
			backdrop: tmdbImageSize(record.backdropUrl ?? record.posterUrl, 'original'), synopsis: record.overview ?? '', match: '' };
		return { item, localDetail: null, transfer };
	}
	if (/^\d+$/.test(params.slug) && isDesktopRuntime()) {
		const mediaId = Number(params.slug);
		const localDetail = await readLocalTitleDetail(mediaId).catch(() => null);
		if (localDetail) {
			const record = localDetail.media;
			const item: MediaItem = {
				id: String(record.id),
				playbackUuid: record.playbackUuid,
				tmdbId: localDetail.tmdbId ?? undefined,
				title: record.title,
				kind: record.kind === 'series' ? 'series' : 'movie',
				year: record.year ?? 0,
				genres: [],
				rating: record.voteAverage?.toFixed(1) ?? '',
				runtime: '',
				poster: tmdbImageSize(record.posterUrl, 'w780'),
				backdrop: tmdbImageSize(record.backdropUrl ?? record.posterUrl, 'original'),
				synopsis: record.overview ?? '',
				match: ''
			};
			return { item, localDetail };
		}
		// A resumed computer title can be opened on a paired phone even when the
		// phone's own library does not contain that file. Keep the item resolvable
		// from the playback row instead of falling through to the sample catalog.
		const resume = await readContinueWatching(24).catch(() => []);
		const item = resume.find((entry) => entry.id === mediaId);
		if (item) {
			const resumedItem: MediaItem = {
					id: String(item.id), title: item.title, kind: item.kind === 'series' ? 'series' : 'movie',
					installed: true, playbackId: item.playbackId, playbackUuid: item.playbackUuid,
					year: item.year ?? 0, genres: [], rating: item.voteAverage?.toFixed(1) ?? '', runtime: '',
					poster: tmdbImageSize(item.posterUrl, 'w780'),
					backdrop: tmdbImageSize(item.backdropUrl ?? item.posterUrl, 'original'),
					synopsis: item.overview ?? '', match: ''
			};
			return { item: resumedItem, localDetail: null };
		}
	}

	const item = savedFavorite(params.slug) ?? getMedia(params.slug);
	if (!item) error(404, 'Title not found');
	return {
		item: {
			...item,
			poster: tmdbImageSize(item.poster, 'w780'),
			backdrop: tmdbImageSize(item.backdrop, 'original')
		},
		localDetail: null
	};
};
