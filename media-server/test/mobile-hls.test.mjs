import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import http from 'node:http';
import { mkdir, mkdtemp, rm, stat } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { Transcoder } from '../src/transcode.mjs';
import { MobileHls } from '../src/mobile-hls.mjs';

const mediaServerDirectory = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const ffmpeg = path.join(mediaServerDirectory, 'tools', 'ffmpeg.exe');
const ffprobe = path.join(mediaServerDirectory, 'tools', 'ffprobe.exe');
const identity = createHash('sha256').update('mobile-hls-test-media').digest('hex');

function run(program, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(program, args, { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
    let stderr = '';
    child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-2000); });
    child.once('error', reject);
    child.once('close', code => code === 0 ? resolve() : reject(new Error(stderr || `${program} exited with ${code}`)));
  });
}

test('mobile HLS streams bounded TS segments, reuses sessions, rejects invalid resources and cancels cleanly', {
  skip: !existsSync(ffmpeg) || !existsSync(ffprobe),
  timeout: 45_000,
}, async () => {
  const runtimeParent = path.join(mediaServerDirectory, '.runtime');
  await mkdir(runtimeParent, { recursive: true });
  const workspace = await mkdtemp(path.join(runtimeParent, 'mobile-hls-test-'));
  const fixturePath = path.join(workspace, 'fixture.mp4');
  const info = { size: 0, mtimeMs: 0 };
  let server;
  let mobileHls;
  let media;
  try {
    await run(ffmpeg, [
      '-hide_banner', '-loglevel', 'error', '-y',
      '-f', 'lavfi', '-i', 'testsrc=size=320x240:rate=24:duration=8',
      '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=8',
      '-t', '8', '-c:v', 'libx264', '-preset', 'ultrafast', '-pix_fmt', 'yuv420p',
      '-c:a', 'aac', '-ac', '2', fixturePath,
    ]);
    const fixtureInfo = await stat(fixturePath);
    info.size = fixtureInfo.size;
    info.mtimeMs = fixtureInfo.mtimeMs;
    const file = { path: fixturePath, info };
    const transcoder = new Transcoder(path.join(mediaServerDirectory, 'tools'));
    assert.equal(transcoder.available, true, 'bundled FFmpeg and ffprobe should be available');
    media = {
      catalog: { matchingFile: async (id, requestedIdentity) => id === 'file-11' && requestedIdentity === identity ? file : null },
      transcoder,
    };
    mobileHls = new MobileHls({ media, workspace, idleTimeoutMs: 45_000, cleanupIntervalMs: 15_000, startupTimeoutMs: 15_000 });
    server = http.createServer(async (req, res) => {
      const handled = await mobileHls.handle(req, res, new URL(req.url, 'http://127.0.0.1'));
      if (!handled && !res.writableEnded) { res.writeHead(404); res.end(); }
    });
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', resolve);
    });
    const origin = `http://127.0.0.1:${server.address().port}`;
    const request = (url, method = 'GET', headers = {}) => fetch(new URL(url, origin), {
      method,
      headers: { 'X-Luma-Control': '1', 'X-Luma-Media-Identity': identity, ...headers },
    });
    const query = '?start=0&quality=480p&bitrate=800000&session=player-test';

    const head = await request(`/api/mobile-hls/file-11${query}`, 'HEAD');
    assert.equal(head.status, 200);
    assert.equal(head.headers.get('content-type'), 'application/vnd.apple.mpegurl');
    assert.equal(head.headers.get('x-luma-media-identity'), identity);
    assert.equal(Number(head.headers.get('x-luma-duration')), 8);
    assert.equal(Number(head.headers.get('x-luma-bitrate-limit')), 800_000);
    assert.equal(transcoder.active.size, 0, 'HEAD must not start an encoder');

    const first = await request(`/api/mobile-hls/file-11${query}`);
    assert.equal(first.status, 200);
    assert.equal(first.headers.get('x-luma-media-identity'), identity);
    const sessionId = first.headers.get('x-luma-hls-session');
    assert.match(sessionId, /^[a-f0-9-]{36}$/);
    const playlist = await first.text();
    const segmentName = playlist.split(/\r?\n/).find(line => /^segment-\d{6}\.ts$/.test(line));
    assert.ok(segmentName, 'initial playlist should contain a completed TS segment');
    assert.doesNotMatch(playlist, /#EXT-X-(?:MAP|KEY)\b|URI\s*=/i, 'playlist must not reference init maps or external resources');

    const reused = await request(`/api/mobile-hls/file-11${query}`);
    assert.equal(reused.status, 200);
    assert.equal(reused.headers.get('x-luma-hls-session'), sessionId, 'same player and options should reuse the encoder');
    await reused.arrayBuffer();
    assert.equal(transcoder.active.size, 1, 'reusing a session must not start another encoder');

    const segment = await request(`/api/mobile-hls/session/${sessionId}/${segmentName}`);
    assert.equal(segment.status, 200);
    assert.equal(segment.headers.get('content-type'), 'video/mp2t');
    assert.equal(segment.headers.get('x-luma-media-identity'), identity);
    const bytes = Buffer.from(await segment.arrayBuffer());
    assert.ok(bytes.length > 0 && bytes.length <= 32 * 1024 * 1024);
    assert.equal(bytes[0], 0x47, 'MPEG-TS segment should start with a sync byte');
    assert.equal(Number(segment.headers.get('content-length')), bytes.length);

    const wrongIdentity = await request(`/api/mobile-hls/session/${sessionId}/index.m3u8`, 'GET', {
      'X-Luma-Media-Identity': 'f'.repeat(64),
    });
    assert.equal(wrongIdentity.status, 404, 'a session must remain bound to its media identity');
    const invalidSegment = await request(`/api/mobile-hls/session/${sessionId}/segment-999999.ts`);
    assert.equal(invalidSegment.status, 404, 'segment IDs must resolve only to files produced by this session');

    const invalidSeek = await request('/api/mobile-hls/file-11?start=999&quality=480p&bitrate=800000&session=player-test', 'HEAD');
    assert.equal(invalidSeek.status, 416, 'seek positions past duration must be rejected before starting an encoder');
    const invalidBitrate = await request('/api/mobile-hls/file-11?start=0&quality=480p&bitrate=123&session=player-test', 'HEAD');
    assert.equal(invalidBitrate.status, 400);
    assert.equal(transcoder.active.size, 1);

    const stopped = await request(`/api/mobile-hls/session/${sessionId}`, 'DELETE');
    assert.equal(stopped.status, 204);
    assert.equal(stopped.headers.get('x-luma-media-identity'), identity);
    assert.equal(transcoder.active.size, 0, 'DELETE should stop and release the FFmpeg process');
  } finally {
    await mobileHls?.close();
    if (server) await new Promise(resolve => server.close(resolve));
    await rm(workspace, { recursive: true, force: true });
  }
});
