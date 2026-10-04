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

const cachedArtwork = new Map<string, Promise<string>>();

async function fetchArtworkThroughDesktop(url: string): Promise<string> {
	let pending = cachedArtwork.get(url);
	if (!pending) {
		pending = import('$lib/platform/invoke').then(({ invoke }) =>
			invoke<string>('load_remote_artwork', { url })
		).catch((error) => {
			cachedArtwork.delete(url);
			throw error;
		});
		cachedArtwork.set(url, pending);
		while (cachedArtwork.size > 32) {
			const oldest = cachedArtwork.keys().next().value;
			if (!oldest) break;
			cachedArtwork.delete(oldest);
		}
	}
	return pending;
}

/** Retry failed WebView poster requests through the native HTTP client. */
export function recoverRemoteArtwork(image: HTMLImageElement) {
	const attempted = new Set<string>();
	const handleError = () => {
		if (!('__TAURI_INTERNALS__' in window)) return;
		const source = image.currentSrc || image.src;
		if (!source.startsWith('https://') || attempted.has(source)) return;
		attempted.add(source);
		void fetchArtworkThroughDesktop(source).then((dataUrl) => {
			if (image.isConnected && (image.currentSrc || image.src) === source) image.src = dataUrl;
		}).catch(() => {
			// Keep the card layout usable when both image transports fail.
		});
	};
	image.addEventListener('error', handleError);
	// Eager resources can fail before Svelte mounts the action during hydration.
	queueMicrotask(() => {
		if (image.complete && image.naturalWidth === 0) handleError();
	});
	return { destroy: () => image.removeEventListener('error', handleError) };
}
