<script lang="ts">
	import { lampNames, presets, sendPreset, setPower } from '../lib/ws.svelte.ts'
	import PresetChip from './PresetChip.svelte'
	import { allBar, chip, chipRow, groupLabel } from '../styles.css.ts'

	const names = $derived(lampNames())
	const presetList = $derived(presets())

	function powerAll(on: boolean): void {
		for (const name of names) setPower(name, on)
	}

	function presetAll(preset: string): void {
		for (const name of names) sendPreset(name, preset)
	}
</script>

<section class={allBar} aria-label="All lamps">
	<div>
		<p class={groupLabel}>Power</p>
		<div class={chipRow}>
			<button type="button" class={chip()} onclick={() => powerAll(true)}>All on</button>
			<button type="button" class={chip()} onclick={() => powerAll(false)}>All off</button>
		</div>
	</div>
	{#if Object.keys(presetList).length > 0}
		<div>
			<p class={groupLabel}>Presets — every lamp</p>
			<div class={chipRow}>
				{#each Object.entries(presetList) as [presetName, preset] (presetName)}
					<PresetChip {presetName} {preset} onpick={presetAll} />
				{/each}
			</div>
		</div>
	{/if}
</section>
