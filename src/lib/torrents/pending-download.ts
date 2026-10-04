import type { TorrentRelease } from './releases';

export type PendingDownload = Pick<TorrentRelease, 'name' | 'infoHash' | 'downloadKey'>;
let pending: PendingDownload | null = null;

export function prepareDownload(release: PendingDownload) {
	pending = { name: release.name, infoHash: release.infoHash, downloadKey: release.downloadKey };
}

export function takePreparedDownload(): PendingDownload | null {
	const selected = pending;
	pending = null;
	return selected;
}
