import test from 'node:test';
import assert from 'node:assert/strict';
import { formatTorrentTargetError, isCurrentTorrentTarget } from '../src/lib/torrents/target-routing.ts';

test('a torrent result only applies to the target that is still selected', () => {
	assert.equal(isCurrentTorrentTarget('pc', 'pc'), true);
	assert.equal(isCurrentTorrentTarget('phone', 'phone'), true);
	assert.equal(isCurrentTorrentTarget('pc', 'phone'), false);
	assert.equal(isCurrentTorrentTarget('phone', 'pc'), false);
});

test('torrent failures name the selected destination', () => {
	assert.equal(formatTorrentTargetError('pc', 'engine unavailable'), 'Computer: engine unavailable');
	assert.equal(formatTorrentTargetError('phone', 'engine unavailable'), 'This phone: engine unavailable');
});
