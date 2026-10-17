<script lang="ts">
	import GlobalBar from './components/GlobalBar.svelte'
	import LampCard from './components/LampCard.svelte'
	import { applyTheme, theme, toggleTheme } from './lib/theme.svelte.ts'
	import { lampNames, lastError, startWs, stopWs, wsStatus } from './lib/ws.svelte.ts'
	import {
		bar,
		barTitle,
		chip,
		column,
		emptyNote,
		errorStrip,
		page,
		selectorRow,
		themeToggle,
		wsDot,
		wsTag,
	} from './styles.css.ts'

	const status = $derived(wsStatus())
	const names = $derived(lampNames())
	const error = $derived(lastError())
	const statusLabel = $derived(
		status === 'live' ? 'Live' : status === 'connecting' ? 'Connecting' : 'Reconnecting',
	)

	// Lamp selection: 'all' or a single lamp name, persisted across loads.
	const SELECT_KEY = 'mdrv-tuya.lamp'
	let selected = $state(loadSelection())

	function loadSelection(): string {
		try {
			return localStorage.getItem(SELECT_KEY) ?? 'all'
		} catch {
			return 'all'
		}
	}

	function select(value: string): void {
		selected = value
		try {
			localStorage.setItem(SELECT_KEY, value)
		} catch {
			// Private mode etc. — selection just won't persist.
		}
	}

	// A selected lamp that vanishes from the daemon (config change) falls
	// back to the all view instead of a blank page.
	$effect(() => {
		if (selected !== 'all' && names.length > 0 && !names.includes(selected)) {
			select('all')
		}
	})

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

	{#if names.length > 1}
		<nav class={selectorRow} aria-label="Lamp selection">
			<button
				type="button"
				class={chip({ on: selected === 'all' })}
				aria-pressed={selected === 'all'}
				onclick={() => select('all')}
			>
				All
			</button>
			{#each names as name (name)}
				<button
					type="button"
					class={chip({ on: selected === name })}
					aria-pressed={selected === name}
					onclick={() => select(name)}
				>
					{name}
				</button>
			{/each}
		</nav>
	{/if}

	<main class={column}>
		{#if selected !== 'all' && names.includes(selected)}
			<LampCard name={selected} />
		{:else}
			{#if names.length > 0}
				<GlobalBar />
			{/if}
			{#each names as name (name)}
				<LampCard {name} />
			{:else}
				<p class={emptyNote}>Waiting for daemon…</p>
			{/each}
		{/if}
	</main>
</div>
