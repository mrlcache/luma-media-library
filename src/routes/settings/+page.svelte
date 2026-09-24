<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import {
		chooseMediaFolder,
		isDesktopRuntime,
		rescanLibrary,
		scanLibrary,
		readLibraryStatus,
		testTmdbConnection,
		type LibraryStatus
	} from '$lib/platform/desktop';
	import { nativeAcrylicStatus, requestNativeAcrylic } from '$lib/platform/native-acrylic';

	let reduceTransparency = $state(false);
	let reduceMotion = $state(false);
	let autoplay = $state(true);
	let desktopAvailable = $state(false);
	let libraryStatus = $state<LibraryStatus | null>(null);
	let loadingLibrary = $state(true);
	let scanning = $state(false);
	let libraryFeedback = $state('');
	let tmdbChecking = $state(false);
	let tmdbFeedback = $state('');

	onMount(() => {
		void refreshLibraryStatus();
		const acrylicRequester = Symbol('Settings surface');
		requestNativeAcrylic(acrylicRequester, true);
		return () => requestNativeAcrylic(acrylicRequester, false);
	});

	async function refreshLibraryStatus() {
		loadingLibrary = true;
		desktopAvailable = isDesktopRuntime();
		try {
			libraryStatus = await readLibraryStatus();
			desktopAvailable = libraryStatus !== null;
			scanning = libraryStatus?.isScanning ?? false;
		} catch {
			libraryStatus = null;
			libraryFeedback = 'The local library could not be reached.';
		} finally {
			loadingLibrary = false;
		}
	}

	async function addOrScanFolder() {
		libraryFeedback = '';
		try {
			const folder = await chooseMediaFolder();
			if (!folder) return;

			scanning = true;
			const result = await scanLibrary(folder);
			libraryFeedback = `${result.fileCount} video files indexed in ${result.rootName}. ${metadataFeedback(result.matchedCount, result.unmatchedCount, result.pendingCount, result.metadataError)}`;
			libraryStatus = await readLibraryStatus();
		} catch (error) {
			libraryFeedback = error instanceof Error ? error.message : 'The library scan could not finish.';
		} finally {
			scanning = false;
		}
	}

	async function refreshLibrary() {
		libraryFeedback = '';
		scanning = true;
		try {
			const result = await rescanLibrary();
			libraryFeedback = `Library refreshed: ${result.fileCount} video files across ${result.rootCount} folders. ${metadataFeedback(result.matchedCount, result.unmatchedCount, result.pendingCount, result.metadataError)}`;
			libraryStatus = await readLibraryStatus();
		} catch (error) {
			libraryFeedback = error instanceof Error ? error.message : 'The library refresh could not finish.';
		} finally {
			scanning = false;
		}
	}

	function metadataFeedback(matched: number, unmatched: number, pending: number, error: string | null): string {
		const summary = `TMDb metadata matched ${matched} files; ${unmatched} were left unmatched; ${pending} are pending.`;
		return error ? `${summary} Lookup paused: ${error}` : summary;
	}

	async function checkTmdbConnection() {
		tmdbChecking = true;
		tmdbFeedback = '';
		try {
			await testTmdbConnection();
			tmdbFeedback = 'Connection confirmed.';
		} catch (error) {
			tmdbFeedback = error instanceof Error ? error.message : 'TMDb could not be reached.';
		} finally {
			tmdbChecking = false;
		}
	}

	function formatScanTime(timestamp: number | null | undefined): string {
		if (!timestamp) return 'Not yet scanned';
		return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(
			new Date(timestamp * 1000)
		);
	}
</script>

<svelte:head><title>Settings · Media library</title></svelte:head>

