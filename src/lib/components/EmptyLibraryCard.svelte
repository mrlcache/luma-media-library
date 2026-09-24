<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';

	type Props = {
		title: string;
		description: string;
		showSettingsLink?: boolean;
		role?: 'status';
		fullHeight?: boolean;
		centered?: boolean;
	};

	let { title, description, showSettingsLink = false, role, fullHeight = false, centered = false }: Props = $props();
</script>

<div class="empty-card-shell" class:empty-card-shell--full-height={fullHeight} class:empty-card-shell--centered={centered} {role}>
	<div class="empty-card">
		<Icon name="library" size={11} />
		<h2>{title}</h2>
		<p>{description}</p>
		{#if showSettingsLink}<a class="empty-card__button" href="/settings">Open settings</a>{/if}
	</div>
</div>

<style>
	.empty-card-shell { display: grid; place-items: center; min-height: 360px; padding: 24px; color: var(--text-muted); text-align: center; }
	.empty-card-shell--full-height { min-height: 0; height: 100%; padding: 0; }
	.empty-card-shell--centered { position: absolute; inset: 0; min-height: 0; padding: 0; pointer-events: none; }
	.empty-card {
		display: grid;
		justify-items: center;
		align-content: center;
		width: min(100%, 585px);
		min-height: 270px;
		padding: 38px 44px;
		border: 1px solid rgba(220, 230, 240, 0.2);
		border-radius: 24px;
		background: linear-gradient(145deg, rgba(255,255,255,0.055), rgba(255,255,255,0.018));
		box-shadow: inset 0 1px rgba(255,255,255,0.11), inset 0 0 0 1px rgba(255,255,255,0.025), 0 22px 56px rgba(0,0,0,0.22);
	}
	.empty-card-shell--centered .empty-card { pointer-events: auto; }
	.empty-card :global(svg) { color: var(--text-soft); }
	.empty-card h2 { margin: 9px 0 0; color: var(--text-strong); font-family: var(--font-display); font-size: 1.25rem; font-weight: 650; letter-spacing: -0.035em; }
	.empty-card p { max-width: 390px; margin: 8px 0 0; font-size: 0.78rem; line-height: 1.55; }
	.empty-card__button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 40px;
		margin-top: 20px;
		padding: 0 15px;
		border: 1px solid rgba(255,255,255,0.14);
		border-radius: 11px;
		color: var(--text-strong);
		background: rgba(54,60,68,0.48);
		box-shadow: inset 0 1px rgba(255,255,255,0.075), 0 8px 24px rgba(0,0,0,0.16);
		font-size: 0.76rem;
		font-weight: 650;
		text-decoration: none;
		-webkit-backdrop-filter: blur(20px) saturate(145%);
		backdrop-filter: blur(20px) saturate(145%);
		transition: background-color 160ms ease, transform 160ms ease;
	}
	.empty-card__button:hover, .empty-card__button:focus-visible { transform: scale(1.025); background: rgba(70,77,86,0.61); }

	@media (max-width: 760px) {
		.empty-card-shell { min-height: 320px; padding: 20px 18px; }
		.empty-card { width: 100%; min-height: 240px; padding: 30px 22px; border-radius: 21px; }
	}
	@media (prefers-reduced-motion: reduce) { .empty-card__button { transition: none; } .empty-card__button:hover, .empty-card__button:focus-visible { transform: none; } }
	@media (prefers-reduced-transparency: reduce) {
		.empty-card { background: var(--surface-2); }
		.empty-card__button { background: var(--surface-3); -webkit-backdrop-filter: none; backdrop-filter: none; }
	}
</style>
