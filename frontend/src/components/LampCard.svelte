<script lang="ts">
	import { HueRing } from './HueRing.svelte'
	import { PowerButton } from './PowerButton.svelte'
	import { PresetChips } from './PresetChips.svelte'
	import { Slider } from './Slider.svelte'
	import { StatusPill } from './StatusPill.svelte'
	import { commitWhite, dragColour, dragWhite, lampView } from '../lib/ws.svelte.ts'
	import {
		card,
		cardBody,
		cardHead,
		lampName,
		sliderGroup,
		tab,
		tabbar,
	} from '../styles.css.ts'

	let { name }: { name: string } = $props()

	// Cards only exist for names already delivered by hello/lamps, so the view
	// is always defined here.
	const lamp = $derived(lampView(name)!)

	// Follow the bulb's reported mode until the user picks a tab by hand —
	// dragging the hue ring flips the bulb to colour, temp/bright flips it to
	// white, and the mode in the snapshot echo makes the tab tag along.
	let picked = $state<'white' | 'colour' | null>(null)
	const shownTab = $derived(picked ?? (lamp.mode === 1 ? 'colour' : 'white'))

	// temp 0 (coolest) → 1000 (warmest); keep in sync with the track ends.
	const TEMP_GRADIENT = 'linear-gradient(90deg, #9ecfef 0%, #e8dcc8 55%, #ffc98a 100%)'
</script>

<section class={card} aria-label={`Lamp ${name}`}>
	<header class={cardHead}>
		<h2 class={lampName}>{name}</h2>
		<StatusPill {name} />
		<PowerButton {name} on={lamp.on === true} />
	</header>

	<div class={tabbar} role="tablist" aria-label={`${name} mode`}>
		<button
			type="button"
			role="tab"
			aria-selected={shownTab === 'white'}
			class={tab({ active: shownTab === 'white' })}
			onclick={() => (picked = 'white')}
		>
			White
		</button>
		<button
			type="button"
			role="tab"
			aria-selected={shownTab === 'colour'}
			class={tab({ active: shownTab === 'colour' })}
			onclick={() => (picked = 'colour')}
		>
			Colour
		</button>
	</div>

	<div class={cardBody({ dim: !lamp.connected })}>
		{#if shownTab === 'white'}
			<div class={sliderGroup}>
				<Slider
					label="Brightness"
					min={10}
					max={1000}
					value={lamp.brightness ?? 500}
					oninput={(v) => dragWhite(name, 'brightness', v)}
					oncommit={(v) => commitWhite(name, 'brightness', v)}
				/>
				<Slider
					label="Temperature"
					min={0}
					max={1000}
					value={lamp.temp ?? 500}
					variant="gradient"
					gradient={TEMP_GRADIENT}
					oninput={(v) => dragWhite(name, 'temp', v)}
					oncommit={(v) => commitWhite(name, 'temp', v)}
				/>
			</div>
		{:else}
			<HueRing
				{name}
				hue={lamp.hue ?? 30}
				sat={lamp.sat ?? 1000}
				val={lamp.val ?? 1000}
			/>
			<div class={sliderGroup}>
				<Slider
					label="Saturation"
					min={0}
					max={1000}
					value={lamp.sat ?? 1000}
					oninput={(v) => dragColour(name, lamp.hue ?? 30, v, lamp.val ?? 1000)}
					oncommit={(v) => commitColour(name, lamp.hue ?? 30, v, lamp.val ?? 1000)}
				/>
				<Slider
					label="Brightness"
					min={0}
					max={1000}
					value={lamp.val ?? 1000}
					oninput={(v) => dragColour(name, lamp.hue ?? 30, lamp.sat ?? 1000, v)}
					oncommit={(v) => commitColour(name, lamp.hue ?? 30, lamp.sat ?? 1000, v)}
				/>
			</div>
		{/if}
	</div>

	<PresetChips {name} />
</section>
