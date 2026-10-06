import type Hls from 'hls.js';

/** HLS owns segment requests and bounded buffering; the existing UI owns controls. */
export function openHlsStream(video: HTMLVideoElement, url: string, onError: (error: Error) => void) {
	let player: Hls | undefined;
	let stopped = false;
	let settled = false;
	let stopUrl: string | undefined;
	let closeRequested = false;
	let recoveries = 0;
	let resolveReady!: () => void;
	let rejectReady!: (error: Error) => void;
	const ready = new Promise<void>((resolve, reject) => { resolveReady = resolve; rejectReady = reject; });
	const timer = setTimeout(() => fail(new Error('Your computer took too long to prepare the video. Try again.')), 30_000);
	function settle(error?: Error) {
		if (settled) return;
		settled = true;
		clearTimeout(timer);
		if (error) rejectReady(error); else resolveReady();
	}
	function checkReady() {
		if (!stopped && video.buffered.length) settle();
	}
	function closeSession() {
		if (!stopUrl || closeRequested) return;
		closeRequested = true;
		void fetch(stopUrl, { method: 'DELETE', keepalive: true }).catch(() => {});
	}
	function suspend() {
		if (stopped) return;
		player?.stopLoad();
		closeSession();
	}
	function stop() {
		if (stopped) return;
		stopped = true;
		clearTimeout(timer);
		video.removeEventListener('loadeddata', checkReady);
		player?.destroy();
		closeSession();
		settle(new DOMException('Stream cancelled', 'AbortError'));
	}
	function fail(error: Error) {
		if (stopped) return;
		settle(error);
		stop();
		onError(error);
	}
	video.addEventListener('loadeddata', checkReady);
	void import('hls.js').then(({ default: Hls }) => {
		if (stopped) return;
		if (!Hls.isSupported()) {
			fail(new Error('This Android WebView cannot play HLS video. Update Android System WebView.'));
			return;
		}
		player = new Hls({
			maxBufferLength: 20, maxMaxBufferLength: 30, backBufferLength: 20,
			maxBufferSize: 20 * 1024 * 1024, enableWorker: false,
			startPosition: 0, liveSyncDurationCount: 10000, liveMaxLatencyDurationCount: Infinity
		});
		player.on(Hls.Events.FRAG_LOADING, (_, data) => {
			const fragment = new URL(data.frag.url, url);
			if (/\/hls\/session\/[a-z0-9-]+\/segment-\d+\.ts$/.test(fragment.pathname)) {
				fragment.pathname = fragment.pathname.replace(/segment-\d+\.ts$/, 'index.m3u8');
				stopUrl = fragment.toString();
			}
		});
		player.on(Hls.Events.FRAG_BUFFERED, checkReady);
		player.on(Hls.Events.ERROR, (_, data) => {
			if (!data.fatal || stopped) return;
			if (data.type === Hls.ErrorTypes.MEDIA_ERROR && recoveries++ < 1) { player?.recoverMediaError(); return; }
			if (data.type === Hls.ErrorTypes.NETWORK_ERROR && recoveries++ < 2) { player?.startLoad(video.currentTime); return; }
			fail(new Error(data.type === Hls.ErrorTypes.NETWORK_ERROR
				? 'The video segments could not be loaded from your computer. Try again to continue.'
				: 'The converted video could not be decoded on your phone.'));
		});
		player.attachMedia(video);
		player.loadSource(url);
	}).catch(error => fail(error instanceof Error ? error : new Error(String(error))));
	return { ready, stop, suspend };
}
