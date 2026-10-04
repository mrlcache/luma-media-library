import type { LocalEpisodeFile, LocalTitleDetail } from '../types';
import type { TvMazeEpisodeDetails } from './tvmaze-episodes';

export type SeriesEpisode = {
	id: string;
	season: number;
	episode: number | null;
	title: string;
	summary: string | null;
	image: string | null;
	runtime: number | null;
	file: LocalEpisodeFile | null;
	watched: boolean;
};

export function mergeSeriesEpisodes(
	files: LocalEpisodeFile[], metadata: TvMazeEpisodeDetails[], watchedBefore: LocalTitleDetail['watchedBefore']
): SeriesEpisode[] {
	const rows = new Map<string, SeriesEpisode>();
	for (const item of metadata) {
		const id = `${item.season}:${item.episode}`;
		rows.set(id, { ...item, id, file: null, watched: false });
	}
	for (const file of files) {
		const season = file.season ?? 1;
		const id = file.episode === null ? `file:${file.mediaId}` : `${season}:${file.episode}`;
		const existing = rows.get(id);
		if (existing) {
			if (!existing.file) existing.file = file;
		} else {
			rows.set(id, { id, season, episode: file.episode,
				title: file.episode === null ? 'Episode' : `Episode ${file.episode}`,
				summary: null, image: null, runtime: null, file, watched: false });
		}
	}
	for (const row of rows.values()) {
		row.watched = !!watchedBefore && row.episode !== null && (row.season < watchedBefore.season ||
			(row.season === watchedBefore.season && row.episode < watchedBefore.episode));
	}
	return [...rows.values()].sort((a, b) => a.season - b.season ||
		(a.episode ?? Number.MAX_SAFE_INTEGER) - (b.episode ?? Number.MAX_SAFE_INTEGER));
}
