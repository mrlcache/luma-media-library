<script lang="ts">
	let {
		value = $bindable(0),
		min,
		max,
		step,
		ariaLabel,
		disabled = false,
		oninput
	}: {
		value?: number;
		min: number;
		max: number;
		step: number;
		ariaLabel: string;
		disabled?: boolean;
		oninput?: (event: Event) => void;
	} = $props();

	let stops = $derived(Array.from({ length: Math.round((max - min) / step) + 1 }, (_, index) => min + index * step));
	let progress = $derived(Math.min(100, Math.max(0, (value - min) / (max - min) * 100)));
</script>

<span class="stepped-range" class:disabled style={`--range-progress: ${progress}%;`}>
	<input type="range" {min} {max} {step} bind:value {oninput} {disabled} aria-label={ariaLabel} />
	<span class="stepped-range__ticks" aria-hidden="true">
		{#each stops as stop, index (stop)}
			<span class:passed={value >= stop} style={`left: ${index / (stops.length - 1) * 100}%`}></span>
		{/each}
	</span>
</span>

<style>
	.stepped-range { position: relative; display: flex; align-items: center; width: 100%; min-width: 0; height: 24px; }
	.stepped-range::before { position: absolute; top: 50%; right: 7px; left: 7px; height: 3px; transform: translateY(-50%); border-radius: 999px; background: linear-gradient(to right, var(--accent) var(--range-progress), #46515b var(--range-progress)); content: ''; pointer-events: none; }
	input { position: relative; z-index: 2; width: 100%; height: 24px; margin: 0; padding: 0; appearance: none; border: 0; background: transparent; cursor: pointer; }
	input::-webkit-slider-runnable-track { height: 3px; border: 0; background: transparent; }
	input::-webkit-slider-thumb { width: 14px; height: 14px; margin-top: -5.5px; appearance: none; border: 0; border-radius: 50%; background: var(--accent); }
	input::-moz-range-track { height: 3px; border: 0; background: transparent; }
	input::-moz-range-progress { background: transparent; }
	input::-moz-range-thumb { width: 14px; height: 14px; border: 0; border-radius: 50%; background: var(--accent); }
	.stepped-range__ticks { position: absolute; z-index: 1; inset: 0 7px; pointer-events: none; }
	.stepped-range__ticks > span { position: absolute; top: 50%; width: 5px; height: 5px; transform: translate(-50%, -50%); border: 0; border-radius: 50%; background: #85929d; }
	.stepped-range__ticks > span.passed { background: #d9edf4; }
	.disabled { opacity: 0.45; }
	input:disabled { cursor: not-allowed; }
</style>
