import { spawn } from 'node:child_process';
import { mkdir, readFile, rm, stat } from 'node:fs/promises';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { setTimeout as delay } from 'node:timers/promises';
import { mobileQualities } from './transcode.mjs';

const PLAYLIST_TYPE = 'application/vnd.apple.mpegurl';
const SEGMENT_TYPE = 'video/mp2t';
const MAX_SEGMENT_BYTES = 32 * 1024 * 1024;
const IDENTITY_PATTERN = /^[a-f0-9]{64}$/;
const PLAYER_SESSION_PATTERN = /^[a-z0-9-]{1,64}$/;
const SESSION_ID_PATTERN = /^[a-f0-9-]{36}$/;
const SEGMENT_PATTERN = /^segment-(\d{6})\.ts$/;

class HlsHttpError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

function send(res, status, body, contentType = 'text/plain; charset=utf-8', extra = {}) {
  const data = Buffer.isBuffer(body) ? body : Buffer.from(body);
  res.writeHead(status, {
    'Content-Type': contentType,
    'Content-Length': data.length,
    'Cache-Control': 'no-store',
    ...extra,
  });
  res.end(data);
}

function sendError(res, status, message) {
  send(res, status, JSON.stringify({ error: message }), 'application/json; charset=utf-8');
}

function validateIdentity(req) {
  const identity = req.headers['x-luma-media-identity'];
  if (typeof identity !== 'string' || !IDENTITY_PATTERN.test(identity)) {
    throw new HlsHttpError(400, 'A valid media identity is required.');
  }
  return identity;
}

function validateOptions(url) {
  const startText = url.searchParams.get('start') ?? '0';
  const start = Number(startText);
  if (!Number.isFinite(start) || start < 0 || start > 86400) {
    throw new HlsHttpError(400, 'Invalid start position.');
  }
  const quality = url.searchParams.get('quality') ?? 'auto';
  if (quality !== 'auto' && !Object.hasOwn(mobileQualities, quality)) {
    throw new HlsHttpError(400, 'Unsupported quality.');
  }
  const bitrateText = url.searchParams.get('bitrate') ?? '0';
  if (!/^\d+$/.test(bitrateText)) throw new HlsHttpError(400, 'Invalid bitrate.');
  const bitrate = Number(bitrateText);
  const allowed = quality === 'auto' ? [0] : [0, ...mobileQualities[quality].bitrates];
  if (!Number.isSafeInteger(bitrate) || !allowed.includes(bitrate)) {
    throw new HlsHttpError(400, 'Invalid bitrate for this quality.');
  }
  const playerSession = url.searchParams.get('session') ?? '';
  if (!PLAYER_SESSION_PATTERN.test(playerSession)) {
    throw new HlsHttpError(400, 'Invalid player session.');
  }
  return { start: Math.round(start * 1000) / 1000, quality, bitrate, playerSession };
}

function safeSegmentNames(playlist) {
  const names = [];
  for (const line of playlist.split(/\r?\n/)) {
    const value = line.trim();
    if (!value || value.startsWith('#')) {
      if (/^#EXT-X-(?:KEY|MAP|SESSION-KEY)\b/i.test(value) || /\bURI\s*=/i.test(value)) {
        throw new HlsHttpError(502, 'The HLS playlist contains an unsupported resource reference.');
      }
      continue;
    }
    if (!SEGMENT_PATTERN.test(value)) {
      throw new HlsHttpError(502, 'The HLS playlist contains an invalid segment reference.');
    }
    names.push(value);
  }
  return names;
}

export class MobileHls {
  #media;
  #root;
  #sessions = new Map();
  #keys = new Map();
  #players = new Map();
  #locks = new Map();
  #cleanupTimer;
  #closed = false;

