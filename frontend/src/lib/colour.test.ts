import { describe, expect, test } from 'bun:test'
import { hsvCss, hueAngle, pct } from './colour.ts'

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
