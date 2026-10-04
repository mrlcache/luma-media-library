import { dev } from '$app/environment';

// This selects the development handset shell, not the platform's capabilities.
export function isMobilePreview(): boolean {
	if (!dev || typeof window === 'undefined') return false;
	try {
		if (new URL(window.location.href).searchParams.get('mobile') === '1') sessionStorage.setItem('luma.mobile-preview', 'true');
		return sessionStorage.getItem('luma.mobile-preview') === 'true';
	} catch { return new URL(window.location.href).searchParams.get('mobile') === '1'; }
}
