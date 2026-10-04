const storageKey = 'luma.mobile.transcoding';
export const bitratePresets: Record<string, number[]> = {
	'480p': [800000, 1200000, 2000000],
	'720p': [1500000, 2500000, 4000000],
	'1080p': [3000000, 5000000, 8000000]
};
export type TranscodePreferences = { enabled: boolean; quality: string; bitrate: number };
export function readTranscodePreferences(): TranscodePreferences {
	const defaults = { enabled: false, quality: 'auto', bitrate: 0 };
	try {
		const saved = JSON.parse(localStorage.getItem(storageKey) ?? '{}');
		const quality = saved.quality === 'auto' || Object.hasOwn(bitratePresets, saved.quality) ? saved.quality : 'auto';
		const bitrate = bitratePresets[quality]?.includes(saved.bitrate) ? saved.bitrate : (bitratePresets[quality]?.[1] ?? 0);
		return { enabled: saved.enabled === true, quality, bitrate };
	} catch { return defaults; }
}
export function saveTranscodePreferences(preferences: TranscodePreferences) {
	try { localStorage.setItem(storageKey, JSON.stringify(preferences)); }
	catch { /* Playback still works when persistent storage is unavailable. */ }
}
