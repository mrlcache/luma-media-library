export type MediaKind = 'movie' | 'series';

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
