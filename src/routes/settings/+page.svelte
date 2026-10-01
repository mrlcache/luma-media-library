<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import SteppedRange from '$lib/components/SteppedRange.svelte';
	import {
		chooseMediaFolder,
		isDesktopRuntime,
		rescanLibrary,
		scanLibrary,
		readLibraryStatus,
		readDesktopBootstrap,
		type LibraryStatus
	} from '$lib/platform/desktop';
	import { nativeAcrylicStatus, requestNativeAcrylic } from '$lib/platform/native-acrylic';
	import { readAppearancePreferences, updateAppearancePreference } from '$lib/platform/appearance-preferences';
	import { readPlaybackPreferences, updatePlaybackPreference, subtitleFontOptions, type PlaybackPreferences, type SubtitleFont } from '$lib/platform/playback-preferences';

	let reduceTransparency = $state(false);
	let reduceMotion = $state(false);
	let autoplay = $state(true);
	let audioLanguage = $state<PlaybackPreferences['audioLanguage']>('system');
	let subtitleLanguage = $state<PlaybackPreferences['subtitleLanguage']>('auto');
	let subtitleFont = $state<SubtitleFont>('Manrope');
	let subtitleSize = $state(100);
	let subtitlePosition = $state(0);
	let desktopAvailable = $state(false);
	let libraryStatus = $state<LibraryStatus | null>(null);
	let loadingLibrary = $state(true);
	let scanning = $state(false);
	let libraryFeedback = $state('');
	let appVersion = $state('');

	onMount(() => {
		const appearance = readAppearancePreferences();
		reduceTransparency = appearance.reduceTransparency;
		reduceMotion = appearance.reduceMotion;
		const playback = readPlaybackPreferences();
		autoplay = playback.autoplayNextEpisode;
		audioLanguage = playback.audioLanguage;
		subtitleLanguage = playback.subtitleLanguage;
		subtitleFont = playback.subtitleFont;
		subtitleSize = playback.subtitleSize;
		subtitlePosition = playback.subtitlePosition / 1.5;
		void refreshLibraryStatus();
		void readDesktopBootstrap().then((bootstrap) => { appVersion = bootstrap?.version ?? ''; }).catch(() => {});
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
			libraryFeedback = `${result.fileCount} video files indexed in ${result.rootName}.${result.metadataError ? ` Metadata unavailable: ${result.metadataError}` : ''}`;
			libraryStatus = await readLibraryStatus();
		} catch (error) {
			libraryFeedback = error instanceof Error ? error.message : typeof error === 'string' ? error : 'The library scan could not finish.';
		} finally {
			scanning = false;
		}
	}

	async function refreshLibrary() {
		libraryFeedback = '';
		scanning = true;
		try {
			const result = await rescanLibrary();
			libraryFeedback = `Library refreshed: ${result.fileCount} video files across ${result.rootCount} folders.${result.metadataError ? ` Metadata unavailable: ${result.metadataError}` : ''}`;
			libraryStatus = await readLibraryStatus();
		} catch (error) {
			libraryFeedback = error instanceof Error ? error.message : typeof error === 'string' ? error : 'The library refresh could not finish.';
		} finally {
			scanning = false;
		}
	}

	function formatScanTime(timestamp: number | null | undefined): string {
		if (!timestamp) return 'Not yet scanned';
		return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(
			new Date(timestamp * 1000)
		);
	}
</script>

