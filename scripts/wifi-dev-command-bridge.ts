import { readFile } from 'node:fs/promises';
import { isIP } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Plugin } from 'vite';

const COMMAND_PATH = '/__luma-wifi-dev/command';
const STATUS_PATH = '/__luma-wifi-dev/status';
const BRIDGE_DEFAULT_PORT = 47631;
const MAX_REQUEST_BYTES = 8 * 1024 * 1024;

// Keep this in sync with the mobile-facing operations in mobile_bridge::dispatch.
// Pairing, settings secrets, scans, and unrelated desktop commands are not exposed.
const ALLOWED_COMMANDS = new Set([
	'desktop_bootstrap',
	'load_remote_artwork',
	'get_library_status',
	'move_library_title',
	'permanently_delete_library_title',
	'get_catalog_page',
	'get_local_title_detail',
	'resolve_media_file',
	'save_playback_progress',
	'record_playback_activity',
	'get_playback_history',
	'get_continue_watching',
	'search_tmdb',
	'get_discovery_feed',
	'get_discovery_title',
	'get_discovery_trailer',
	'get_discovery_logo',
	'get_title_trailer',
	'get_title_logo',
	'get_search_browse',
	'release_search',
	'torrent_snapshot',
	'torrent_add_magnet',
	'torrent_add_file',
	'torrent_add_data',
	'torrent_set_paused',
	'torrent_move_queue',
	'torrent_set_limits',
	'torrent_remove'
]);

type BridgeCredentials = { port: number; token: string };

function appDataDirectory(): string | null {
	if (process.env.APPDATA) return path.join(process.env.APPDATA, 'local.media.platform');
	if (process.env.XDG_DATA_HOME) return path.join(process.env.XDG_DATA_HOME, 'local.media.platform');
	if (process.platform !== 'win32') return path.join(os.homedir(), '.local', 'share', 'local.media.platform');
	return null;
}

async function readCredentials(): Promise<BridgeCredentials> {
	const directory = appDataDirectory();
	if (!directory) throw new Error('Luma Desktop connection settings are unavailable.');
	const tokenPath = path.join(directory, 'mobile-bridge-token');
	let enabled = '', token = '', portText = String(BRIDGE_DEFAULT_PORT);
	try {
		[enabled, token, portText] = await Promise.all([
			readFile(`${tokenPath}.enabled`, 'utf8'),
			readFile(tokenPath, 'utf8'),
			readFile(`${tokenPath}.port`, 'utf8').catch(() => String(BRIDGE_DEFAULT_PORT))
		]);
	} catch {
		throw new Error('Start Luma Desktop and enable Mobile connection in Settings.');
	}
	if (enabled.trim() !== '1') throw new Error('Enable Mobile connection in Luma Desktop Settings.');
	const normalizedToken = token.trim();
	if (!/^[\da-f]{64}$/i.test(normalizedToken)) throw new Error('The Luma Desktop connection key is unavailable.');
	const port = Number(portText.trim());
	if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error('The Luma Desktop connection port is invalid.');
	return { port, token: normalizedToken };
}

function sendJson(response: ServerResponse, status: number, value: unknown) {
	if (response.headersSent) return;
	const body = JSON.stringify(value);
	response.writeHead(status, {
		'Content-Type': 'application/json; charset=utf-8',
		'Content-Length': Buffer.byteLength(body),
		'Cache-Control': 'no-store',
		'X-Content-Type-Options': 'nosniff'
	});
	response.end(body);
}

function sendError(response: ServerResponse, status: number, message: string) {
	sendJson(response, status, { error: message });
}

async function readJsonBody(request: IncomingMessage, response: ServerResponse): Promise<unknown | null> {
	if (!/^application\/json(?:\s*;|$)/i.test(request.headers['content-type'] ?? '')) {
		sendError(response, 415, 'Content-Type must be application/json.');
		request.resume();
		return null;
	}
	const chunks: Buffer[] = [];
	let bytes = 0;
	let tooLarge = false;
	for await (const chunk of request) {
		const part = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
		bytes += part.byteLength;
		if (bytes > MAX_REQUEST_BYTES) {
			tooLarge = true;
			continue;
		}
		chunks.push(part);
	}
	if (tooLarge) {
		sendError(response, 413, 'Request is too large.');
		return null;
	}
	try {
		return JSON.parse(Buffer.concat(chunks).toString('utf8'));
	} catch {
		sendError(response, 400, 'Request must contain valid JSON.');
		return null;
	}
}

