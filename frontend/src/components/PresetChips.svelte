<script lang="ts">
	import { presets, sendPreset } from '../lib/ws.svelte.ts'
	import { chip, chipRow } from '../styles.css.ts'

	let { lamp }: { lamp: string } = $props()

	const names = $derived(Object.keys(presets()))

	// Preset ids are lower-kebab (sleep, warm-full); display capitalizes the
	// first letter only, leaving the rest verbatim.
	function label(name: string): string {
		return name.charAt(0).toUpperCase() + name.slice(1)
	}
</script>

{#if names.length > 0}
	<div class={chipRow}>
		{#each names as name (name)}
			<button type="button" class={chip()} onclick={() => sendPreset(lamp, name)}>
				{label(name)}
			</button>
		{/each}
	</div>
{/if}
