const MIME_TYPE = 'video/mp4; codecs="avc1.640028, mp4a.40.2"';
const STARTUP_TIMEOUT_MS = 20_000;

function reportDevStream(event: Record<string, unknown>) {
	if (typeof window === 'undefined' || window.location.port !== '1424' || window.location.hostname !== '192.168.1.2') return;
	// Temporary Wi-Fi DEV diagnostics: never send signed URLs or credentials.
	void fetch('http://192.168.1.2:1425/stream-events', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify(event) }).catch(() => {});
}

export type CompatibleStreamOptions = {
	startupTimeoutMs?: number;
};

export async function fetchCompatibleMedia(url: URL | string, options: RequestInit, stage: 'prepare' | 'receive') {
	try {
		return await fetch(url, options);
	} catch (error) {
		if (options.signal?.aborted) throw error;
		if (!(error instanceof TypeError)) throw error;
		const health = new URL('/api/v1/health', url);
		let reachable = false;
		try {
			const response = await fetch(health, { cache: 'no-store', signal: AbortSignal.timeout(4000) });
			reachable = response.ok;
		} catch { /* Distinguish a lost computer connection from a stream-specific failure. */ }
		throw new Error(reachable
			? stage === 'prepare'
				? 'Your computer is connected, but the transcoding preparation request failed.'
				: 'Your computer is connected, but the converted video request failed.'
			: 'The player could not reach your computer to load the converted video. Check the computer connection and Wi-Fi.');
	}
}

export function openCompatibleStream(
	video: HTMLVideoElement,
	url: string,
	duration: number,
	onError: (error: Error) => void,
	options: CompatibleStreamOptions = {}
) {
	if (typeof MediaSource === 'undefined' || !MediaSource.isTypeSupported(MIME_TYPE)) {
		throw new Error('Update Android System WebView to play converted video.');
	}

	const controller = new AbortController();
	const source = new MediaSource();
	const objectUrl = URL.createObjectURL(source);
	const startupTimeoutMs = options.startupTimeoutMs ?? STARTUP_TIMEOUT_MS;
	let stopped = false;
	let readySettled = false;
	let startupTimer: ReturnType<typeof setTimeout>;
	let activeReader: ReadableStreamDefaultReader<Uint8Array> | undefined;
	let receivedBytes = 0;
	let receivedChunks = 0;
	const requestPath = (() => { try { return new URL(url).pathname; } catch { return '/stream'; } })();
	reportDevStream({phase:'start', path:requestPath, duration});
	let resolveReady!: () => void;
	let rejectReady!: (error: Error) => void;

	const ready = new Promise<void>((resolve, reject) => {
		resolveReady = resolve;
		rejectReady = reject;
	});
	const abortError = () => new DOMException('Stream cancelled', 'AbortError');

	function settleReady(error?: Error) {
		if (readySettled) return;
		readySettled = true;
		clearTimeout(startupTimer);
		if (error) rejectReady(error);
		else resolveReady();
	}

	function stop() {
		if (stopped) return;
		stopped = true;
		clearTimeout(startupTimer);
		controller.abort();
		void activeReader?.cancel().catch(() => {});
		URL.revokeObjectURL(objectUrl);
		settleReady(abortError());
	}

	function fail(error: unknown) {
		if (stopped) return;
		const failure = error instanceof Error ? error : new Error(String(error));
		reportDevStream({phase:'failure', path:requestPath, receivedBytes, receivedChunks, error:failure.message, duration, currentTime:video.currentTime});
		settleReady(failure);
		stop();
		try {
			onError(failure);
		} catch {
			// An observer must not leave the stream request running.
		}
	}

	function waitFor(target: EventTarget, event: string) {
		return new Promise<void>((resolve, reject) => {
			const cleanup = () => {
				target.removeEventListener(event, done);
				target.removeEventListener('error', failEvent);
				controller.signal.removeEventListener('abort', cancel);
			};
			const done = () => {
				cleanup();
				resolve();
			};
			const failEvent = () => {
				cleanup();
				reject(new Error('Could not decode the converted stream.'));
			};
			const cancel = () => {
				cleanup();
				reject(abortError());
			};
			target.addEventListener(event, done, { once: true });
			target.addEventListener('error', failEvent, { once: true });
			controller.signal.addEventListener('abort', cancel, { once: true });
			if (controller.signal.aborted) cancel();
		});
	}

	startupTimer = setTimeout(() => {
		fail(new Error('The converted stream did not send media data in time. Try again.'));
	}, startupTimeoutMs);

	void (async () => {
		await waitFor(source, 'sourceopen');
		if (stopped) throw abortError();

		source.duration = duration;
		const buffer = source.addSourceBuffer(MIME_TYPE);
		const response = await fetchCompatibleMedia(url, { signal: controller.signal, cache: 'no-store' }, 'receive');
		reportDevStream({phase:'response', path:requestPath, status:response.status, length:response.headers?.get('content-length'), type:response.headers?.get('content-type')});
		if (!response.ok || !response.body) {
			throw new Error(`The computer could not provide the converted video (${response.status}).`);
		}

		const reader = response.body.getReader();
		activeReader = reader;
		let appendedData = false;
		try {
			while (!stopped) {
				// Keep a bounded buffer instead of retaining the entire film in RAM.
				while (!stopped && buffer.buffered.length && buffer.buffered.end(buffer.buffered.length - 1) - video.currentTime > 30) {
					await new Promise<void>((resolve, reject) => {
						const cancel = () => {
							clearTimeout(timer);
							reject(abortError());
						};
						const timer = setTimeout(() => {
							controller.signal.removeEventListener('abort', cancel);
							resolve();
						}, 250);
						controller.signal.addEventListener('abort', cancel, { once: true });
					});
				}
				if (stopped) break;

				if (buffer.buffered.length && video.currentTime > 35 && buffer.buffered.start(0) < video.currentTime - 30) {
					const removed = waitFor(buffer, 'updateend');
					try {
						buffer.remove(0, video.currentTime - 20);
					} catch (error) {
						void removed.catch(() => {});
						throw error;
					}
					await removed;
				}

				const { value, done } = await reader.read();
				if (done) break;
				if (!value?.byteLength) continue;
				receivedBytes += value.byteLength;
				receivedChunks++;

				const appended = waitFor(buffer, 'updateend');
				try {
					buffer.appendBuffer(value.slice().buffer);
				} catch (error) {
					void appended.catch(() => {});
					throw error;
				}
				await appended;
				if (!appendedData && buffer.buffered.length) {
					appendedData = true;
					settleReady();
				}
			}

			if (stopped) return;
			reportDevStream({phase:'end', path:requestPath, receivedBytes, receivedChunks, buffered:buffer.buffered.length, duration, currentTime:video.currentTime});
			if (!appendedData) throw new Error('The converted stream ended before media data arrived.');
			if (source.readyState === 'open') source.endOfStream();
		} finally {
			await reader.cancel().catch(() => {});
			if (activeReader === reader) activeReader = undefined;
		}
	})().catch(error => {
		if (!stopped) fail(error);
	});

	video.src = objectUrl;
	return { ready, stop };
}
