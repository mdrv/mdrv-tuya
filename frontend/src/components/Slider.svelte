<script lang="ts">
	import { cssVar } from '../lib/cssvar.ts'
	import {
		slider,
		sliderHead,
		sliderValueButton,
		sliderValueInput,
		sliderWrap,
		vars,
	} from '../styles.css.ts'

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

	const uid = $props.id()

	const pct = $derived(max > min ? ((value - min) / (max - min)) * 100 : 0)
	// Dynamic track paint must be inline (per-instance value), hence the raw
	// var() references via cssVar.
	const trackImage = $derived(
		variant === 'gradient' && gradient !== undefined
			? gradient
			: `linear-gradient(90deg, ${cssVar(vars.color.accent)} ${pct}%, ${cssVar(vars.color.surfaceAlt)} ${pct}%)`,
	)

	// Click the number to type an exact value. Enter or blur commits
	// (clamped to the track range), Escape cancels.
	let editing = $state(false)
	let draft = $state('')

	const startEdit = () => {
		draft = String(value)
		editing = true
	}
	const commitEdit = () => {
		if (!editing) return
		editing = false
		const n = Number(draft)
		if (draft.trim() === '' || !Number.isFinite(n) || n === value) return
		;(oncommit ?? oninput)(Math.min(max, Math.max(min, Math.round(n))))
	}
	const cancelEdit = () => {
		editing = false
	}
	const autofocus = (el: HTMLInputElement) => {
		el.focus()
		el.select()
	}

</script>

<div class={sliderWrap}>
	<span class={sliderHead}>
		<label for={uid}>{label}</label>
		{#if editing}
			<input
				class={sliderValueInput}
				type="text"
				inputmode="numeric"
				bind:value={draft}
				{@attach autofocus}
				aria-label={`${label} exact value`}
				onkeydown={(e) => {
					if (e.key === 'Enter') commitEdit()
					else if (e.key === 'Escape') cancelEdit()
				}}
				onblur={commitEdit}
			/>
		{:else}
			<button type="button" class={sliderValueButton} onclick={startEdit} title="Click to type a value">
				{value}
			</button>
		{/if}
	</span>
	<input
		type="range"
		id={uid}
		class={slider}
		style:--slider-image={trackImage}
		{min}
		{max}
		{value}
		aria-label={label}
		oninput={(e) => oninput(e.currentTarget.valueAsNumber)}
		onchange={(e) => oncommit?.(e.currentTarget.valueAsNumber)}
	/>
</div>
