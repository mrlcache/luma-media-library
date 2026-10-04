import test from 'node:test';
import assert from 'node:assert/strict';
import { mergeSeriesEpisodes } from '../src/lib/media/series-episodes.ts';

const file = (mediaId, season, episode) => ({ mediaId, season, episode, path: `${mediaId}.mkv`, fileName: `${mediaId}.mkv` });
const remote = (season, episode) => ({ id: season * 100 + episode, season, episode,
	title: `API title ${episode}`, summary: 'API summary', image: null, runtime: 45 });

test('merges real downloaded files with API episodes, preserving missing entries and API titles', () => {
	const rows = mergeSeriesEpisodes([file(33, 1, 3), file(11, 1, 1)], [remote(1, 1), remote(1, 2), remote(1, 3)], null);
	assert.deepEqual(rows.map((row) => [row.episode, row.file?.mediaId ?? null, row.title]),
		[[1, 11, 'API title 1'], [2, null, 'API title 2'], [3, 33, 'API title 3']]);
});

test('watched boundary covers earlier seasons and missing episodes, excluding current and later ones', () => {
	const rows = mergeSeriesEpisodes([], [remote(2, 4), remote(1, 10), remote(2, 2), remote(2, 3)], { season: 2, episode: 3 });
	assert.deepEqual(rows.map((row) => [row.season, row.episode, row.watched]),
		[[1, 10, true], [2, 2, true], [2, 3, false], [2, 4, false]]);
});

test('API failure preserves local files without inventing metadata or discarding unknown episode numbers', () => {
	const rows = mergeSeriesEpisodes([file(8, 2, 7), file(9, null, null)], [], null);
	assert.equal(rows.length, 2);
	assert.deepEqual(rows.map((row) => row.file.mediaId), [9, 8]);
	assert.ok(rows.every((row) => !row.watched && row.image === null));
});
