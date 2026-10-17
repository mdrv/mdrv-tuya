import { describe, expect, test } from 'bun:test'
import { hsvCss, hueAngle, pct, swatchCss, tempKelvin } from './colour.ts'

describe('hsvCss', () => {
	test('sleep preset values', () => {
		// 899‰ → 90%, 104‰ → 5% (the dim end is scaled to 46% max lightness)
		expect(hsvCss(39, 899, 104)).toBe('hsl(39 90% 5%)')
	})
	test('full saturation and value', () => {
		expect(hsvCss(180, 1000, 1000)).toBe('hsl(180 100% 46%)')
	})
	test('zero saturation and value', () => {
		expect(hsvCss(0, 0, 0)).toBe('hsl(0 0% 0%)')
	})
})

describe('hueAngle', () => {
	// 0° at 12 o'clock, clockwise (screen coords: y grows downward)
	test('top is 0°', () => {
		expect(hueAngle(0, -1)).toBe(0)
	})
	test('right is 90°', () => {
		expect(hueAngle(1, 0)).toBe(90)
	})
	test('bottom is 180°', () => {
		expect(hueAngle(0, 1)).toBe(180)
	})
	test('left is 270°', () => {
		expect(hueAngle(-1, 0)).toBe(270)
	})
	test('wraps below 0 into range, not to 360', () => {
		// atan2(-1, 1000) ≈ -0.06° — one step left of 12 o'clock. Rounding
		// must land on 0 (not 360), and a slightly wider arc stays at 359.
		expect(hueAngle(-1, -1000)).toBe(0)
		expect(hueAngle(-10, -1000)).toBe(359)
	})
})

describe('pct', () => {
	test('rounds per-mille to percent', () => {
		expect(pct(899)).toBe(90)
		expect(pct(104)).toBe(10)
		expect(pct(0)).toBe(0)
		expect(pct(1000)).toBe(100)
	})
})

describe('tempKelvin', () => {
	test('track ends map to the bead range (blue left, yellow right)', () => {
		expect(tempKelvin(0)).toBe(6500)
		expect(tempKelvin(1000)).toBe(2700)
	})
	test('midpoint', () => {
		expect(tempKelvin(500)).toBe(4600)
	})
})

describe('swatchCss', () => {
	test('white preset samples the temp gradient at its position', () => {
		// warmest (dp 1000) sits at the right/yellow end of the track
		const css = swatchCss({ mode: 'white', temp: 1000, brightness: 1000 })
		expect(css).toContain('linear-gradient')
		expect(css).toContain('100% 0% / 300% 100%')
	})
	test('coolest white preset lands on the left', () => {
		const css = swatchCss({ mode: 'white', temp: 0 })
		expect(css).toContain('0% 0% / 300% 100%')
	})
	test('white preset without temp defaults to the middle', () => {
		const css = swatchCss({ mode: 'white' })
		expect(css).toContain('50% 0% / 300% 100%')
	})
	test('colour preset keys lightness to val so dim reads dim', () => {
		// sleep: sat 899 → 90%, val 104 → lightness 15 + round(3.64) = 19%
		expect(swatchCss({ mode: 'colour', hue: 39, sat: 899, val: 104 })).toBe('hsl(39 90% 19%)')
	})
	test('full colour preset lands at the bright end', () => {
		expect(swatchCss({ mode: 'colour', hue: 180, sat: 1000, val: 1000 })).toBe('hsl(180 100% 50%)')
	})
	test('colour preset without hsv is neutral', () => {
		expect(swatchCss({ mode: 'colour' })).toBe('transparent')
	})
})
