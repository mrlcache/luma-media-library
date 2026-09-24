const MAX_THUMBNAILS = 96;
const thumbnails = new Map<string, Promise<string | null>>();
let extractionQueue: Promise<void> = Promise.resolve();

export function extractOpeningThumbnail(source: string, cacheKey: string): Promise<string | null> {
	const cached = thumbnails.get(cacheKey);
	if (cached) {
		thumbnails.delete(cacheKey);
		thumbnails.set(cacheKey, cached);
		return cached;
	}

	const result = extractionQueue.then(() => captureOpeningFrame(source, cacheKey));
	extractionQueue = result.then(() => undefined, () => undefined);
	thumbnails.set(cacheKey, result);
	while (thumbnails.size > MAX_THUMBNAILS) {
		const oldest = thumbnails.keys().next().value;
		if (oldest === undefined) break;
		thumbnails.delete(oldest);
	}
	return result;
}

async function captureOpeningFrame(source: string, cacheKey: string): Promise<string | null> {
	const video = document.createElement('video');
	video.muted = true;
	video.preload = 'metadata';
	video.playsInline = true;
	video.crossOrigin = 'anonymous';
	try {
		await waitFor(video, 'loadedmetadata', () => {
			video.src = source;
			video.load();
		});
		if (!Number.isFinite(video.duration) || video.duration < 2) return null;

		const seed = [...cacheKey].reduce((sum, char) => (sum * 31 + char.charCodeAt(0)) >>> 0, 7);
		const openingWindow = Math.max(0.5, Math.min(60, video.duration * 0.05));
		const ratios = [0.18, 0.5, 0.82];
		const times = [...new Set(ratios.map((ratio, index) => Math.min(video.duration - 0.25, openingWindow, openingWindow * ratio + ((seed >>> (index * 4)) % 3))))];
		for (const time of times) {
			await seek(video, Math.max(0.1, time));
			if (isUsefulFrame(video)) return makeThumbnail(video);
		}
		return null;
	} catch {
		return null;
	} finally {
		video.pause();
		video.removeAttribute('src');
		video.load();
	}
}

function waitFor(video: HTMLVideoElement, eventName: 'loadedmetadata', start: () => void): Promise<void> {
	return new Promise((resolve, reject) => {
		const timeout = window.setTimeout(() => finish(new Error('Video metadata timeout')), 12_000);
		const onReady = () => finish();
		const onError = () => finish(new Error('Video format is not available for thumbnail preview'));
		function finish(error?: Error) {
			window.clearTimeout(timeout);
			video.removeEventListener(eventName, onReady);
			video.removeEventListener('error', onError);
			if (error) reject(error);
			else resolve();
		}
		video.addEventListener(eventName, onReady, { once: true });
		video.addEventListener('error', onError, { once: true });
		start();
	});
}

function seek(video: HTMLVideoElement, time: number): Promise<void> {
	return new Promise((resolve, reject) => {
		const timeout = window.setTimeout(() => finish(new Error('Video seek timeout')), 12_000);
		const onSeeked = () => finish();
		const onError = () => finish(new Error('Video frame decode failed'));
		function finish(error?: Error) {
			window.clearTimeout(timeout);
			video.removeEventListener('seeked', onSeeked);
			video.removeEventListener('error', onError);
			if (error) reject(error);
			else resolve();
		}
		video.addEventListener('seeked', onSeeked, { once: true });
		video.addEventListener('error', onError, { once: true });
		video.currentTime = time;
	});
}

function isUsefulFrame(video: HTMLVideoElement): boolean {
	const canvas = document.createElement('canvas');
	canvas.width = 40;
	canvas.height = 23;
	const context = canvas.getContext('2d', { willReadFrequently: true });
	if (!context || video.videoWidth === 0 || video.videoHeight === 0) return false;
	try {
		context.drawImage(video, 0, 0, canvas.width, canvas.height);
		const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
		let sum = 0;
		let sumSquares = 0;
		let dark = 0;
		let bright = 0;
		const count = pixels.length / 4;
		for (let index = 0; index < pixels.length; index += 4) {
			const luminance = pixels[index] * 0.2126 + pixels[index + 1] * 0.7152 + pixels[index + 2] * 0.0722;
			sum += luminance;
			sumSquares += luminance * luminance;
			if (luminance < 12) dark++;
			if (luminance > 243) bright++;
		}
		const average = sum / count;
		const deviation = Math.sqrt(Math.max(0, sumSquares / count - average * average));
		return !((average < 15 && dark / count > 0.94) || (average > 240 && bright / count > 0.94) || deviation < 2.5);
	} catch {
		return false;
	}
}

function makeThumbnail(video: HTMLVideoElement): string | null {
	const canvas = document.createElement('canvas');
	canvas.width = 480;
	canvas.height = 270;
	const context = canvas.getContext('2d');
	if (!context) return null;
	try {
		const scale = Math.max(canvas.width / video.videoWidth, canvas.height / video.videoHeight);
		const width = video.videoWidth * scale;
		const height = video.videoHeight * scale;
		context.drawImage(video, (canvas.width - width) / 2, (canvas.height - height) / 2, width, height);
		return canvas.toDataURL('image/jpeg', 0.76);
	} catch {
		return null;
	}
}
