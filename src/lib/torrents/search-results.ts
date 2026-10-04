export type SearchSource = '1337x' | 'YTS' | 'EZTV';
export type SearchRelease = {
	name: string; source: SearchSource; uploader: string; size: string; sizeBytes: number;
	seeds: number; peers: number; url: string; filename?: string; infoHash?: string;
	quality?: string; codec?: string; releaseType?: string; fileType?: string | null; group?: string;
};

export function releaseDetails(item: SearchRelease) {
	const text = `${item.name} ${item.filename || ''}`;
	const resolution = item.quality || text.match(/\b(4320p|2160p|1440p|1080[pi]|720p|576p|480p|360p|4k|8k)\b/i)?.[1] || '';
	const quality = /^(2160p|4k)$/i.test(resolution) ? '4K' : /^(4320p|8k)$/i.test(resolution) ? '8K' : resolution.toLowerCase() || 'Não informada';
	// Container format must come from actual metadata or an explicit extension, never the codec.
	const fileType = (item.fileType || text.match(/\.(mkv|mp4|avi|mov|m4v|webm|ts|wmv)(?=$|[\s\]\)])/i)?.[1] || '').toUpperCase() || 'Não informado';
	const codec = item.codec || text.match(/\b(?:x26[45]|h[ .]?26[45]|hevc|av1|xvid)\b/i)?.[0] || '';
	const group = item.group || text.replace(/\s+EZTV\s*$/i, '').replace(/\.(mkv|mp4|avi|mov|m4v|webm|ts|wmv)$/i, '').match(/-([\w]+)(?:\s*\[[^\]]*\])?\s*$/)?.[1] || '';
	return { quality, fileType, codec, group };
}

export function releaseKey(item: SearchRelease) {
	return `${item.source}:${item.infoHash || `${item.url}:${item.name}:${item.sizeBytes}`}`;
}
