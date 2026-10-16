<script lang="ts">
	import { cssVar } from '../lib/cssvar.ts'
	import { slider, sliderHead, sliderValue, sliderWrap, vars } from '../styles.css.ts'

	interface Props {
		label: string
		min: number
		max: number
		value: number
		/** Fires per input event: optimistic local update + coalesced send. */
		oninput: (value: number) => void
		/** Fires on release (native change): immediate send. */
		oncommit?: (value: number) => void
		/** 'fill' paints an accent progress bar; 'gradient' paints `gradient`. */
		variant?: 'fill' | 'gradient'
		gradient?: string
	}

	let {
		label,
		min,
		max,
		value,
		oninput,
		oncommit,
		variant = 'fill',
		gradient,
	}: Props = $props()

	const pct = $derived(max > min ? ((value - min) / (max - min)) * 100 : 0)
	// Dynamic track paint must be inline (per-instance value), hence the raw
	// var() references via cssVar.
	const trackImage = $derived(
		variant === 'gradient' && gradient !== undefined
			? gradient
			: `linear-gradient(90deg, ${cssVar(vars.color.accent)} ${pct}%, ${cssVar(vars.color.surfaceAlt)} ${pct}%)`,
	)
</script>

<label class={sliderWrap}>
	<span class={sliderHead}>
		<span>{label}</span>
		<span class={sliderValue}>{value}</span>
	</span>
	<input
		type="range"
		class={slider}
		style:--slider-image={trackImage}
		{min}
		{max}
		{value}
		aria-label={label}
		oninput={(e) => oninput(e.currentTarget.valueAsNumber)}
		onchange={(e) => oncommit?.(e.currentTarget.valueAsNumber)}
	/>
</label>
