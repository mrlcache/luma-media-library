export type YouTubePlayer = {
	playVideo(): void;
	pauseVideo(): void;
	mute(): void;
	seekTo(seconds: number, allowSeekAhead: boolean): void;
	destroy(): void;
};

type YouTubePlayerEvent = { target: YouTubePlayer };
type YouTubePlayerStateEvent = YouTubePlayerEvent & { data: number };

export type YouTubeIframeApi = {
	Player: new (iframe: HTMLIFrameElement, options: {
		events: {
			onReady: (event: YouTubePlayerEvent) => void;
			onStateChange: (event: YouTubePlayerStateEvent) => void;
			onError?: (event: YouTubePlayerStateEvent) => void;
		};
	}) => YouTubePlayer;
};

declare global {
	interface Window {
		YT?: YouTubeIframeApi;
		onYouTubeIframeAPIReady?: () => void;
	}
}

let pendingApi: Promise<YouTubeIframeApi> | null = null;

export function loadYouTubeIframeApi(): Promise<YouTubeIframeApi> {
	if (window.YT?.Player) return Promise.resolve(window.YT);
	if (pendingApi) return pendingApi;

	pendingApi = new Promise((resolve, reject) => {
		const script = document.createElement('script');
		const previousReady = window.onYouTubeIframeAPIReady;
		let settled = false;
		let timeout: number;

		const finish = (api?: YouTubeIframeApi) => {
			if (settled) return;
			settled = true;
			window.clearTimeout(timeout);
			window.onYouTubeIframeAPIReady = previousReady;
			if (api) resolve(api);
			else {
				script.remove();
				pendingApi = null;
				reject(new Error('YouTube iframe API did not load'));
			}
		};

		window.onYouTubeIframeAPIReady = () => {
			previousReady?.();
			finish(window.YT?.Player ? window.YT : undefined);
		};
		script.src = 'https://www.youtube.com/iframe_api';
		script.async = true;
		script.onerror = () => finish();
		timeout = window.setTimeout(() => finish(), 12000);
		document.head.appendChild(script);
	});

	return pendingApi;
}
