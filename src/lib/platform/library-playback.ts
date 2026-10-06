import type { CatalogMedia, LocalTitleDetail, ResolvedMediaFile } from '../types';

export function verifyPlaybackIdentity(source: ResolvedMediaFile, uuid?: string): ResolvedMediaFile {
	if (uuid && source.playbackUuid !== uuid) throw new Error('The computer could not confirm this media file. Refresh the library and try again.');
	return source;
}

type Page = { items: CatalogMedia[]; total: number; offset: number };
type LibraryReader = {
	detail: (id: number) => Promise<LocalTitleDetail | null>;
	page: (offset: number, count: number, query?: string) => Promise<Page>;
};

/** IDs can change after a scan, and an episode ID is not always a grouped series ID. */
export async function identifyLibraryPlayback(
	reader: LibraryReader, mediaId: number, title: string, episodeLabel?: string,
	tmdbId?: number, kind?: 'movie' | 'series'
): Promise<number> {
	const normalize = (value: string) => value.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase().replace(/[^\p{L}\p{N}]/gu, '');
	const marker = episodeLabel?.match(/\bS(\d+)\s*[·.\-–:]?\s*E(\d+)\b/i);
	const matchesTitle = (detail: LocalTitleDetail) =>
		(!kind || !detail.media.kind || detail.media.kind === kind) &&
		(tmdbId && detail.tmdbId ? tmdbId === detail.tmdbId : normalize(detail.media.title) === normalize(title));
	const matches = new Set<number>();
	const inspect = (detail: LocalTitleDetail | null) => {
		if (!detail || !matchesTitle(detail)) return;
		const files = marker ? detail.files.filter(file => file.season === Number(marker[1]) && file.episode === Number(marker[2])) : detail.files;
		// Prefer the exact existing file, including Continue Watching's episode ID.
		if (files.some(file => file.mediaId === mediaId)) { matches.clear(); matches.add(mediaId); return; }
		// Legacy entries without UUID may verify their exact ID, but never switch
		// to another file based on its title/episode or a single search result.
	};
	inspect(await reader.detail(mediaId));
	if (matches.size === 1) return [...matches][0];
	const visited = new Set<number>([mediaId]);
	const scan = async (query?: string) => {
		let offset = 0;
		while (true) {
			const page = await reader.page(offset, 200, query);
			for (const item of page.items) {
				if (item.id >= 1_000_000_000 || visited.has(item.id)) continue;
				if (kind && item.kind && item.kind !== kind) continue;
				if (!tmdbId && normalize(item.title) !== normalize(title)) continue;
				visited.add(item.id);
				inspect(await reader.detail(item.id));
				if (matches.has(mediaId)) return;
			}
			offset += page.items.length;
			if (!page.items.length || offset >= page.total) return;
		}
	};
	await scan(title);
	// An API title may differ from the indexed/localized title. TMDB identity wins.
	if (!matches.size) await scan();
	if (matches.size !== 1) throw new Error('Couldn’t identify this video in your computer’s current library.');
	return [...matches][0];
}
