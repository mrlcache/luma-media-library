export type MediaKind = 'movie' | 'series';

export type TmdbSearchResult = {
	id: number;
	title: string;
	kind: MediaKind;
	year: number | null;
	overview: string;
	voteAverage: number | null;
	posterUrl: string | null;
	backdropUrl: string | null;
};

export type TmdbTrailer = { key: string; name: string; isTeaser?: boolean };

export type CatalogMedia = {
	id: number;
	playbackUuid?: string;
	title: string;
	extension: string;
	sizeBytes: number;
	modifiedAt: number | null;
	kind: MediaKind | null;
	year: number | null;
	overview: string | null;
	voteAverage: number | null;
	posterUrl: string | null;
	backdropUrl: string | null;
};

export type LocalEpisodeFile = {
	mediaId: number;
	playbackUuid?: string;
	fileName: string;
	path: string;
	season: number | null;
	episode: number | null;
};

export type LocalTitleDetail = {
	tmdbId?: number | null;
	media: CatalogMedia;
	files: LocalEpisodeFile[];
	extras?: LocalEpisodeFile[];
	watchedBefore?: { season: number; episode: number } | null;
};

export type ContinueWatchingItem = {
	id: number;
	playbackId: number;
	playbackUuid?: string;
	title: string;
	kind: MediaKind | null;
	year: number | null;
	overview: string | null;
	voteAverage: number | null;
	posterUrl: string | null;
	backdropUrl: string | null;
	positionSeconds: number;
	durationSeconds: number;
	updatedAt: number;
};

export type PlaybackHistoryItem = {
	id: number;
	title: string;
	kind: 'movie' | 'series' | null;
	year: number | null;
	overview: string | null;
	voteAverage: number | null;
	posterUrl: string | null;
	backdropUrl: string | null;
	updatedAt: number;
};

export type SubtitleFileSource = { label: string; path: string; language?: string; streamIndex?: number | null; supported?: boolean };
export type ResolvedMediaFile = { path: string; subtitles: SubtitleFileSource[]; resumePositionSeconds: number; mediaId?: number; playbackUuid?: string; subtitleWarning?: string | null };

export type OpenSubtitleSearchResult = {
	fileId: number;
	release: string;
	language: string;
	downloads: number;
	hearingImpaired: boolean;
	uploader: string | null;
};

export type Episode = {
	id: string;
	title: string;
	number: number;
	duration: string;
	summary: string;
	thumbnail: string;
	progress?: number;
};

export type MediaItem = {
	playbackUuid?: string;
	installed?: boolean;
	playbackId?: number;
	tmdbId?: number;
	id: string;
	title: string;
	kind: MediaKind;
	year: number;
	genres: string[];
	rating: string;
	runtime: string;
	poster: string;
	backdrop: string;
	synopsis: string;
	match: string;
	progress?: number;
	progressLabel?: string;
	seasons?: number;
	episodes?: Episode[];
	episodeLabel?: string;
	nextEpisode?: MediaItem;
};
