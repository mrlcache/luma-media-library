import { error } from '@sveltejs/kit';
import { getMedia } from '$lib/data';
import { tmdbImageSize } from '$lib/media/artwork';
import { isDesktopRuntime, readLocalTitleDetail } from '$lib/platform/desktop';
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
		const localDetail = await readLocalTitleDetail(Number(params.slug));
		if (localDetail) {
			const record = localDetail.media;
			const item: MediaItem = {
				id: String(record.id),
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
