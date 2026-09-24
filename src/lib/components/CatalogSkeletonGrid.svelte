<script lang="ts">
	type Props = { count?: number; label?: string };
	let { count = 12, label = 'Loading media' }: Props = $props();
	let cards = $derived(Array.from({ length: count }, (_, index) => index));
</script>

<div class="catalog-skeleton-grid" role="status" aria-label={label}>
	{#each cards as card (card)}
		<div class="skeleton-card" style={`--text-delay: ${card * 130}ms`} aria-hidden="true">
			<div class="skeleton-poster"></div>
			<div class="skeleton-copy">
				<div class="skeleton-line skeleton-line--title"></div>
				<div class="skeleton-line skeleton-line--meta"></div>
			</div>
		</div>
	{/each}
</div>

<style>
	.catalog-skeleton-grid {
		position: relative;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(100%, 146px), 1fr));
		gap: 18px;
		margin: -14px;
		padding: 14px 22px 36px 15px;
		opacity: 0;
		animation: skeleton-show 1ms step-end 120ms forwards;
	}
	.skeleton-card { min-width: 0; }
	.skeleton-poster {
		position: relative;
		overflow: hidden;
		width: 100%;
		aspect-ratio: 2 / 3;
		border: 1px solid rgba(255,255,255,0.07);
		border-radius: 12px;
		background: rgba(255,255,255,0.045);
	}
	.skeleton-copy { display: grid; gap: 8px; padding: 12px 2px 0; }
	.skeleton-line { position: relative; overflow: hidden; height: 8px; border-radius: 999px; background: rgba(255,255,255,0.07); }
	.skeleton-line--title { width: 72%; }
	.skeleton-line--meta { width: 46%; opacity: 0.65; }
	.skeleton-poster::after,
	.skeleton-line::after {
		position: absolute;
		inset: 0;
		border-radius: inherit;
		content: '';
		pointer-events: none;
	}
	.skeleton-poster::after {
		background: linear-gradient(105deg, transparent 48%, rgba(232,240,246,0.02) 49.5%, rgba(232,240,246,0.075) 50%, rgba(232,240,246,0.02) 50.5%, transparent 52%);
		background-attachment: fixed;
		background-size: 220vw 100vh;
		background-position: 100% 0;
		background-repeat: no-repeat;
		animation: poster-shimmer 4.2s linear infinite;
	}
	.skeleton-line::after {
		inset: 0 auto 0 0;
		width: 38%;
		background: linear-gradient(105deg, transparent 15%, rgba(232,240,246,0.1) 50%, transparent 85%);
		transform: translateX(-130%);
	}
	.skeleton-line--title::after { animation: text-shimmer 2.4s ease-in-out var(--text-delay, 0ms) infinite; }
	.skeleton-line--meta::after { animation: text-shimmer 2.4s ease-in-out calc(var(--text-delay, 0ms) + 170ms) infinite; }
	@keyframes skeleton-show { to { opacity: 1; } }
	@keyframes poster-shimmer { from { background-position: 100% 0; } to { background-position: 0 0; } }
	@keyframes text-shimmer { to { transform: translateX(390%); } }
	@media (max-width: 760px) {
		.catalog-skeleton-grid { gap: 15px 14px; padding-right: 17px; }
	}
	@media (prefers-reduced-motion: reduce) {
		.catalog-skeleton-grid { opacity: 1; animation: none; }
		.skeleton-poster::after,
		.skeleton-line::after { display: none; animation: none; }
	}
</style>
