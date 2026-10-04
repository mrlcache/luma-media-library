<script module lang="ts">
	let closeActiveMenu: (() => void) | undefined;
</script>

<script lang="ts" generics="T extends string | number">
	import { tick } from 'svelte';
	import Icon from './Icon.svelte';
	type Props = {
		value: T; options: readonly { value: T; label: string }[]; label: string;
		onchange?: (value: T) => void; variant?: 'field' | 'plain'; disabled?: boolean;
	};
	let { value = $bindable(), options, label, onchange, variant = 'field', disabled = false }: Props = $props();
	let host: HTMLDivElement;
	let trigger: HTMLButtonElement;
	let menu: HTMLDivElement;
	let open = $state(false);
	const supportsPopover = typeof HTMLElement !== 'undefined' && 'showPopover' in HTMLElement.prototype;
	let active = $state(-1);
	let selected = $derived(options.find((option) => option.value === value));
	const uid = $props.id();

	function close() {
		open = false;
		if (supportsPopover && menu?.matches(':popover-open')) menu.hidePopover();
		if (closeActiveMenu === close) closeActiveMenu = undefined;
	}

	async function show() {
		if (disabled) return;
		if (open) { close(); return; }
		closeActiveMenu?.();
		closeActiveMenu = close;
		active = Math.max(0, options.findIndex((option) => option.value === value));
		open = true;
		await tick();
		if (!open || !menu?.isConnected) return;
		const bounds = trigger.getBoundingClientRect();
		menu.style.width = `${Math.min(Math.max(bounds.width, 160), window.innerWidth - 16)}px`;
		if (supportsPopover) menu.showPopover();
		const height = menu.getBoundingClientRect().height;
		const width = menu.getBoundingClientRect().width;
		const below = bounds.bottom + 6;
		menu.style.top = `${Math.max(8, below + height <= window.innerHeight - 8 ? below : bounds.top - height - 6)}px`;
		menu.style.left = `${Math.max(8, Math.min(bounds.left, window.innerWidth - width - 8))}px`;
		menu.focus({ preventScroll: true });
		scrollActiveOption();
	}

	function scrollActiveOption() {
		const option = menu?.querySelector<HTMLElement>('[data-active="true"]');
		if (!option) return;
		if (option.offsetTop < menu.scrollTop) menu.scrollTop = option.offsetTop;
		else if (option.offsetTop + option.offsetHeight > menu.scrollTop + menu.clientHeight) menu.scrollTop = option.offsetTop + option.offsetHeight - menu.clientHeight;
	}

	function choose(index: number) {
		const option = options[index];
		if (!option) return;
		value = option.value;
		onchange?.(option.value);
		close();
		trigger.focus({ preventScroll: true });
	}

	function keyboard(event: KeyboardEvent) {
		if (['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' ', 'Escape'].includes(event.key)) event.stopPropagation();
		if (event.key === 'Escape') { event.preventDefault(); close(); trigger.focus({ preventScroll: true }); }
		else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			if (!open) { void show(); return; }
			active = (active + (event.key === 'ArrowDown' ? 1 : -1) + options.length) % Math.max(1, options.length);
			void tick().then(scrollActiveOption);
		} else if (open && (event.key === 'Home' || event.key === 'End')) {
			event.preventDefault(); active = event.key === 'Home' ? 0 : options.length - 1; void tick().then(scrollActiveOption);
		} else if (open && (event.key === 'Enter' || event.key === ' ')) { event.preventDefault(); choose(active); }
	}

	$effect(() => {
		if (!open) return;
		const outside = (event: PointerEvent) => { if (!host.contains(event.target as Node)) close(); };
		const scroll = (event: Event) => { if (!(event.target instanceof Node) || !menu.contains(event.target)) close(); };
		window.addEventListener('pointerdown', outside, true);
		window.addEventListener('scroll', scroll, true);
		window.addEventListener('resize', close);
		return () => {
			window.removeEventListener('pointerdown', outside, true);
			window.removeEventListener('scroll', scroll, true);
			window.removeEventListener('resize', close);
			close();
		};
	});
</script>

<div bind:this={host} class="app-select" class:app-select--plain={variant === 'plain'}>
	<button bind:this={trigger} class="app-select__trigger" type="button" {disabled} aria-label={`${label}: ${selected?.label ?? ''}`} aria-haspopup="listbox" aria-expanded={open} aria-controls={uid} onclick={() => show()} onkeydown={keyboard}>
		<span>{selected?.label ?? 'Select'}</span><Icon name="chevron-down" size={14} />
	</button>
	<div bind:this={menu} id={uid} class="app-select__menu" class:fallback={!supportsPopover} hidden={!supportsPopover && !open} popover={supportsPopover ? 'manual' : undefined} role="listbox" aria-label={label} aria-activedescendant={active >= 0 ? `${uid}-${active}` : undefined} tabindex="-1" onkeydown={keyboard} data-lenis-prevent>
		{#each options as option, index (option.value)}
			<button id={`${uid}-${index}`} type="button" role="option" aria-selected={value === option.value} data-active={active === index} tabindex="-1" onpointerenter={() => active = index} onclick={() => choose(index)}><span>{option.label}</span>{#if value === option.value}<Icon name="check" size={13} />{/if}</button>
		{/each}
	</div>
</div>

<style>
	.app-select__menu.fallback { z-index: 200; }
	.app-select__menu[hidden] { display:none; }
	.app-select { min-width:0; width:100%; font-size:.72rem; }
	.app-select__trigger { display:flex; align-items:center; justify-content:space-between; gap:12px; width:100%; min-height:36px; padding:0 12px; border:1px solid var(--line-subtle); border-radius:var(--radius-sm); color:var(--text-soft); background:var(--surface-1); font:inherit; text-align:left; cursor:pointer; }
	.app-select__trigger span { overflow:hidden; white-space:nowrap; text-overflow:ellipsis; }
	.app-select__trigger :global(svg) { flex-shrink:0; color:var(--text-muted); }
	.app-select__trigger:hover, .app-select__trigger[aria-expanded='true'] { border-color:var(--line-strong); background:var(--surface-2); }
	.app-select__trigger:disabled { opacity:.45; cursor:default; }
	.app-select--plain { width:auto; }
	.app-select--plain .app-select__trigger { min-height:34px; padding:0 6px; border-color:transparent; background:transparent; }
	.app-select__menu { position:fixed; inset:auto; margin:0; max-height:min(280px,calc(100dvh - 16px)); padding:5px; overflow-y:auto; border:1px solid var(--line-strong); border-radius:9px; color:var(--text-soft); background:#151d24; box-shadow:0 12px 32px rgba(0,0,0,.35); font:inherit; }
	.app-select__menu button { display:flex; align-items:center; justify-content:space-between; gap:16px; width:100%; min-height:34px; padding:7px 10px; border:0; border-radius:5px; background:transparent; color:var(--text-soft); font:inherit; text-align:left; cursor:pointer; }
	.app-select__menu button[data-active='true'] { background:rgba(158,198,214,.1); }
	.app-select__menu button[aria-selected='true'] { color:var(--accent-soft); }
	.app-select__menu :global(svg) { flex-shrink:0; }
</style>
