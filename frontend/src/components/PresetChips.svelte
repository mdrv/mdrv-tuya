<script lang="ts">
	import { presets, sendPreset } from '../lib/ws.svelte.ts'
	import PresetChip from './PresetChip.svelte'
	import { chipRow } from '../styles.css.ts'

	let { name }: { name: string } = $props()

	const list = $derived(presets())
</script>

{#if Object.keys(list).length > 0}
	<div class={chipRow}>
		{#each Object.entries(list) as [presetName, preset] (presetName)}
			<PresetChip {presetName} {preset} onpick={(n) => sendPreset(name, n)} />
		{/each}
	</div>
{/if}
