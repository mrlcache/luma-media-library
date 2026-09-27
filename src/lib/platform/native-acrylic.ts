import { writable } from 'svelte/store';
import { isDesktopRuntime } from './desktop';

type AcrylicStatus = 'idle' | 'ready' | 'unavailable';

export const nativeAcrylicStatus = writable<AcrylicStatus>('idle');

const requesters = new Set<symbol>();
let suspendedForPlayback = false;
let applied = false;
let updating = false;
let listening = false;
let loggedFailure = false;

function shouldEnable() {
	return requesters.size > 0 &&
		!suspendedForPlayback &&
		document.visibilityState === 'visible' &&
		document.documentElement.dataset.reduceTransparency !== 'true' &&
		!window.matchMedia('(prefers-reduced-transparency: reduce)').matches;
}

function listenForSystemChanges() {
	if (listening) return;
	listening = true;
	document.addEventListener('visibilitychange', () => void updateNativeAcrylic());
	document.addEventListener('appearance-preferences-changed', () => void updateNativeAcrylic());
	window.matchMedia('(prefers-reduced-transparency: reduce)')
		.addEventListener('change', () => void updateNativeAcrylic());
}

async function updateNativeAcrylic() {
	if (updating || !isDesktopRuntime()) return;
	updating = true;
	try {
		const { invoke } = await import('@tauri-apps/api/core');
		while (applied !== shouldEnable()) {
			const enabled = shouldEnable();
			try {
				await invoke('set_hss_acrylic_enabled', { enabled });
				applied = enabled;
				nativeAcrylicStatus.set(enabled ? 'ready' : 'idle');
				loggedFailure = false;
			} catch (error) {
				applied = enabled;
				nativeAcrylicStatus.set('unavailable');
				if (!loggedFailure) console.warn('Native Acrylic is unavailable.', error);
				loggedFailure = true;
				break;
			}
		}
	} catch (error) {
		applied = shouldEnable();
		nativeAcrylicStatus.set('unavailable');
		if (!loggedFailure) console.warn('Native Acrylic could not start.', error);
		loggedFailure = true;
	} finally {
		updating = false;
		if (applied !== shouldEnable()) void updateNativeAcrylic();
	}
}

export function requestNativeAcrylic(requester: symbol, enabled: boolean) {
	if (!isDesktopRuntime()) return;
	listenForSystemChanges();
	if (enabled) requesters.add(requester);
	else requesters.delete(requester);
	void updateNativeAcrylic();
}

export function suspendNativeAcrylicForPlayback(suspended: boolean) {
	if (!isDesktopRuntime()) return;
	suspendedForPlayback = suspended;
	void updateNativeAcrylic();
}
