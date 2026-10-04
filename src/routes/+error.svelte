<script lang="ts">
	import { page } from '$app/state';
</script>

<section class="page-error">
	<strong>Could not open this page</strong>
	<p>{page.error?.message ?? 'Please try again.'}</p>
	<button type="button" onclick={() => window.location.replace('/')}>Try again</button>
	{#if page.error?.details}<pre>{page.error.details}</pre>{/if}
</section>

<style>
	.page-error { display: grid; align-content: center; justify-items: center; gap: 14px; min-height: 65dvh; padding: 24px; text-align: center; }
	strong { font-size: 1.1rem; color: var(--text-strong); }
	p { margin: 0; color: var(--text-muted); font-size: .85rem; }
	button { min-height: 44px; padding: 0 20px; border: 1px solid var(--line-strong); border-radius: 12px; background: var(--material-background); color: var(--text-strong); font: inherit; cursor: pointer; }
	pre { max-width: 100%; padding: 16px; overflow: auto; border-radius: 12px; background: #111b26; color: #c8d9e5; font-size: .72rem; text-align: left; white-space: pre-wrap; overflow-wrap: anywhere; user-select: text; }
</style>
