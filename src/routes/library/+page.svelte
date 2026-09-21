<script lang="ts">
	import { page } from '$app/state';
	import LibraryView from '$lib/components/LibraryView.svelte';

	let type = $derived(page.url.searchParams.get('type'));
</script>

<LibraryView
	heading={type === 'movie' ? 'Movies' : type === 'series' ? 'Series' : 'Library'}
	description={type === 'movie' ? 'Feature films and documentaries in your collection.' : type === 'series' ? 'Series, seasons and episodes ready when you are.' : 'A focused view of everything in your collection.'}
	initialType={type === 'movie' || type === 'series' ? type : 'all'}
/>
