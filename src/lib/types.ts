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

export type CatalogMedia = {
	id: number;
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
};
