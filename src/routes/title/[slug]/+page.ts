import { error } from '@sveltejs/kit';
import { getMedia } from '$lib/data';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }) => {
	const item = getMedia(params.slug);
	if (!item) error(404, 'Title not found');
	return { item };
};