async function forwardCommand(command: string, args: Record<string, unknown>) {
	const { port, token } = await readCredentials();
	let upstream: Response;
	try {
		upstream = await fetch(`http://127.0.0.1:${port}/api/v1/command`, {
			method: 'POST',
			redirect: 'error',
			headers: {
				Authorization: `Bearer ${token}`,
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ command, args }),
			signal: AbortSignal.timeout(45_000)
		});
	} catch {
		throw new Error(`Cannot reach the Luma Desktop bridge on port ${port}.`);
	}
	let body: unknown;
	try {
		body = await upstream.json();
	} catch {
		throw new Error('Luma Desktop returned an invalid response.');
	}
	if (!upstream.ok) {
		const message = typeof body === 'object' && body !== null && 'error' in body && typeof body.error === 'string'
			? body.error
			: 'The Luma Desktop request failed.';
		throw Object.assign(new Error(message), { status: upstream.status });
	}
	return { body, port };
}

export function wifiDevCommandBridge(): Plugin {
	return {
		name: 'luma-wifi-dev-command-bridge',
		apply: 'serve',
		configureServer(server) {
			const host = process.env.LUMA_LAN_HOST ?? '';
			const expectedOrigin = isIP(host) === 4 ? `http://${host}:1424` : '';
			server.middlewares.use((request, response, next) => {
				let pathname = '';
				try { pathname = new URL(request.url ?? '/', 'http://localhost').pathname; }
				catch { next(); return; }
				if (pathname !== COMMAND_PATH && pathname !== STATUS_PATH) { next(); return; }
				if (!expectedOrigin || request.headers.origin !== expectedOrigin) {
					sendError(response, 403, 'This Luma Wi-Fi DEV request has an invalid origin.');
					return;
				}
				if (pathname === STATUS_PATH) {
					if (request.method !== 'POST') {
						response.setHeader('Allow', 'POST');
						sendError(response, 405, 'Method not allowed.');
						return;
					}
					void readJsonBody(request, response).then((body) => {
						if (body === null || response.headersSent) return;
						if (typeof body !== 'object' || Array.isArray(body) || body === null) {
							sendError(response, 400, 'Request must be a JSON object.');
							return;
						}
						void forwardCommand('desktop_bootstrap', {}).then(({ body: result, port }) => {
							sendJson(response, 200, { url: `http://${host}:${port}`, paired: !!result });
						}).catch((error: unknown) => {
							sendError(response, Number((error as { status?: number })?.status) || 503,
								error instanceof Error ? error.message : 'Luma Desktop is unavailable.');
						});
					}).catch(() => sendError(response, 400, 'Could not read the status request.'));
					return;
				}
				if (request.method !== 'POST') {
					response.setHeader('Allow', 'POST');
					sendError(response, 405, 'Method not allowed.');
					return;
				}
				void readJsonBody(request, response).then(async (body) => {
					if (body === null || response.headersSent) return;
					if (typeof body !== 'object' || Array.isArray(body) || body === null) {
						sendError(response, 400, 'Request must be a JSON object.');
						return;
					}
					const command = (body as { command?: unknown }).command;
					const args = (body as { args?: unknown }).args ?? {};
					if (typeof command !== 'string' || !ALLOWED_COMMANDS.has(command)) {
						sendError(response, 403, 'This command is not available in Luma Wi-Fi DEV.');
						return;
					}
					if (typeof args !== 'object' || args === null || Array.isArray(args)) {
						sendError(response, 400, 'Command arguments must be a JSON object.');
						return;
					}
					try {
						const result = await forwardCommand(command, args as Record<string, unknown>);
						sendJson(response, 200, result.body);
					} catch (error) {
						sendError(response, Number((error as { status?: number })?.status) || 502,
							error instanceof Error ? error.message : 'Luma Desktop is unavailable.');
					}
				}).catch(() => sendError(response, 400, 'Could not read the command request.'));
			});
		}
	};
}