<svelte:head><title>Settings · Luma</title></svelte:head>

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
					<input class="toggle" type="checkbox" bind:checked={autoplay} onchange={(event) => updatePlaybackPreference('autoplayNextEpisode', (event.currentTarget as HTMLInputElement).checked)} />
				</label>
				<div class="setting-row setting-row--static">
					<span class="setting-copy"><strong>Default quality</strong><small>Play local media at its original quality.</small></span>
					<span class="setting-value">Original file</span>
				</div>
				<label class="setting-row">
					<span class="setting-copy"><strong>Preferred audio</strong><small>Default language when available.</small></span>
					<select class="setting-select" bind:value={audioLanguage} onchange={(event) => updatePlaybackPreference('audioLanguage', (event.currentTarget as HTMLSelectElement).value as PlaybackPreferences['audioLanguage'])} aria-label="Preferred audio language">
						<option value="system">System default</option><option value="en">English</option><option value="pt">Portuguese</option><option value="ja">Japanese</option>
					</select>
				</label>
			</div>
		</section>

		<section class="settings-section settings-section--subtitles" aria-labelledby="subtitles-heading">
			<div class="settings-section__heading">
				<h2 id="subtitles-heading">Subtitles</h2>
				<p>Make every line easy to read.</p>
			</div>
			<div class="subtitle-workspace">
				<aside class="subtitle-preview-card" aria-label="Live subtitle preview">
					<div class="subtitle-preview-card__scene">
						<img src="/images/subtitle-preview-mobland.jpg" alt="Two characters talking in a scene from MobLand" width="1280" height="534" />
						<p class="subtitle-preview-card__caption" style={`font-family: '${subtitleFont}', sans-serif; font-size: ${1 * subtitleSize / 100}rem; bottom: ${6 + subtitlePosition * 4.5}%;`}>
							Give me ten minutes.<br />I’ll be right there.
						</p>
					</div>
				</aside>
				<div class="settings-list subtitle-controls">
				<label class="setting-row">
					<span class="setting-copy"><strong>Language</strong><small>Default subtitle track.</small></span>
					<select class="setting-select" bind:value={subtitleLanguage} onchange={(event) => updatePlaybackPreference('subtitleLanguage', (event.currentTarget as HTMLSelectElement).value as PlaybackPreferences['subtitleLanguage'])} aria-label="Preferred subtitle language">
						<option value="auto">Automatic</option><option value="off">Off</option><option value="en">English</option><option value="pt">Portuguese</option><option value="ja">Japanese</option>
					</select>
				</label>
				<label class="setting-row">
					<span class="setting-copy"><strong>Font</strong></span>
					<select class="setting-select" bind:value={subtitleFont} onchange={(event) => updatePlaybackPreference('subtitleFont', (event.currentTarget as HTMLSelectElement).value as SubtitleFont)} aria-label="Subtitle font">
						{#each subtitleFontOptions as font (font.value)}<option value={font.value}>{font.label}</option>{/each}
					</select>
				</label>
				<label class="setting-row setting-row--slider">
					<span class="setting-copy"><strong>Size</strong><small>{subtitleSize}%</small></span>
					<span class="setting-range-control">
						<SteppedRange min={70} max={150} step={10} bind:value={subtitleSize} oninput={(event) => updatePlaybackPreference('subtitleSize', Number((event.currentTarget as HTMLInputElement).value))} ariaLabel="Default subtitle size" />
					</span>
				</label>
				<label class="setting-row setting-row--slider">
					<span class="setting-copy"><strong>Position</strong><small>{subtitlePosition === 0 ? 'Bottom' : `${subtitlePosition} steps above the bottom`}</small></span>
					<span class="setting-range-control">
						<SteppedRange min={0} max={8} step={1} bind:value={subtitlePosition} oninput={(event) => updatePlaybackPreference('subtitlePosition', Number((event.currentTarget as HTMLInputElement).value) * 1.5)} ariaLabel="Default subtitle vertical position" />
					</span>
				</label>
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
					<input class="toggle" type="checkbox" bind:checked={reduceTransparency} onchange={(event) => updateAppearancePreference('reduceTransparency', event.currentTarget.checked)} />
				</label>
				<label class="setting-row">
					<span class="setting-copy"><strong>Reduce motion</strong><small>Minimize non-essential transitions and movement.</small></span>
					<input class="toggle" type="checkbox" bind:checked={reduceMotion} onchange={(event) => updateAppearancePreference('reduceMotion', event.currentTarget.checked)} />
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
						<span>{libraryStatus?.rootCount ? `${libraryStatus.fileCount} video files indexed.` : desktopAvailable ? 'Choose a folder to index your videos.' : 'Available in the desktop app.'}</span>
					</div>
				</div>
				<div class="setting-row setting-row--meta"><span>Folders</span><strong>{loadingLibrary ? '—' : `${libraryStatus?.rootCount ?? 0} added`}</strong></div>
				{#if appVersion}<div class="setting-row setting-row--meta"><span>Luma version</span><strong>{appVersion}</strong></div>{/if}
				<div class="setting-row setting-row--meta"><span>Last updated</span><strong>{formatScanTime(libraryStatus?.lastScanAt)}</strong></div>
				<div class="server-action-row">
					<button class="settings-action" type="button" disabled={!desktopAvailable || loadingLibrary || scanning} onclick={libraryStatus?.rootCount ? refreshLibrary : addOrScanFolder}>
						<Icon name="refresh" size={15} />{scanning ? 'Refreshing library…' : libraryStatus?.rootCount ? 'Refresh library' : 'Choose folder'}
					</button>
					{#if libraryStatus?.rootCount}<button class="settings-action settings-action--secondary" type="button" disabled={!desktopAvailable || loadingLibrary || scanning} onclick={addOrScanFolder}><Icon name="folder" size={15} />Add folder</button>{/if}
					{#if libraryFeedback}<p class="library-feedback" role="status" aria-live="polite">{libraryFeedback}</p>{/if}
				</div>
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
	.settings-section { display: grid; grid-template-columns: minmax(245px, 1.05fr) minmax(0, 1.95fr); gap: clamp(24px, 3.3vw, 44px); padding: 28px 0; border-top: 1px solid var(--line-subtle); }
	.settings-section:first-child { padding-top: 0; border-top: 0; }
	.settings-section__heading { align-self: start; }
	.settings-section--subtitles { grid-template-columns: 1fr; gap: 20px; }
	.settings-section--subtitles .settings-section__heading > p { max-width: none; }
	.subtitle-workspace { display: grid; grid-template-columns: minmax(0, 1.25fr) minmax(280px, 1fr); align-items: center; gap: 28px; padding: 24px; border: 1px solid rgba(170, 205, 220, 0.14); border-radius: 16px; background: linear-gradient(145deg, rgba(28, 36, 44, 0.68), rgba(14, 19, 25, 0.8) 58%, rgba(11, 16, 22, 0.84)); box-shadow: inset 0 1px rgba(255,255,255,0.035), 0 14px 34px rgba(0,0,0,0.12); -webkit-backdrop-filter: blur(20px) saturate(118%); backdrop-filter: blur(20px) saturate(118%); }
	.subtitle-preview-card { min-width: 0; overflow: hidden; border-radius: 10px; background: #030405; }
	.subtitle-workspace .subtitle-controls { border-top: 0; min-width: 0; }
	.subtitle-controls .setting-row { min-height: 64px; padding: 12px 0; gap: 14px; }
	.subtitle-controls .setting-row:first-child { padding-top: 0; }
	.subtitle-controls .setting-row:last-child { padding-bottom: 0; border-bottom: 0; }
	.setting-range-control { flex: 0 0 160px; min-width: 0; }
	.subtitle-preview-card__scene { position: relative; overflow: hidden; aspect-ratio: 16 / 9; background: #000; }
	.subtitle-preview-card__scene img { display: block; width: 100%; height: 100%; object-fit: contain; }
	.subtitle-preview-card__caption { position: absolute; z-index: 1; right: 9px; left: 9px; width: auto; max-width: none; margin: 0; color: #fff; font-weight: 600; line-height: 1.3; text-align: center; text-shadow: -1px -1px 1px #000, 1px -1px 1px #000, -1px 1px 1px #000, 1px 1px 1px #000, 0 2px 5px rgba(0,0,0,0.9); transition: bottom 120ms ease, font-size 120ms ease; }
	.settings-section h2 { margin: 0; color: var(--text-strong); font-size: 0.91rem; font-weight: 650; letter-spacing: -0.025em; }
	.settings-section__heading > p { max-width: 170px; margin: 7px 0 0; color: var(--text-muted); font-size: 0.7rem; line-height: 1.5; }
	.settings-list { border-top: 1px solid var(--line-subtle); }
	.setting-row { display: flex; align-items: center; justify-content: space-between; gap: 24px; min-height: 74px; padding: 16px 0; border-bottom: 1px solid var(--line-subtle); cursor: pointer; }
	.setting-copy { display: grid; gap: 4px; }
	.setting-row strong { color: var(--text-soft); font-size: 0.78rem; font-weight: 600; }
	.setting-row small { color: var(--text-muted); font-size: 0.68rem; line-height: 1.4; }
	.setting-row--static { cursor: default; }
	.setting-value { flex: 0 0 auto; color: var(--text-muted); font-size: 0.7rem; }
	.setting-select { flex: 0 0 155px; min-width: 0; height: 34px; padding: 0 31px 0 9px; appearance: none; border: 1px solid var(--line-subtle); border-radius: var(--radius-sm); color: var(--text-soft); background-color: var(--surface-2); background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16' fill='none'%3E%3Cpath d='m4 6 4 4 4-4' stroke='%23ccd2d7' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E"); background-repeat: no-repeat; background-position: right 10px center; background-size: 14px; font: inherit; font-size: 0.7rem; }
	.setting-select option { background: var(--surface-2); }
	.setting-row--slider { cursor: default; }
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
	.server-action-row { display: flex; align-items: center; flex-wrap: wrap; gap: 9px; padding-top: 17px; }
	.settings-action { display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-height: 35px; padding: 0 13px; border: 1px solid var(--line-subtle); border-radius: var(--radius-sm); color: var(--text-soft); background: transparent; font-size: 0.7rem; cursor: pointer; transition: border-color 160ms ease, color 160ms ease, background 160ms ease; }
	.settings-action:hover { border-color: var(--line-strong); color: var(--text-strong); background: var(--material-highlight); }
	.settings-action--secondary { color: var(--text-muted); }
	.settings-action:disabled { color: var(--text-dim); cursor: not-allowed; opacity: 0.6; }
	.library-feedback { flex-basis: 100%; margin: 1px 0 0; color: var(--text-muted); font-size: 0.68rem; line-height: 1.5; }
	@media (max-width: 760px) {
		.settings-page { padding: 42px 18px 70px; }
		.settings-heading h1 { font-size: 2rem; }
		.settings-layout { margin-top: 34px; }
		.settings-section { display: block; padding: 26px 0; }
		.settings-section:first-child { padding-top: 0; }
		.settings-section__heading { margin-bottom: 16px; }
		.settings-section__heading > p { max-width: none; margin-top: 5px; }
		.subtitle-workspace { grid-template-columns: 1fr; gap: 20px; padding: 18px; }
		.subtitle-preview-card { width: 100%; }
		.setting-row { gap: 15px; min-height: 76px; padding: 16px 0; }
		.setting-copy { min-width: 0; }
		.setting-row small { max-width: 230px; }
		.setting-select { flex-basis: 138px; }
		.setting-range-control { flex-basis: 120px; }
	}
	@media (prefers-reduced-motion: reduce) {
		.toggle, .toggle::after, .settings-action, .subtitle-preview-card__caption { transition: none; }
	}
	@media (prefers-reduced-transparency: reduce) {
		:global(html[data-runtime='desktop']) .settings-surface { background: #0c0f13; }
		.toggle:checked { background: #263740; }
		.settings-action:hover { background: var(--surface-2); }
	}
</style>
