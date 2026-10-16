<script lang="ts">
	import LampCard from './components/LampCard.svelte'
	import { applyTheme, theme, toggleTheme } from './lib/theme.svelte.ts'
	import { lampNames, lastError, startWs, stopWs, wsStatus } from './lib/ws.svelte.ts'
	import { bar, barTitle, column, emptyNote, errorStrip, page, themeToggle, wsDot, wsTag } from './styles.css.ts'

	const status = $derived(wsStatus())
	const names = $derived(lampNames())
	const error = $derived(lastError())
	const statusLabel = $derived(
		status === 'live' ? 'Live' : status === 'connecting' ? 'Connecting' : 'Reconnecting',
	)

	// Root socket lifecycle: one connection for the whole app, torn down if
	// the tree ever unmounts (HMR included).
	$effect(() => {
		startWs()
		return () => stopWs()
	})
</script>

<div class={page}>
	<header class={bar}>
		<h1 class={barTitle}>mdrv-tuya</h1>
		<span class={wsTag}>
			<span class={wsDot({ status })} aria-hidden="true"></span>
			{statusLabel}
		</span>
		<button
			type="button"
			class={themeToggle}
			aria-label="Toggle theme"
			onclick={() => {
				toggleTheme()
				applyTheme()
			}}
		>
			{theme() === 'light' ? '🌙' : '☀️'}
		</button>
	</header>

	{#if error !== null}
		<div class={errorStrip} role="alert">{error}</div>
	{/if}

	<main class={column}>
		{#each names as name (name)}
			<LampCard {name} />
		{:else}
			<p class={emptyNote}>Waiting for daemon…</p>
		{/each}
	</main>
</div>
