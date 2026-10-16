<script lang="ts">
	import { lampLink } from '../lib/ws.svelte.ts'
	import { pill, pillDot } from '../styles.css.ts'

	let { lamp }: { lamp: string } = $props()

	const status = $derived(lampLink(lamp))
	const label = $derived(status === 'live' ? 'Live' : status === 'waking' ? 'Connecting…' : 'Offline')
</script>

<span class={pill({ status })}>
	<span class={pillDot({ status })} aria-hidden="true"></span>
	{label}
</span>
