export type DesktopBootstrap = {
	version: string;
	platform: string;
	mediaCoreStatus: 'not-configured';
};

function isTauriRuntime(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function readDesktopBootstrap(): Promise<DesktopBootstrap | null> {
	if (!isTauriRuntime()) return null;

	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<DesktopBootstrap>('desktop_bootstrap');
}
