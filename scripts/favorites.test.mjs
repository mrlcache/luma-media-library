import test from 'node:test';
import assert from 'node:assert/strict';
import { get } from 'svelte/store';
import { favorites, toggleFavorite, sameFavorite, favoriteCard, cardMedia, savedFavorite } from '../src/lib/media/favorites.ts';

const storage = new Map();
globalThis.localStorage = { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) };
globalThis.window = { addEventListener() {}, removeEventListener() {} };
const media = { id: 'tmdb-series-42', tmdbId: 42, installed: false, title: 'Example', kind: 'series', year: 2025, genres: ['Drama'], rating: '8.2', runtime: '', poster: 'https://image.tmdb.org/t/p/w780/example.jpg', backdrop: '', synopsis: 'Summary', match: '' };

test('favorites persist across subscriptions and keep offline details without starting playback', () => {
	toggleFavorite(media);
	assert.equal(get(favorites).length, 1);
	assert.equal(savedFavorite(media.id).installed, false);
	assert.equal(savedFavorite(media.id).poster, media.poster);
	assert.equal(favoriteCard(get(favorites)[0]).id, media.id);
	toggleFavorite(media);
	assert.equal(get(favorites).length, 0);
	assert.equal(savedFavorite(media.id), undefined);
});

test('a downloaded catalog item matches its saved recommendation without merging different releases or shows', () => {
	const local = { id: 123, title: 'Example', kind: 'series', year: 2025, posterUrl: media.poster, voteAverage: 8.2 };
	assert.ok(sameFavorite(media, cardMedia(local)));
	assert.equal(sameFavorite(media, { ...media, tmdbId: 43, id: 'tmdb-series-43' }), false);
	assert.equal(sameFavorite(media, { ...media, tmdbId: undefined, id: '777', year: 2011 }), false);
	assert.equal(sameFavorite(media, { ...media, tmdbId: undefined, id: '777', kind: 'movie' }), false);
});

test('saving failure leaves the heart and collection unchanged', () => {
	const stop = favorites.subscribe(() => {});
	const setItem = localStorage.setItem;
	localStorage.setItem = () => { throw new Error('Storage full'); };
	assert.throws(() => toggleFavorite(media), /Storage full/);
	assert.equal(get(favorites).length, 0);
	localStorage.setItem = setItem;
	stop();
});
