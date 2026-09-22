<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';

	let reduceTransparency = $state(false);
	let reduceMotion = $state(false);
	let autoplay = $state(true);
</script>

<svelte:head><title>Settings · Media library</title></svelte:head>

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
					<span class="setting-copy"><strong>Default quality</strong><small>The server will choose the best compatible stream.</small></span>
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

		<section class="settings-section" aria-labelledby="server-heading">
			<div class="settings-section__heading">
				<h2 id="server-heading">Local library</h2>
				<p>Information about this local collection.</p>
			</div>
			<div class="settings-list settings-list--server">
				<div class="server-summary">
					<Icon name="library" size={18} />
					<div><strong>Local server</strong><span>Available on this device and local network.</span></div>
				</div>
				<div class="setting-row setting-row--meta"><span>Library roots</span><strong>1 connected</strong></div>
				<div class="setting-row setting-row--meta"><span>Last scan</span><strong>Today, 09:42</strong></div>
				<div class="server-action-row">
					<button class="settings-action" type="button"><Icon name="refresh" size={15} />Run library scan</button>
				</div>
			</div>
		</section>
	</div>
</div>

<style>
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
		.toggle:checked { background: #263740; }
		.settings-action:hover { background: var(--surface-2); }
	}
</style>
