<script lang="ts">
	import { dismissToast, toastList } from '../lib/ws.svelte.ts'
	import { toast, toastStack } from '../styles.css.ts'

	const list = $derived(toastList())
</script>

<div class={toastStack} role="status" aria-live="polite">
	{#each list as t (t.id)}
		<button type="button" class={toast({ kind: t.kind })} onclick={() => dismissToast(t.id)}>
			{#if t.lamp !== null}<strong>{t.lamp}</strong>{' '}{/if}{t.text}
		</button>
	{/each}
</div>
