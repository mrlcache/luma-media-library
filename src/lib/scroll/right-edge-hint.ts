export function rightEdgeHint(node: HTMLElement) {
	const update = () => {
		const parent = node.parentElement;
		if (!parent) return;
		parent.dataset.moreRight = String(node.scrollWidth > node.clientWidth + node.scrollLeft + 2);
	};

	const resizeObserver = new ResizeObserver(update);
	resizeObserver.observe(node);
	for (const child of Array.from(node.children)) resizeObserver.observe(child);

	const mutationObserver = new MutationObserver(() => {
		for (const child of Array.from(node.children)) resizeObserver.observe(child);
		update();
	});
	mutationObserver.observe(node, { childList: true });
	node.addEventListener('scroll', update, { passive: true });
	queueMicrotask(update);

	return {
		destroy() {
			mutationObserver.disconnect();
			resizeObserver.disconnect();
			node.removeEventListener('scroll', update);
			if (node.parentElement) delete node.parentElement.dataset.moreRight;
		}
	};
}
