export type AppearancePreference = 'reduceTransparency' | 'reduceMotion';

export type AppearancePreferences = Record<AppearancePreference, boolean>;

const storageKeys: Record<AppearancePreference, string> = {
	reduceTransparency: 'media-library.reduce-transparency',
	reduceMotion: 'media-library.reduce-motion'
};

export function readAppearancePreferences(): AppearancePreferences {
	const read = (preference: AppearancePreference) => {
		try {
			return localStorage.getItem(storageKeys[preference]) === 'true';
		} catch {
			return false;
		}
	};

	return {
		reduceTransparency: read('reduceTransparency'),
		reduceMotion: read('reduceMotion')
	};
}

export function applyAppearancePreferences(preferences = readAppearancePreferences()) {
	if (typeof document === 'undefined') return;
	document.documentElement.dataset.reduceTransparency = String(preferences.reduceTransparency);
	document.documentElement.dataset.reduceMotion = String(preferences.reduceMotion);
	document.dispatchEvent(new Event('appearance-preferences-changed'));
}

export function updateAppearancePreference(preference: AppearancePreference, enabled: boolean) {
	try {
		localStorage.setItem(storageKeys[preference], String(enabled));
	} catch {
		// The current session still receives the preference if persistent storage is unavailable.
	}
	applyAppearancePreferences(readAppearancePreferences());
}
