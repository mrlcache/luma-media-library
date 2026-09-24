export function tmdbImageSize(url: string | null | undefined, size: 'w500' | 'w780' | 'w1280' | 'original'): string {
	if (!url) return '';
	try {
		const parsed = new URL(url);
		if (parsed.hostname !== 'image.tmdb.org') return url;
		const parts = parsed.pathname.split('/');
		const sizeIndex = parts.indexOf('t');
		if (parts[sizeIndex + 1] !== 'p' || !/^(?:w\d+|original)$/.test(parts[sizeIndex + 2] ?? '')) return url;
		parts[sizeIndex + 2] = size;
		parsed.pathname = parts.join('/');
		return parsed.toString();
	} catch {
		return url;
	}
}