  constructor({ media, workspace, idleTimeoutMs = 45_000, cleanupIntervalMs = 15_000, startupTimeoutMs = 30_000, maxSessions = 6, maxEncoders = 2 }) {
    if (!media?.catalog || !media?.transcoder) throw new TypeError('MobileHls requires a media server.');
    if (!workspace || !path.isAbsolute(workspace)) throw new TypeError('MobileHls requires an absolute workspace path.');
    this.#media = media;
    this.#root = path.resolve(workspace, 'mobile-hls');
    this.idleTimeoutMs = Math.max(10_000, idleTimeoutMs);
    this.startupTimeoutMs = Math.max(1_000, startupTimeoutMs);
    this.maxSessions = Math.max(1, maxSessions);
    this.maxEncoders = Math.max(1, maxEncoders);
    mkdirSync(this.#root, { recursive: true });
    this.#cleanupTimer = setInterval(() => { void this.#cleanupIdle(); }, Math.max(5_000, cleanupIntervalMs));
    this.#cleanupTimer.unref?.();
  }

  async handle(req, res, url) {
    const initial = /^\/api\/mobile-hls\/file-(\d+)$/.exec(url.pathname);
    const sessionRoute = /^\/api\/mobile-hls\/session\/([a-f0-9-]{36})(?:\/(index\.m3u8|segment-\d{6}\.ts))?$/.exec(url.pathname);
    if (!initial && !sessionRoute) return false;
    if (req.headers['x-luma-control'] !== '1') {
      sendError(res, 403, 'Mobile stream control is required.');
      return true;
    }

    try {
      const identity = validateIdentity(req);
      if (initial) {
        await this.#handleInitial(req, res, url, initial[1], identity);
      } else {
        await this.#handleSession(req, res, sessionRoute[1], sessionRoute[2] ?? '', identity);
      }
    } catch (error) {
      if (res.headersSent || res.destroyed) {
        res.destroy();
      } else if (error instanceof HlsHttpError) {
        sendError(res, error.status, error.message);
      } else {
        sendError(res, 500, 'Mobile HLS request failed.');
      }
    }
    return true;
  }

  async close() {
    if (this.#closed) return;
    this.#closed = true;
    clearInterval(this.#cleanupTimer);
    await Promise.all([...this.#sessions.values()].map(record => this.#removeSession(record, 'server stopped')));
    try { await rm(this.#root, { recursive: false, force: true }); } catch { /* The parent may contain a concurrent instance. */ }
  }

  async #handleInitial(req, res, url, numericId, identity) {
    if (!['GET', 'HEAD'].includes(req.method)) {
      sendError(res, 405, 'Method not allowed.');
      return;
    }
    const options = validateOptions(url);
    const fileId = `file-${numericId}`;
    const file = await this.#media.catalog.matchingFile(fileId, identity);
    if (!file) throw new HlsHttpError(404, 'Media not found.');
    if (!this.#media.transcoder.available) throw new HlsHttpError(503, 'FFmpeg is unavailable.');

    let plan;
    try {
      plan = await this.#media.transcoder.plan(file, 'mp4', options.quality, options.bitrate, options.start);
    } catch (error) {
      throw new HlsHttpError(422, error instanceof Error ? error.message : 'Could not inspect the media file.');
    }
    if (plan.duration > 0 && options.start >= plan.duration) {
      throw new HlsHttpError(416, 'Start position is past the end of the media.');
    }
    const headers = {
      'X-Luma-Media-Identity': identity,
      'X-Luma-Duration': String(plan.duration),
      'X-Luma-Source-Bitrate': String(plan.sourceBitrate || 0),
      'X-Luma-Bitrate-Limit': String(plan.profile?.bitrate || mobileQualities['720p'].bitrate),
    };
    if (req.method === 'HEAD') {
      res.writeHead(200, { 'Content-Type': PLAYLIST_TYPE, 'Cache-Control': 'no-store', ...headers });
      res.end();
      return;
    }

    const record = await this.#getOrStartSession(file, fileId, identity, options, plan);
    try {
      await this.#waitForFirstSegment(record);
      record.lastAccessAt = Date.now();
      const playlist = await this.#readPlaylist(record);
      send(res, 200, playlist, PLAYLIST_TYPE, {
        ...headers,
        'X-Luma-Hls-Session': record.id,
      });
    } catch (error) {
      if (record.state === 'failed' || record.state === 'stopped') await this.#removeSession(record, 'failed startup');
      throw error;
    }
  }

  async #handleSession(req, res, sessionId, resource, identity) {
    if (req.method === 'DELETE' && !resource) {
      const record = this.#sessions.get(sessionId);
      if (!record || record.identity !== identity) throw new HlsHttpError(404, 'HLS session not found.');
      await this.#removeSession(record, 'client stopped');
      res.writeHead(204, { 'Content-Length': '0', 'Cache-Control': 'no-store', 'X-Luma-Media-Identity': identity });
      res.end();
      return;
    }
    if (!['GET', 'HEAD'].includes(req.method)) {
      sendError(res, 405, 'Method not allowed.');
      return;
    }
    const record = this.#sessions.get(sessionId);
    if (!record || record.identity !== identity) throw new HlsHttpError(404, 'HLS session not found.');
    record.lastAccessAt = Date.now();
    if (!resource || resource === 'index.m3u8') {
      await this.#waitForFirstSegment(record);
      const playlist = await this.#readPlaylist(record);
      const headers = {
        'X-Luma-Media-Identity': identity,
        'X-Luma-Hls-Session': record.id,
      };
      if (req.method === 'HEAD') {
        res.writeHead(200, { 'Content-Type': PLAYLIST_TYPE, 'Cache-Control': 'no-store', ...headers });
        res.end();
      } else {
        send(res, 200, playlist, PLAYLIST_TYPE, headers);
      }
      return;
    }

    const match = SEGMENT_PATTERN.exec(resource);
    if (!match) throw new HlsHttpError(404, 'HLS segment not found.');
    const filename = path.join(record.directory, resource);
    let info;
    try { info = await stat(filename); } catch { throw new HlsHttpError(404, 'HLS segment has expired.'); }
    if (!info.isFile() || info.size <= 0 || info.size > MAX_SEGMENT_BYTES) {
      throw new HlsHttpError(413, 'HLS segment exceeds the supported size.');
    }
    if (req.method === 'HEAD') {
      res.writeHead(200, {
        'Content-Type': SEGMENT_TYPE,
        'Content-Length': String(info.size),
        'Cache-Control': 'no-store',
        'X-Luma-Media-Identity': identity,
      });
      res.end();
      return;
    }
    let bytes;
    try { bytes = await readFile(filename); } catch { throw new HlsHttpError(404, 'HLS segment has expired.'); }
    if (bytes.length > MAX_SEGMENT_BYTES) throw new HlsHttpError(413, 'HLS segment exceeds the supported size.');
    send(res, 200, bytes, SEGMENT_TYPE, { 'X-Luma-Media-Identity': identity });
  }

  async #getOrStartSession(file, fileId, identity, options, knownPlan) {
    const key = JSON.stringify([fileId, identity, options.start, options.quality, options.bitrate, options.playerSession]);
    const playerKey = JSON.stringify([fileId, identity, options.playerSession]);
    return this.#withPlayerLock(playerKey, async () => {
      const existing = this.#keys.get(key);
      if (existing && this.#sessions.get(existing.id) === existing && existing.state !== 'stopped' && existing.state !== 'failed') {
        existing.lastAccessAt = Date.now();
        return existing;
      }
      const previous = this.#players.get(playerKey);
      if (previous) await this.#removeSession(previous, 'player changed stream options');
      await this.#makeRoom();
      if (this.#closed) throw new HlsHttpError(503, 'HLS service is shutting down.');

      const transcoder = this.#media.transcoder;
      if (transcoder.active.size + transcoder.pending >= this.maxEncoders) {
        throw new HlsHttpError(503, 'The transcoder is busy. Try again shortly.');
      }

      transcoder.pending++;
      let plan;
      let record;
      try {
        plan = knownPlan || await transcoder.plan(file, 'mp4', options.quality, options.bitrate, options.start);
        if (plan.duration > 0 && options.start >= plan.duration) throw new HlsHttpError(416, 'Start position is past the end of the media.');
        const id = randomUUID();
        const directory = path.resolve(this.#root, id);
        if (path.dirname(directory) !== this.#root) throw new HlsHttpError(500, 'Invalid HLS session directory.');
        await mkdir(directory, { recursive: false });
        record = {
          id, key, playerKey, fileId, identity, file, options, plan, directory,
          child: null, closePromise: null, state: 'starting', error: null,
          lastAccessAt: Date.now(), exitCode: null,
        };
        this.#sessions.set(id, record);
        this.#keys.set(key, record);
        this.#players.set(playerKey, record);
        this.#startEncoder(record);
      } catch (error) {
        if (record) await this.#removeSession(record, 'encoder start failed');
        if (error instanceof HlsHttpError) throw error;
        throw new HlsHttpError(422, error instanceof Error ? error.message : 'Could not start HLS transcoding.');
      } finally {
        transcoder.pending = Math.max(0, transcoder.pending - 1);
      }
      return record;
    });
  }

  #startEncoder(record) {
    const transcoder = this.#media.transcoder;
    const profile = record.plan.profile || mobileQualities['720p'];
    const bitrate = record.options.bitrate || profile.bitrate;
    const playlistPath = path.join(record.directory, 'index.m3u8');
    const segmentPath = path.join(record.directory, 'segment-%06d.ts');
    const args = ['-nostdin', '-hide_banner', '-loglevel', 'error', '-re'];
    if (record.options.start > 0) args.push('-ss', String(record.options.start));
    args.push('-i', record.file.path, '-map', '0:v:0', '-map', '0:a:0?', '-sn', '-dn');
    args.push(
      '-c:v', 'libx264', '-preset', 'veryfast', '-tune', 'zerolatency', '-threads', '2',
      '-pix_fmt', 'yuv420p', '-g', '120', '-sc_threshold', '0',
      '-force_key_frames', 'expr:gte(t,n_forced*4)',
      '-vf', `scale=w='min(${profile.width},iw)':h='min(${profile.height},ih)':force_original_aspect_ratio=decrease:force_divisible_by=2`,
      '-b:v', String(bitrate), '-maxrate', String(bitrate), '-bufsize', String(bitrate * 2),
      '-c:a', 'aac', '-b:a', '160k', '-ac', '2',
      '-f', 'hls', '-hls_time', '4', '-hls_list_size', '12', '-hls_delete_threshold', '1',
      '-hls_flags', 'delete_segments+independent_segments+temp_file',
      '-hls_segment_filename', segmentPath, playlistPath,
    );
    let child;
    try {
      child = spawn(transcoder.ffmpeg, args, { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
    } catch (error) {
      throw new HlsHttpError(502, `Could not start FFmpeg: ${error instanceof Error ? error.message : 'unknown error'}`);
    }
    record.child = child;
    transcoder.active.add(child);
    record.closePromise = new Promise(resolve => {
      child.once('close', (code, signal) => {
        record.exitCode = code;
        if (record.state !== 'stopped') record.state = code === 0 ? 'ended' : 'failed';
        if (code !== 0 && record.state !== 'stopped') record.error = 'FFmpeg exited before the HLS stream finished.';
        transcoder.active.delete(child);
        if (record.child === child) record.child = null;
        resolve({ code, signal });
      });
      child.once('error', error => {
        if (record.state !== 'stopped') {
          record.state = 'failed';
          record.error = `Could not start FFmpeg: ${error.message}`;
        }
        transcoder.active.delete(child);
        resolve({ error });
      });
    });
    child.stderr.on('data', chunk => {
      const detail = String(chunk).trim();
      if (detail && record.state !== 'stopped') record.error = detail.slice(-1000);
    });
  }

  async #waitForFirstSegment(record) {
    const deadline = Date.now() + this.startupTimeoutMs;
    while (Date.now() < deadline) {
      if (record.state === 'stopped') throw new HlsHttpError(410, 'HLS session was stopped.');
      if (record.state === 'failed' && record.exitCode !== null) {
        throw new HlsHttpError(502, record.error || 'FFmpeg failed before producing an HLS segment.');
      }
      try {
        const playlist = await readFile(path.join(record.directory, 'index.m3u8'), 'utf8');
        const names = safeSegmentNames(playlist);
        for (const name of names) {
          const filename = path.join(record.directory, name);
          const info = await stat(filename);
          if (info.isFile() && info.size > 0 && info.size <= MAX_SEGMENT_BYTES) return;
        }
      } catch (error) {
        if (error instanceof HlsHttpError) throw error;
      }
      if (record.state === 'ended') throw new HlsHttpError(502, 'FFmpeg ended without producing an HLS segment.');
      await delay(100);
    }
    await this.#removeSession(record, 'HLS startup timed out');
    throw new HlsHttpError(504, 'FFmpeg did not produce an HLS segment in time.');
  }

  async #readPlaylist(record) {
    if (record.state === 'stopped') throw new HlsHttpError(410, 'HLS session was stopped.');
    let content;
    try { content = await readFile(path.join(record.directory, 'index.m3u8'), 'utf8'); }
    catch { throw new HlsHttpError(503, 'The HLS playlist is not available yet.'); }
    const names = safeSegmentNames(content);
    if (!names.length) throw new HlsHttpError(503, 'The HLS playlist has no completed segments yet.');
    return `${content.trimEnd()}\n`;
  }

  async #makeRoom() {
    await this.#cleanupIdle();
    if (this.#sessions.size < this.maxSessions) return;
    const candidates = [...this.#sessions.values()].sort((a, b) => a.lastAccessAt - b.lastAccessAt);
    for (const record of candidates) {
      if (record.state === 'ended' || record.state === 'failed' || Date.now() - record.lastAccessAt >= this.idleTimeoutMs) {
        await this.#removeSession(record, 'session capacity');
        if (this.#sessions.size < this.maxSessions) return;
      }
    }
    throw new HlsHttpError(503, 'Too many HLS sessions are active.');
  }

  async #cleanupIdle() {
    const cutoff = Date.now() - this.idleTimeoutMs;
    await Promise.all([...this.#sessions.values()]
      .filter(record => record.lastAccessAt <= cutoff)
      .map(record => this.#removeSession(record, 'idle timeout')));
  }

  async #removeSession(record, reason) {
    if (!record || record.removing) return record?.removing;
    record.removing = (async () => {
      record.state = 'stopped';
      record.stopReason = reason;
      const child = record.child;
      if (child && child.exitCode === null) {
        try { child.kill(); } catch { /* The process may already be exiting. */ }
        const first = await Promise.race([record.closePromise, delay(1000).then(() => null)]);
        if (!first && child.exitCode === null) {
          try { child.kill('SIGKILL'); } catch { /* The process may already be gone. */ }
          await Promise.race([record.closePromise, delay(1000)]);
        }
      }
      if (this.#sessions.get(record.id) === record) this.#sessions.delete(record.id);
      if (this.#keys.get(record.key) === record) this.#keys.delete(record.key);
      if (this.#players.get(record.playerKey) === record) this.#players.delete(record.playerKey);
      await rm(record.directory, { recursive: true, force: true }).catch(() => {});
    })();
    return record.removing;
  }

  async #withPlayerLock(key, action) {
    const previous = this.#locks.get(key) || Promise.resolve();
    let unlock;
    const gate = new Promise(resolve => { unlock = resolve; });
    const chain = previous.then(() => gate);
    this.#locks.set(key, chain);
    await previous;
    try { return await action(); }
    finally {
      unlock();
      if (this.#locks.get(key) === chain) this.#locks.delete(key);
    }
  }
}
