<script lang="ts">
	import HouseIcon from 'phosphor-svelte/lib/HouseIcon';
	import SquaresFourIcon from 'phosphor-svelte/lib/SquaresFourIcon';
	import FilmSlateIcon from 'phosphor-svelte/lib/FilmSlateIcon';
	import TelevisionIcon from 'phosphor-svelte/lib/TelevisionIcon';
	import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
	import GearSixIcon from 'phosphor-svelte/lib/GearSixIcon';
	import DotsThreeIcon from 'phosphor-svelte/lib/DotsThreeIcon';
	import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
	import PauseIcon from 'phosphor-svelte/lib/PauseIcon';
	import XIcon from 'phosphor-svelte/lib/XIcon';
	import ArrowLeftIcon from 'phosphor-svelte/lib/ArrowLeftIcon';
	import CaretRightIcon from 'phosphor-svelte/lib/CaretRightIcon';
	import CaretDownIcon from 'phosphor-svelte/lib/CaretDownIcon';
	import BookmarkSimpleIcon from 'phosphor-svelte/lib/BookmarkSimpleIcon';
	import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
	import SpeakerHighIcon from 'phosphor-svelte/lib/SpeakerHighIcon';
	import CornersOutIcon from 'phosphor-svelte/lib/CornersOutIcon';
	import ClosedCaptioningIcon from 'phosphor-svelte/lib/ClosedCaptioningIcon';
	import WaveformIcon from 'phosphor-svelte/lib/WaveformIcon';
	import InfoIcon from 'phosphor-svelte/lib/InfoIcon';
	import SlidersHorizontalIcon from 'phosphor-svelte/lib/SlidersHorizontalIcon';
	import ArrowClockwiseIcon from 'phosphor-svelte/lib/ArrowClockwiseIcon';

	export type IconName =
		| 'home'
		| 'library'
		| 'film'
		| 'tv'
		| 'search'
		| 'settings'
		| 'more'
		| 'play'
		| 'pause'
		| 'close'
		| 'arrow-left'
		| 'chevron-right'
		| 'chevron-down'
		| 'bookmark'
		| 'check'
		| 'volume'
		| 'fullscreen'
		| 'captions'
		| 'audio'
		| 'info'
		| 'sliders'
		| 'refresh';

	type IconWeight = 'thin' | 'light' | 'regular' | 'bold' | 'fill' | 'duotone';
	type Props = { name: IconName; size?: number; weight?: IconWeight; mirrored?: boolean };

	let { name, size = 18, weight = 'regular', mirrored = false }: Props = $props();

	const icons = {
		home: HouseIcon,
		library: SquaresFourIcon,
		film: FilmSlateIcon,
		tv: TelevisionIcon,
		search: MagnifyingGlassIcon,
		settings: GearSixIcon,
		more: DotsThreeIcon,
		play: PlayIcon,
		pause: PauseIcon,
		close: XIcon,
		'arrow-left': ArrowLeftIcon,
		'chevron-right': CaretRightIcon,
		'chevron-down': CaretDownIcon,
		bookmark: BookmarkSimpleIcon,
		check: CheckIcon,
		volume: SpeakerHighIcon,
		fullscreen: CornersOutIcon,
		captions: ClosedCaptioningIcon,
		audio: WaveformIcon,
		info: InfoIcon,
		sliders: SlidersHorizontalIcon,
		refresh: ArrowClockwiseIcon
	};

	let Glyph = $derived(icons[name]);
</script>

<Glyph {size} {weight} {mirrored} color="currentColor" aria-hidden="true" />
