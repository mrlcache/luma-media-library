import { mkdir, readFile, rename, writeFile, rmdir, stat } from 'node:fs/promises';
import { dirname, join } from 'node:path';

export const RELEASE_CACHE_TTL_MS = 10 * 24 * 60 * 60 * 1000;

export function defaultReleaseCachePath(env = process.env) {
	const appData = env.LUMA_APP_DATA || join(env.APPDATA || env.HOME || '.', 'local.media.platform');
	return join(appData, 'torrent-search-cache.json');
}

/** Persistent sliding-expiry cache for release pages and resolved torrent metadata. */
export function createReleaseCache({ filePath = defaultReleaseCachePath(), now = Date.now } = {}) {
	/** @type {Map<string, { data: any, refreshedAt: number, lastViewedAt: number }>} */
	const entries = new Map();
	/** @type {Promise<void> | undefined} */
	let loadPromise;
	/** @type {Promise<void> | undefined} */
	let pendingWrite;
	let revision = 0;
	let writtenRevision = 0;

	async function lockFile() {
		const lock = `${filePath}.lock`;
		for (let attempt = 0; attempt < 300; attempt++) {
			try { await mkdir(lock); return () => rmdir(lock); }
			catch (error) {
				if (!(error instanceof Error) || !('code' in error) || error.code !== 'EEXIST') throw error;
				const info = await stat(lock).catch(() => null);
				if (info && Date.now() - info.mtimeMs > 30000) await rmdir(lock).catch(() => {});
				await new Promise(resolve => setTimeout(resolve, 20));
			}
		}
		throw new Error('Release cache is busy');
	}

	async function persist() {
		revision++;
		if (!pendingWrite) pendingWrite = Promise.resolve().then(async () => {
			await mkdir(dirname(filePath), { recursive: true });
			while (writtenRevision < revision) {
				const currentRevision = revision;
				const unlock = await lockFile();
				try {
					// Preserve updates made by another running service before writing ours.
					const disk = await readFile(filePath, 'utf8').then(text => { try { return JSON.parse(text); } catch { return {}; } }).catch(() => ({}));
					if (disk.version === 1 && Array.isArray(disk.entries)) for (const pair of disk.entries) {
						if (!Array.isArray(pair) || typeof pair[0] !== 'string') continue;
						const [key, incoming] = pair;
						if (!incoming || !Number.isFinite(incoming.refreshedAt) || !Number.isFinite(incoming.lastViewedAt)) continue;
						const current = entries.get(key);
						const newest = !current || incoming.refreshedAt > current.refreshedAt ? incoming : current;
						entries.set(key, { ...newest, lastViewedAt: Math.max(incoming.lastViewedAt, current?.lastViewedAt ?? 0) });
					}
					for (const [key, entry] of entries) if (now() - entry.lastViewedAt >= RELEASE_CACHE_TTL_MS) entries.delete(key);
					const snapshot = JSON.stringify({ version: 1, entries: [...entries.entries()] });
					const temporary = `${filePath}.${process.pid}.tmp`;
					await writeFile(temporary, snapshot, 'utf8');
					await rename(temporary, filePath);
				} finally { await unlock(); }
				writtenRevision = currentRevision;
			}
		}).finally(() => { pendingWrite = undefined; });
		return pendingWrite;
	}

	async function load() {
		if (!loadPromise) loadPromise = (async () => {
			let dropped = false;
			try {
				const parsed = JSON.parse(await readFile(filePath, 'utf8'));
				if (parsed?.version !== 1 || !Array.isArray(parsed.entries)) return;
				for (const pair of parsed.entries) {
					if (!Array.isArray(pair) || typeof pair[0] !== 'string' || !pair[1] || typeof pair[1] !== 'object') continue;
					const [key, entry] = pair;
					if (!Number.isFinite(entry.lastViewedAt) || !Number.isFinite(entry.refreshedAt) || now() - entry.lastViewedAt >= RELEASE_CACHE_TTL_MS) { dropped = true; continue; }
					entries.set(key, entry);
				}
			} catch { /* A missing or partial cache starts empty and is rebuilt by searches. */ }
			if (dropped) await persist();
		})();
		await loadPromise;
	}

	return {
		/** @param {string} key */
		async get(key) {
			await load();
			const entry = entries.get(key);
			if (!entry) return null;
			if (now() - entry.lastViewedAt >= RELEASE_CACHE_TTL_MS) {
				entries.delete(key);
				await persist();
				return null;
			}
			entry.lastViewedAt = now();
			entries.delete(key);
			entries.set(key, entry);
			void persist().catch(() => {});
			return { data: entry.data, refreshedAt: entry.refreshedAt, lastViewedAt: entry.lastViewedAt };
		},
		/** @param {string} key @param {any} data @param {{ viewed?: boolean }} [options] */
		async set(key, data, { viewed = true } = {}) {
			await load();
			const previous = entries.get(key);
			const timestamp = now();
			entries.delete(key);
			entries.set(key, {
				data,
				refreshedAt: timestamp,
				lastViewedAt: viewed ? timestamp : previous?.lastViewedAt ?? timestamp
			});
			await persist();
		},
		async prune() {
			await load();
			const cutoff = now() - RELEASE_CACHE_TTL_MS;
			let changed = false;
			for (const [key, entry] of entries) {
				if (entry.lastViewedAt <= cutoff) { entries.delete(key); changed = true; }
			}
			if (changed) await persist();
		},
		async flush() { await pendingWrite; }
	};
}
