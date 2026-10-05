// A fragmented stream's duration is not the original file's duration.
export function playbackPosition(streamTime: number, offset: number, duration: number): number {
	const position = Math.max(0, streamTime + offset);
	return duration > 0 ? Math.min(duration, position) : position;
}
export function resumePlaybackPosition(position: number, duration: number): number {
	if (!Number.isFinite(position) || position < 0) return 0;
	if (duration > 0 && (position >= duration - 20 || position / duration >= .95)) return 0;
	return position;
}
export function reachedPlaybackEnd(position: number, duration: number): boolean {
	return Number.isFinite(position) && Number.isFinite(duration) && duration > 0 && position >= duration - 2;
}
