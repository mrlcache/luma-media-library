import type { HandleClientError } from '@sveltejs/kit';

export const handleError: HandleClientError = ({ error }) => {
	console.error('Luma page error:', error);
	// Keep demo failures readable on a phone without a remote debugger.
	const details = import.meta.env.VITE_LUMA_MOBILE_DEMO === 'true' && error instanceof Error
		? `${error.name}: ${error.message}\n${error.stack ?? ''}`.slice(0, 1800)
		: undefined;
	return { message: 'This page could not be opened.', details };
};
