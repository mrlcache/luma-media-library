import type Lenis from 'lenis';
import { isDesktopRuntime } from '$lib/platform/desktop';
import { isMobilePreview } from '$lib/platform/mobile-preview';

const instances = new Set<Lenis>();
let frame: number | undefined;

function animate(time: number) {
	frame = undefined;
	for (const instance of instances) instance.raf(time);
	if (instances.size) frame = requestAnimationFrame(animate);
}

export function registerLenis(instance: Lenis): () => void {
	instances.add(instance);
	if (frame === undefined) frame = requestAnimationFrame(animate);
	return () => {
		instances.delete(instance);
		instance.destroy();
		if (!instances.size && frame !== undefined) {
			cancelAnimationFrame(frame);
			frame = undefined;
		}
	};
}

export function smoothHorizontalScroll(node: HTMLElement) {
	if (!isDesktopRuntime() || isMobilePreview()) return;

	let disposed = false;
	let loading = false;
	let release: (() => void) | undefined;
	const update = () => {
		if (disposed) return;
		if (node.scrollWidth <= node.clientWidth + 1) {
			release?.();
			release = undefined;
			return;
		}
		if (release || loading) return;
		loading = true;
		void import('lenis')
			.then(({ default: Lenis }) => {
				if (disposed || node.scrollWidth <= node.clientWidth + 1) return;
				const instance = new Lenis({
					wrapper: node,
					content: node,
					eventsTarget: node,
					orientation: 'horizontal',
					gestureOrientation: 'horizontal',
					autoRaf: false,
					lerp: 0.13,
					syncTouch: false,
					overscroll: false,
					virtualScroll: (gesture) => {
						const { event } = gesture;
						if (!(event instanceof WheelEvent) || event.ctrlKey) return false;
						if (event.shiftKey && gesture.deltaX === 0) {
							gesture.deltaX = gesture.deltaY;
							gesture.deltaY = 0;
						}
						return gesture.deltaX !== 0 && Math.abs(gesture.deltaX) >= Math.abs(gesture.deltaY);
					}
				});
				release = registerLenis(instance);
			})
			.catch((error) => console.warn('Horizontal smooth scrolling unavailable', error))
			.finally(() => { loading = false; });
	};

	const observer = new ResizeObserver(update);
	observer.observe(node);
	queueMicrotask(update);
	return {
		destroy() {
			disposed = true;
			observer.disconnect();
			release?.();
		}
	};
}
