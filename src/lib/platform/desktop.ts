export type DesktopBootstrap = {
	version: string;
	platform: string;
	mediaCoreStatus: 'library-index-ready';
	nativeWindowFrame: boolean;
	windowShape?: 'css-squircle' | 'system';
};

export type LibraryStatus = {
	rootCount: number;
	fileCount: number;
	lastScanAt: number | null;
	isScanning: boolean;
};

export type ScanSummary = {
	rootName: string;
	fileCount: number;
};

export type CatalogMedia = {
	id: number;
	title: string;
	extension: string;
	sizeBytes: number;
	modifiedAt: number | null;
};

export type CatalogPage = {
	items: CatalogMedia[];
	total: number;
	offset: number;
};

export function isDesktopRuntime(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function readDesktopBootstrap(): Promise<DesktopBootstrap | null> {
	if (!isDesktopRuntime()) return null;

	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<DesktopBootstrap>('desktop_bootstrap');
}

export async function readLibraryStatus(): Promise<LibraryStatus | null> {
	if (!isDesktopRuntime()) return null;

	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<LibraryStatus>('get_library_status');
}

export async function readCatalogPage(offset = 0, count = 50): Promise<CatalogPage | null> {
	if (!isDesktopRuntime()) return null;

	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<CatalogPage>('get_catalog_page', { offset, count });
}

export async function chooseMediaFolder(): Promise<string | null> {
	if (!isDesktopRuntime()) return null;

	const { open } = await import('@tauri-apps/plugin-dialog');
	const selected = await open({ directory: true, multiple: false, title: 'Choose a media folder' });
	return typeof selected === 'string' ? selected : null;
}

export async function scanLibrary(rootPath: string): Promise<ScanSummary> {
	if (!isDesktopRuntime()) throw new Error('Local library scanning is available in the desktop app.');

	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<ScanSummary>('scan_library', { rootPath });
}
