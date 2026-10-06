import type { DownloadTarget } from '$lib/platform/mobile-connection';

/** Ignore async queue results after the user switches between PC and phone. */
export function isCurrentTorrentTarget(requestTarget: DownloadTarget, selectedTarget: DownloadTarget): boolean {
	return requestTarget === selectedTarget;
}

export function formatTorrentTargetError(target: DownloadTarget, message: string): string {
	return `${target === 'phone' ? 'This phone' : 'Computer'}: ${message}`;
}
