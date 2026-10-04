import test from 'node:test';
import assert from 'node:assert/strict';
import { normalizeRelease, releaseMatches, sortReleases } from '../src/lib/torrents/releases.ts';

const media = { title: 'Example Show', year: 2025 };
const release = (name, seeds = 1) => normalizeRelease({ name, seeds, source: '1337x' });

test('QXR is inferred as a complete token and precedes other groups under every sort', () => {
	const rows = [release('Example Show S01 1080p ABC', 1000), release('Example Show S01 720p [QxR]', 5), release('Example Show S01 2160p-QXR', 20)];
	assert.equal(release('Example Show MyQXRGroup').qxr, false);
	for (const sort of ['seeds', 'name', 'size']) assert.ok(sortReleases(rows, sort).slice(0, 2).every((row) => row.qxr));
	assert.deepEqual(sortReleases(rows, 'seeds').map((row) => row.seeds), [20, 5, 1000]);
});

test('full season picker excludes individual episodes and selects the correct season', () => {
	const scope = { type: 'season', season: 2 };
	for (const name of ['Example Show S02 1080p', 'Example Show Season 2 COMPLETE', 'Example Show S01-S03 Complete', 'Example Show Complete Series']) assert.ok(releaseMatches(release(name), media, scope), name);
	for (const name of ['Example Show S01 1080p', 'Example Show S02E03', 'Example Show S02E01-E10', 'Example Show 2x03', 'Example Show Season 2 Episode 3', 'Another Show S02']) assert.equal(releaseMatches(release(name), media, scope), false, name);
});

test('episode picker accepts only the exact episode, excluding packs and multi-episode releases', () => {
	const scope = { type: 'episode', season: 1, episode: 3 };
	for (const name of ['Example Show S01E03.1080p.x265-QXR', 'Example Show 1x03', 'Example Show Season 1 Episode 3']) assert.ok(releaseMatches(release(name), media, scope), name);
	for (const name of ['Example Show S01', 'Example Show S01E02', 'Example Show S02E03', 'Example Show S01E030', 'Example Show S01E03-E05', 'Example Show S01E03E04', 'Example Show S01E03-05', 'Example Show S01E03-S01E05', 'Other Show S01E03']) assert.equal(releaseMatches(release(name), media, scope), false, name);
});

test('unknown formats/uploaders remain unknown; actual format and codec are preserved', () => {
	const unknown = release('Example Show S01E03 1080p x265 [QXR]');
	assert.equal(unknown.fileType, '');
	assert.equal(unknown.uploader, '');
	assert.equal(unknown.group, 'QXR');
	assert.equal(unknown.codec, 'HEVC');
	assert.equal(normalizeRelease({ name: 'Example Show S01E03', filename: 'show.mp4' }).fileType, 'MP4');
});

test('movie search excludes episode releases and a different release year', () => {
	const movie = { title: 'Example Movie', year: 2025 };
	assert.equal(releaseMatches(release('Example Movie 2025 1080p'), movie, { type: 'movie' }), true);
	assert.equal(releaseMatches(release('Example Movie 1999 1080p'), movie, { type: 'movie' }), false);
	assert.equal(releaseMatches(release('Example Movie S01E03'), movie, { type: 'movie' }), false);
});