<div class="settings-surface" data-native-backdrop={$nativeAcrylicStatus}>
	<div class="settings-page">
	<div class="settings-heading">
		<h1>Settings</h1>
	</div>

	<div class="settings-layout">
		<section class="settings-section" aria-labelledby="playback-heading">
			<div class="settings-section__heading">
				<h2 id="playback-heading">Playback</h2>
				<p>Controls for starting and continuing media.</p>
			</div>
			<div class="settings-list">
				<label class="setting-row">
					<span class="setting-copy"><strong>Autoplay next episode</strong><small>Continue a series when an episode ends.</small></span>
					<input class="toggle" type="checkbox" bind:checked={autoplay} />
				</label>
				<div class="setting-row setting-row--static">
					<span class="setting-copy"><strong>Default quality</strong><small>Use the best compatible stream when available.</small></span>
					<span class="setting-value">Auto</span>
				</div>
				<div class="setting-row setting-row--static">
					<span class="setting-copy"><strong>Audio track</strong><small>Preferred track when a title offers more than one.</small></span>
					<span class="setting-value">English</span>
				</div>
			</div>
		</section>

		<section class="settings-section" aria-labelledby="appearance-heading">
			<div class="settings-section__heading">
				<h2 id="appearance-heading">Appearance</h2>
				<p>Keep the interface quiet and readable.</p>
			</div>
			<div class="settings-list">
				<label class="setting-row">
					<span class="setting-copy"><strong>Reduce transparency</strong><small>Use solid surfaces instead of translucent overlays.</small></span>
					<input class="toggle" type="checkbox" bind:checked={reduceTransparency} />
				</label>
				<label class="setting-row">
					<span class="setting-copy"><strong>Reduce motion</strong><small>Minimize non-essential transitions and movement.</small></span>
					<input class="toggle" type="checkbox" bind:checked={reduceMotion} />
				</label>
			</div>
		</section>

		<section class="settings-section" aria-labelledby="library-heading">
			<div class="settings-section__heading">
				<h2 id="library-heading">Local library</h2>
				<p>Information about this local collection.</p>
			</div>
			<div class="settings-list settings-list--server">
				<div class="server-summary">
					<Icon name="library" size={18} />
					<div>
						<strong>Local library</strong>
						<span>{libraryStatus?.rootCount ? `${libraryStatus.fileCount} video files · ${libraryStatus.matchedCount} matched with metadata.` : desktopAvailable ? 'Choose a folder to index your videos.' : 'Available in the desktop app.'}</span>
					</div>
				</div>
				<div class="setting-row setting-row--meta"><span>Folders</span><strong>{loadingLibrary ? '—' : `${libraryStatus?.rootCount ?? 0} added`}</strong></div>
				<div class="setting-row setting-row--meta"><span>Last updated</span><strong>{formatScanTime(libraryStatus?.lastScanAt)}</strong></div>
				<div class="server-action-row">
					<button class="settings-action" type="button" disabled={!desktopAvailable || loadingLibrary || scanning} onclick={libraryStatus?.rootCount ? refreshLibrary : addOrScanFolder}>
						<Icon name="refresh" size={15} />{scanning ? 'Refreshing library…' : libraryStatus?.rootCount ? 'Refresh library' : 'Choose folder'}
					</button>
					{#if libraryStatus?.rootCount}
						<button class="settings-action settings-action--secondary" type="button" disabled={!desktopAvailable || loadingLibrary || scanning} onclick={addOrScanFolder}>Add folder</button>
					{/if}
					{#if libraryFeedback}<p class="library-feedback" role="status" aria-live="polite">{libraryFeedback}</p>{/if}
				</div>
			</div>
		</section>

		<section class="settings-section" aria-labelledby="metadata-heading">
			<div class="settings-section__heading">
				<h2 id="metadata-heading">Metadata</h2>
				<p>Search film and series information.</p>
			</div>
			<div class="settings-list settings-list--server">
				<div class="server-summary">
					<Icon name="search" size={18} />
					<div>
						<strong>The Movie Database</strong>
						<span>Read-only metadata access. The credential stays in this app profile.</span>
					</div>
				</div>
				<div class="server-action-row">
					<button class="settings-action" type="button" disabled={!desktopAvailable || tmdbChecking} onclick={checkTmdbConnection}>
						{tmdbChecking ? 'Checking connection…' : 'Test connection'}
					</button>
					{#if tmdbFeedback}<p class="library-feedback" role="status" aria-live="polite">{tmdbFeedback}</p>{/if}
				</div>
				<p class="provider-attribution">This product uses the TMDB API but is not endorsed or certified by TMDB. <a href="https://www.themoviedb.org" target="_blank" rel="noreferrer">The Movie Database</a></p>
			</div>
		</section>
	</div>
</div>
</div>

<style>
	.settings-surface { min-height: 100vh; background: #101419; }
	:global(html[data-runtime='desktop']) .settings-surface { background: var(--acrylic-content-tint); }
	:global(html[data-runtime='desktop'] .settings-surface[data-native-backdrop='unavailable']) { background: var(--acrylic-content-fallback); }
	.settings-page { max-width: 1080px; margin: 0 auto; padding: clamp(48px, 7vh, 82px) clamp(22px, 4vw, 62px) 84px; }
	.settings-heading h1 { margin: 0; color: var(--text-strong); font-family: var(--font-display); font-size: clamp(2rem, 4vw, 3rem); font-weight: 600; letter-spacing: -0.06em; line-height: 1; }
	.settings-layout { display: grid; gap: 0; margin-top: 52px; }
	.settings-section { display: grid; grid-template-columns: minmax(170px, 0.38fr) minmax(0, 1fr); gap: clamp(30px, 5vw, 68px); padding: 28px 0; border-top: 1px solid var(--line-subtle); }
	.settings-section:first-child { padding-top: 0; border-top: 0; }
	.settings-section__heading { align-self: start; }
	.settings-section h2 { margin: 0; color: var(--text-strong); font-size: 0.91rem; font-weight: 650; letter-spacing: -0.025em; }
	.settings-section__heading p { max-width: 170px; margin: 7px 0 0; color: var(--text-muted); font-size: 0.7rem; line-height: 1.5; }
	.settings-list { border-top: 1px solid var(--line-subtle); }
	.setting-row { display: flex; align-items: center; justify-content: space-between; gap: 24px; min-height: 74px; padding: 16px 0; border-bottom: 1px solid var(--line-subtle); cursor: pointer; }
	.setting-copy { display: grid; gap: 4px; }
	.setting-row strong { color: var(--text-soft); font-size: 0.78rem; font-weight: 600; }
	.setting-row small { color: var(--text-muted); font-size: 0.68rem; line-height: 1.4; }
	.setting-row--static { cursor: default; }
	.setting-value { flex: 0 0 auto; color: var(--text-muted); font-size: 0.7rem; }
	.toggle { position: relative; flex: 0 0 auto; width: 36px; height: 21px; appearance: none; border: 1px solid var(--line-strong); border-radius: 999px; background: var(--surface-3); cursor: pointer; transition: background 160ms ease, border-color 160ms ease; }
	.toggle::after { position: absolute; top: 3px; left: 3px; width: 13px; height: 13px; border-radius: 50%; background: var(--text-muted); content: ''; transition: transform 160ms ease, background 160ms ease; }
	.toggle:checked { border-color: var(--accent); background: rgba(158, 198, 214, 0.24); }
	.toggle:checked::after { background: var(--accent-soft); transform: translateX(15px); }
	.server-summary { display: flex; align-items: center; gap: 11px; min-height: 72px; border-bottom: 1px solid var(--line-subtle); color: var(--accent); }
	.server-summary > div { display: grid; gap: 3px; }
	.server-summary strong { color: var(--text-soft); font-size: 0.78rem; font-weight: 600; }
	.server-summary span { color: var(--text-muted); font-size: 0.68rem; }
	.setting-row--meta { min-height: 51px; padding: 13px 0; color: var(--text-muted); font-size: 0.7rem; cursor: default; }
	.setting-row--meta strong { color: var(--text-soft); font-size: 0.7rem; font-weight: 550; }
	.server-action-row { padding-top: 17px; }
	.settings-action { display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-height: 35px; padding: 0 13px; border: 1px solid var(--line-subtle); border-radius: var(--radius-sm); color: var(--text-soft); background: transparent; font-size: 0.7rem; cursor: pointer; transition: border-color 160ms ease, color 160ms ease, background 160ms ease; }
	.settings-action:hover { border-color: var(--line-strong); color: var(--text-strong); background: var(--material-highlight); }
	.settings-action--secondary { margin-left: 8px; color: var(--text-muted); }
	.settings-action:disabled { color: var(--text-dim); cursor: not-allowed; opacity: 0.6; }
	.library-feedback { margin: 10px 0 0; color: var(--text-muted); font-size: 0.68rem; line-height: 1.5; }
	.provider-attribution { margin: 14px 0 0; color: var(--text-dim); font-size: 0.64rem; line-height: 1.55; }
	.provider-attribution a { color: var(--text-muted); text-decoration: underline; text-underline-offset: 2px; }
	@media (max-width: 760px) {
		.settings-page { padding: 42px 18px 70px; }
		.settings-heading h1 { font-size: 2rem; }
		.settings-layout { margin-top: 34px; }
		.settings-section { display: block; padding: 26px 0; }
		.settings-section:first-child { padding-top: 0; }
		.settings-section__heading { margin-bottom: 16px; }
		.settings-section__heading p { max-width: none; margin-top: 5px; }
		.setting-row { gap: 15px; min-height: 76px; padding: 16px 0; }
		.setting-copy { min-width: 0; }
		.setting-row small { max-width: 230px; }
	}
	@media (prefers-reduced-motion: reduce) {
		.toggle, .toggle::after, .settings-action { transition: none; }
	}
	@media (prefers-reduced-transparency: reduce) {
		:global(html[data-runtime='desktop']) .settings-surface { background: #0c0f13; }
		.toggle:checked { background: #263740; }
		.settings-action:hover { background: var(--surface-2); }
	}
</style>
