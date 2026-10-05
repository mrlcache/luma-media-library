// Own the fragmented MP4 request and declare its full duration explicitly.
// This avoids browser range requests competing for the same encoder session.
export function openCompatibleStream(video: HTMLVideoElement, url: string, duration: number, onError: (error: Error) => void) {
	const type = 'video/mp4; codecs="avc1.640028, mp4a.40.2"';
	if (typeof MediaSource === 'undefined' || !MediaSource.isTypeSupported(type)) throw new Error('Update Android System WebView to play converted video.');
	const controller = new AbortController();
	const source = new MediaSource();
	const objectUrl = URL.createObjectURL(source);
	let stopped = false;
	const abortError = () => new DOMException('Stream cancelled', 'AbortError');
	function waitFor(target: EventTarget, event: string) {
		return new Promise<void>((resolve, reject) => {
			const cleanup = () => { target.removeEventListener(event, done); target.removeEventListener('error', fail); controller.signal.removeEventListener('abort', cancel); };
			const done = () => { cleanup(); resolve(); };
			const fail = () => { cleanup(); reject(new Error('Could not decode the converted stream.')); };
			const cancel = () => { cleanup(); reject(abortError()); };
			target.addEventListener(event, done, { once: true }); target.addEventListener('error', fail, { once: true }); controller.signal.addEventListener('abort', cancel, { once: true });
			if (controller.signal.aborted) cancel();
		});
	}
	const ready = waitFor(source, 'sourceopen').then(() => {
		if (stopped) throw abortError();
		source.duration = duration;
		const buffer = source.addSourceBuffer(type);
		void (async () => {
			const response = await fetch(url, { signal: controller.signal, cache: 'no-store' });
			if (!response.ok || !response.body) throw new Error(`The computer could not provide the converted video (${response.status}).`);
			const reader = response.body.getReader();
			try {
				while (!stopped) {
					// Keep a bounded buffer instead of retaining the entire film in RAM.
					while (!stopped && buffer.buffered.length && buffer.buffered.end(buffer.buffered.length - 1) - video.currentTime > 30) {
						await new Promise<void>((resolve, reject) => {
							const cancel = () => { clearTimeout(timer); reject(abortError()); };
							const timer = setTimeout(() => { controller.signal.removeEventListener('abort', cancel); resolve(); }, 250);
							controller.signal.addEventListener('abort', cancel, { once: true });
						});
					}
					if (stopped) break;
					if (buffer.buffered.length && video.currentTime > 35 && buffer.buffered.start(0) < video.currentTime - 30) {
						const removed = waitFor(buffer, 'updateend'); buffer.remove(0, video.currentTime - 20); await removed;
					}
					const { value, done } = await reader.read();
					if (done) break;
					const appended = waitFor(buffer, 'updateend'); buffer.appendBuffer(value.slice().buffer); await appended;
				}
				if (!stopped && source.readyState === 'open') source.endOfStream();
			} finally { await reader.cancel().catch(() => {}); }
		})().catch(error => { if (!stopped) onError(error instanceof Error ? error : new Error(String(error))); });
	});
	video.src = objectUrl;
	return { ready, stop() { if (stopped) return; stopped = true; controller.abort(); URL.revokeObjectURL(objectUrl); } };
}
