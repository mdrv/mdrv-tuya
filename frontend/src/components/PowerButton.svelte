<script lang="ts">
	import { setPower } from '../lib/ws.svelte.ts'
	import { powerButton, powerIcon } from '../styles.css.ts'

	let { name, on }: { name: string; on: boolean } = $props()
</script>

<button
	type="button"
	class={powerButton({ on })}
	aria-pressed={on}
	aria-label={on ? 'Turn off' : 'Turn on'}
	onclick={() => setPower(name, !on)}
>
	<svg
		class={powerIcon}
		viewBox="0 0 24 24"
		width="20"
		height="20"
		fill="none"
		stroke="currentColor"
		stroke-width="2.4"
		stroke-linecap="round"
		aria-hidden="true"
	>
		<path d="M12 2.5v8.5" />
		<path d="M17.66 5.34a8 8 0 1 1-11.32 0" />
	</svg>
</button>
