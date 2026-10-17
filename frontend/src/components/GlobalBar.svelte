<script lang="ts">
	import { lampNames, presets, sendPreset, setPower } from '../lib/ws.svelte.ts'
	import { allBar, chip, chipRow, groupLabel } from '../styles.css.ts'

	const names = $derived(lampNames())
	const presetNames = $derived(Object.keys(presets()))

	// Preset ids are lower-kebab (sleep, warm-full); display capitalizes the
	// first letter only, leaving the rest verbatim.
	function label(name: string): string {
		return name.charAt(0).toUpperCase() + name.slice(1)
	}

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
	{#if presetNames.length > 0}
		<div>
			<p class={groupLabel}>Presets — every lamp</p>
			<div class={chipRow}>
				{#each presetNames as presetName (presetName)}
					<button type="button" class={chip()} onclick={() => presetAll(presetName)}>
						{label(presetName)}
					</button>
				{/each}
			</div>
		</div>
	{/if}
</section>
