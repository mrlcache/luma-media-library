<script lang="ts">
	import { page } from '$app/state';
	import LibraryView from '$lib/components/LibraryView.svelte';

	let type = $derived(page.url.searchParams.get('type'));
</script>

<LibraryView
	heading={type === 'movie' ? 'Movies' : type === 'series' ? 'Series' : 'Library'}
	initialType={type === 'movie' || type === 'series' ? type : 'all'}
/>
