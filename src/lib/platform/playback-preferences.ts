export type PreferredTrackLanguage = 'system' | 'en' | 'pt' | 'ja';
export type PreferredSubtitleLanguage = 'auto' | 'off' | 'en' | 'pt' | 'ja';
export const subtitleFontOptions = [
	{ value: 'Manrope', label: 'Manrope · App' },
	{ value: 'Arial', label: 'Arial' },
	{ value: 'Segoe UI', label: 'Segoe UI' },
	{ value: 'Verdana', label: 'Verdana' },
	{ value: 'Tahoma', label: 'Tahoma' }
] as const;
export type SubtitleFont = (typeof subtitleFontOptions)[number]['value'];

export type PlaybackPreferences = {
	autoplayNextEpisode: boolean;
	markPreviousEpisodesWatched: boolean;
	audioLanguage: PreferredTrackLanguage;
	subtitleLanguage: PreferredSubtitleLanguage;
	subtitleFont: SubtitleFont;
	subtitleSize: number;
	subtitlePosition: number;
};

const storageKey = 'media-library.playback-preferences';
const defaults: PlaybackPreferences = {
	autoplayNextEpisode: true,
	markPreviousEpisodesWatched: false,
	audioLanguage: 'system',
	subtitleLanguage: 'auto',
	subtitleFont: 'Manrope',
	subtitleSize: 100,
	subtitlePosition: 0
};

export function readPlaybackPreferences(): PlaybackPreferences {
	try {
		const saved = JSON.parse(localStorage.getItem(storageKey) ?? '{}') as Partial<PlaybackPreferences>;
		return {
			autoplayNextEpisode: typeof saved.autoplayNextEpisode === 'boolean' ? saved.autoplayNextEpisode : defaults.autoplayNextEpisode,
			markPreviousEpisodesWatched: typeof saved.markPreviousEpisodesWatched === 'boolean' ? saved.markPreviousEpisodesWatched : defaults.markPreviousEpisodesWatched,
			audioLanguage: isAudioLanguage(saved.audioLanguage) ? saved.audioLanguage : defaults.audioLanguage,
			subtitleLanguage: isSubtitleLanguage(saved.subtitleLanguage) ? saved.subtitleLanguage : defaults.subtitleLanguage,
			subtitleFont: isSubtitleFont(saved.subtitleFont) ? saved.subtitleFont : defaults.subtitleFont,
			subtitleSize: snapToStep(saved.subtitleSize, 70, 150, 10, defaults.subtitleSize),
			subtitlePosition: snapToStep(saved.subtitlePosition, 0, 12, 1.5, defaults.subtitlePosition)
		};
	} catch {
		return { ...defaults };
	}
}

export function updatePlaybackPreference<K extends keyof PlaybackPreferences>(key: K, value: PlaybackPreferences[K]): void {
	const next = { ...readPlaybackPreferences(), [key]: value };
	try { localStorage.setItem(storageKey, JSON.stringify(next)); }
	catch { /* Keep playback usable if browser storage is unavailable. */ }
	if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('media-library:playback-preferences', { detail: next }));
}

function clamp(value: unknown, min: number, max: number, fallback: number): number {
	return typeof value === 'number' && Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : fallback;
}

function snapToStep(value: unknown, min: number, max: number, step: number, fallback: number): number {
	if (typeof value !== 'number' || !Number.isFinite(value)) return fallback;
	const bounded = Math.min(max, Math.max(min, value));
	return min + Math.round((bounded - min) / step) * step;
}

function isAudioLanguage(value: unknown): value is PreferredTrackLanguage {
	return value === 'system' || value === 'en' || value === 'pt' || value === 'ja';
}

function isSubtitleLanguage(value: unknown): value is PreferredSubtitleLanguage {
	return value === 'auto' || value === 'off' || value === 'en' || value === 'pt' || value === 'ja';
}

export function isSubtitleFont(value: unknown): value is SubtitleFont {
	return subtitleFontOptions.some((font) => font.value === value);
}
