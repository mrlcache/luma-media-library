<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';

	let reduceTransparency = $state(false);
	let reduceMotion = $state(false);
	let autoplay = $state(true);
</script>

<svelte:head><title>Settings · Media library</title></svelte:head>

<div class="settings-page">
	<div class="settings-heading">
		<p class="page-kicker">Workspace</p>
		<h1>Settings</h1>
		<p>Keep playback and browsing tuned to the way you use your library.</p>
	</div>

	<div class="settings-layout">
		<section class="settings-section" aria-labelledby="playback-heading">
			<div class="settings-section__heading"><span class="settings-index">01</span><div><h2 id="playback-heading">Playback</h2><p>Controls for starting and continuing media.</p></div></div>
			<div class="settings-card">
				<label class="setting-row"><span><strong>Autoplay next episode</strong><small>Continue a series when an episode ends.</small></span><input class="toggle" type="checkbox" bind:checked={autoplay} /></label>
				<div class="setting-divider"></div>
				<div class="setting-row setting-row--static"><span><strong>Default quality</strong><small>The server will choose the best compatible stream.</small></span><span class="setting-value">Auto <Icon name="chevron-right" size={15} /></span></div>
				<div class="setting-divider"></div>
				<div class="setting-row setting-row--static"><span><strong>Audio track</strong><small>Preferred track when a title offers more than one.</small></span><span class="setting-value">English <Icon name="chevron-right" size={15} /></span></div>
			</div>
		</section>

		<section class="settings-section" aria-labelledby="appearance-heading">
			<div class="settings-section__heading"><span class="settings-index">02</span><div><h2 id="appearance-heading">Appearance</h2><p>Keep the interface quiet and readable.</p></div></div>
			<div class="settings-card">
				<label class="setting-row"><span><strong>Reduce transparency</strong><small>Use solid surfaces instead of translucent overlays.</small></span><input class="toggle" type="checkbox" bind:checked={reduceTransparency} /></label>
				<div class="setting-divider"></div>
				<label class="setting-row"><span><strong>Reduce motion</strong><small>Minimize non-essential transitions and movement.</small></span><input class="toggle" type="checkbox" bind:checked={reduceMotion} /></label>
			</div>
		</section>

		<section class="settings-section" aria-labelledby="server-heading">
			<div class="settings-section__heading"><span class="settings-index">03</span><div><h2 id="server-heading">Server</h2><p>Information about this local library.</p></div></div>
			<div class="settings-card server-card">
				<div class="server-card__state"><span class="status-dot"></span><div><strong>Local server is ready</strong><span>Available on this device and local network.</span></div></div>
				<div class="server-card__meta"><span>Library roots</span><strong>1 connected</strong></div>
				<div class="server-card__meta"><span>Last scan</span><strong>Today, 09:42</strong></div>
				<button class="settings-action"><Icon name="refresh" size={15} />Run library scan</button>
			</div>
		</section>
	</div>
</div>

<style>
	.settings-page { max-width: 1060px; margin: 0 auto; padding: 54px 42px 76px; }
	.page-kicker { margin: 0 0 10px; color: var(--accent); font-size: 0.66rem; font-weight: 700; letter-spacing: 0.16em; text-transform: uppercase; }
	.settings-heading h1 { margin: 0; color: var(--text-strong); font-size: clamp(2rem, 4vw, 3.2rem); font-weight: 600; letter-spacing: -0.055em; line-height: 1; }
	.settings-heading > p:last-child { margin: 12px 0 0; color: var(--text-muted); font-size: 0.84rem; }
	.settings-layout { display: grid; gap: 40px; margin-top: 52px; }
	.settings-section { display: grid; grid-template-columns: 200px minmax(0, 1fr); gap: 34px; }
	.settings-section__heading { display: flex; align-items: start; gap: 13px; }
	.settings-index { color: var(--accent); font-size: 0.65rem; font-weight: 700; letter-spacing: 0.08em; }
	.settings-section h2 { margin: -3px 0 4px; color: var(--text-strong); font-size: 0.93rem; font-weight: 650; }
	.settings-section__heading p { margin: 0; color: var(--text-muted); font-size: 0.72rem; line-height: 1.5; }
	.settings-card { overflow: hidden; border: 1px solid var(--line-subtle); border-radius: var(--radius-md); background: var(--surface-1); }
	.setting-row { display: flex; align-items: center; justify-content: space-between; gap: 20px; min-height: 76px; padding: 17px 20px; cursor: pointer; }
	.setting-row > span:first-child { display: grid; gap: 4px; }
	.setting-row strong { color: var(--text-soft); font-size: 0.78rem; font-weight: 600; }
	.setting-row small { color: var(--text-muted); font-size: 0.68rem; }
	.setting-divider { height: 1px; margin: 0 20px; background: var(--line-subtle); }
	.setting-row--static { cursor: default; }
	.setting-value { display: inline-flex; align-items: center; gap: 5px; color: var(--text-muted); font-size: 0.7rem; }
	.toggle { position: relative; flex: 0 0 auto; width: 37px; height: 22px; appearance: none; border: 1px solid var(--line-strong); border-radius: 999px; background: var(--surface-3); cursor: pointer; transition: background 160ms ease, border-color 160ms ease; }
	.toggle::after { position: absolute; top: 3px; left: 3px; width: 14px; height: 14px; border-radius: 50%; background: var(--text-muted); content: ''; transition: transform 160ms ease, background 160ms ease; }
	.toggle:checked { border-color: var(--accent); background: rgba(201,174,124,0.3); }
	.toggle:checked::after { background: var(--accent-soft); transform: translateX(15px); }
	.server-card { display: grid; gap: 0; padding: 20px; }
	.server-card__state { display: flex; align-items: center; gap: 10px; padding-bottom: 18px; }
	.server-card__state > div { display: grid; gap: 3px; }
	.server-card__state strong { color: var(--text-soft); font-size: 0.78rem; font-weight: 600; }
	.server-card__state span:last-child { color: var(--text-muted); font-size: 0.68rem; }
	.server-card__meta { display: flex; justify-content: space-between; padding: 11px 0; border-top: 1px solid var(--line-subtle); color: var(--text-muted); font-size: 0.7rem; }
	.server-card__meta strong { color: var(--text-soft); font-weight: 550; }
	.settings-action { display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-height: 36px; margin-top: 12px; border: 1px solid var(--line-subtle); border-radius: 7px; color: var(--text-soft); font-size: 0.72rem; background: var(--surface-2); cursor: pointer; }
	.settings-action:hover { border-color: var(--line-strong); color: var(--text-strong); }
	@media (max-width: 760px) { .settings-page { padding: 36px 18px 58px; } .settings-layout { gap: 32px; margin-top: 38px; } .settings-section { grid-template-columns: 1fr; gap: 15px; } }
</style>
