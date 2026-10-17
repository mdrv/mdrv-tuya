<script lang="ts">
	import { swatchCss, tempKelvin } from '../lib/colour.ts'
	import type { Preset } from '../lib/types.ts'
	import { chip, chipSwatch, chipTip, chipWrap } from '../styles.css.ts'

	let {
		presetName,
		preset,
		onpick,
	}: {
		presetName: string
		preset: Preset
		onpick: (name: string) => void
	} = $props()

	// Preset ids are lower-kebab (sleep, warm-full); display capitalizes the
	// first letter only, leaving the rest verbatim.
	const label = $derived(presetName.charAt(0).toUpperCase() + presetName.slice(1))

	// Hover/focus readout of exactly what the lamp will get.
	const tip = $derived.by(() => {
		if (preset.mode === 'white') {
			const k = preset.temp !== undefined ? ` ≈${tempKelvin(preset.temp)} K` : ''
			const b = preset.brightness !== undefined ? ` · brightness ${preset.brightness}` : ''
			return `white${k}${b}`
		}
		if (preset.hue === undefined || preset.sat === undefined || preset.val === undefined) {
			return 'colour (unset)'
		}
		const b = preset.brightness !== undefined ? ` · brightness ${preset.brightness}` : ''
		return `colour ${preset.hue}° · ${Math.round(preset.sat / 10)}% · ${Math.round(preset.val / 10)}%${b}`
	})
</script>

<span class={chipWrap}>
	<button type="button" class={chip()} onclick={() => onpick(presetName)}>
		<span class={chipSwatch} style:background={swatchCss(preset)} aria-hidden="true"></span>
		{label}
	</button>
	<span class={chipTip} role="tooltip">{tip}</span>
</span>
