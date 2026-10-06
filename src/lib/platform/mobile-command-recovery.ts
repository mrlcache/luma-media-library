const playbackReads = new Set(['desktop_bootstrap', 'resolve_media_file', 'get_local_title_detail', 'get_catalog_page']);

export async function recoverMobileRead<T>(command: string, send: () => Promise<T>): Promise<T> {
	try { return await send(); }
	catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		if (!playbackReads.has(command) || !/Cannot reach Luma Desktop|error sending request|connection (?:refused|reset|closed)|network (?:error|request failed)|failed to fetch|load failed|networkerror|timed? ?out|timeout/i.test(message)) throw error;
		// Retry only reads: a dropped reply must never duplicate a download or edit.
		await new Promise(resolve => setTimeout(resolve, 150));
		return send();
	}
}

export function mobileMediaOrigin<T>(command: string, value: T, connectionUrl: string | null): T {
	if (command !== 'resolve_media_file' || !connectionUrl || !value || typeof value !== 'object') return value;
	let origin: URL;
	try { origin = new URL(connectionUrl); } catch { return value; }
	if (!/^https?:$/.test(origin.protocol) || origin.username || origin.password) return value;
	const mediaUrl = (path: unknown) => {
		if (typeof path !== 'string') return path;
		try {
			const url = new URL(path);
			if (!/^https?:$/.test(url.protocol) || !/^\/api\/v1\/media\/\d+(?:\/|$)/.test(url.pathname) || !url.searchParams.has('signature') || !url.searchParams.has('expires')) return path;
			// Commands already reached this computer; default-route discovery can
			// advertise another adapter (VPN, Ethernet) for the media response.
			return new URL(url.pathname + url.search, origin.origin).href;
		} catch { return path; }
	};
	const source = value as Record<string, unknown>;
	return {
		...source,
		path: mediaUrl(source.path),
		streamUrl: mediaUrl(source.streamUrl),
		...(Array.isArray(source.subtitles) ? { subtitles: source.subtitles.map(subtitle => subtitle && typeof subtitle === 'object' ? { ...subtitle, path: mediaUrl(subtitle.path) } : subtitle) } : {})
	} as T;
}
