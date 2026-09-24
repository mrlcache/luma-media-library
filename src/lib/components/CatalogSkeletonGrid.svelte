<script lang="ts">
	type Props = { count?: number; label?: string };
	let { count = 12, label = 'Loading media' }: Props = $props();
	let cards = $derived(Array.from({ length: count }, (_, index) => index));
</script>

<div class="catalog-skeleton-grid" role="status" aria-label={label}>
	{#each cards as card (card)}
		<div class="skeleton-card" aria-hidden="true">
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
		isolation: isolate;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(100%, 146px), 1fr));
		gap: 18px;
		margin: -14px;
		padding: 14px 22px 36px 15px;
		overflow: hidden;
		opacity: 0;
		animation: skeleton-show 1ms step-end 120ms forwards;
	}
	.skeleton-card { min-width: 0; }
	.skeleton-poster {
		width: 100%;
		aspect-ratio: 2 / 3;
		border: 1px solid rgba(255,255,255,0.07);
		border-radius: 12px;
		background: rgba(255,255,255,0.045);
	}
	.skeleton-copy { display: grid; gap: 8px; padding: 12px 2px 0; }
	.skeleton-line { height: 8px; border-radius: 999px; background: rgba(255,255,255,0.07); }
	.skeleton-line--title { width: 72%; }
	.skeleton-line--meta { width: 46%; opacity: 0.65; }
	.catalog-skeleton-grid::after {
		position: absolute;
		inset: 0 auto 0 0;
		width: 34%;
		background: linear-gradient(105deg, transparent 8%, rgba(232,240,246,0.075) 48%, transparent 88%);
		content: '';
		pointer-events: none;
		transform: translateX(-130%);
		animation: skeleton-shimmer 1.7s ease-in-out 120ms infinite;
	}
	@keyframes skeleton-show { to { opacity: 1; } }
	@keyframes skeleton-shimmer { to { transform: translateX(390%); } }
	@media (max-width: 760px) {
		.catalog-skeleton-grid { gap: 15px 14px; padding-right: 17px; }
	}
	@media (prefers-reduced-motion: reduce) {
		.catalog-skeleton-grid { opacity: 1; animation: none; }
		.catalog-skeleton-grid::after { display: none; animation: none; }
	}
</style>
