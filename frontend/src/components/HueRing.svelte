<script lang="ts">
	import { hueAngle, hsvCss, pct } from '../lib/colour.ts'
	import { commitColour, dragColour } from '../lib/ws.svelte.ts'
	import { hueCenter, hueReadout, hueRing, hueSub, hueThumb } from '../styles.css.ts'

	interface Props {
		/** Lamp name — targets the store mutators. */
		name: string
		/** Merged (server+optimistic) HSV, already defaulted by the parent. */
		hue: number
		sat: number
		val: number
	}

	let { name, hue, sat, val }: Props = $props()

	let el: HTMLDivElement | undefined = $state()
	let dragging = $state(false)
	// Hue of the in-flight drag; falls back to the prop the rest of the time.
	let dragHue = $state<number | null>(null)
	const shownHue = $derived(dragHue ?? hue)

	const colour = $derived(hsvCss(shownHue, sat, val))
	// Thumb rides the ring midline (center disc is 12% inset → midline at 44%).
	const thumbPos = $derived.by(() => {
		const rad = (shownHue * Math.PI) / 180
		const r = 44
		return {
			left: `${50 + Math.sin(rad) * r}%`,
			top: `${50 - Math.cos(rad) * r}%`,
		}
	})

	function hueFromPointer(e: PointerEvent): number {
		const rect = el!.getBoundingClientRect()
		const dx = e.clientX - (rect.left + rect.width / 2)
		const dy = e.clientY - (rect.top + rect.height / 2)
		return hueAngle(dx, dy)
	}

	function apply(e: PointerEvent): void {
		const h = hueFromPointer(e)
		dragHue = h
		dragColour(name, h, sat, val)
	}

	function commit(h: number): void {
		commitColour(name, h, sat, val)
		// Let the snapshot echo own the value again.
		dragHue = null
	}
</script>

<div
	class={hueRing}
	bind:this={el}
	role="slider"
	tabindex="0"
	aria-label="Hue"
	aria-valuemin={0}
	aria-valuemax={360}
	aria-valuenow={shownHue}
	onpointerdown={(e) => {
		e.currentTarget.setPointerCapture(e.pointerId)
		dragging = true
		apply(e)
	}}
	onpointermove={(e) => {
		if (dragging) apply(e)
	}}
	onpointerup={() => {
		if (!dragging) return
		dragging = false
		commit(shownHue)
	}}
	onpointercancel={() => {
		dragging = false
		dragHue = null
	}}
	onkeydown={(e) => {
		const step = e.shiftKey ? 15 : 3
		let next: number | null = null
		if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') next = (shownHue - step + 360) % 360
		else if (e.key === 'ArrowRight' || e.key === 'ArrowUp') next = (shownHue + step) % 360
		if (next !== null) {
			e.preventDefault()
			dragHue = next
			commit(next)
		}
	}}
>
	<div class={hueThumb} style:left={thumbPos.left} style:top={thumbPos.top} style:background={colour}>
	</div>
	<div class={hueCenter} style:background={colour}>
		<span class={hueReadout}>{shownHue}°</span>
		<span class={hueSub}>{pct(sat)}% · {pct(val)}%</span>
	</div>
</div>
