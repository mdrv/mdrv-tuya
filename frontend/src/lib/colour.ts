// Pure colour/space math shared by the ring and readouts. Plain TS (no
// runes) so bun test can exercise it directly.

import type { Preset } from './types.ts'

/** Bulb HSV (sat/val in per-mille) → the ring's centre swatch CSS colour. */
export function hsvCss(hue: number, sat: number, val: number): string {
	return `hsl(${hue} ${Math.round((sat / 1000) * 100)}% ${Math.round((val / 1000) * 46)}%)`
}

/**
 * Pointer offset from the ring's centre → hue in degrees.
 * 0° at 12 o'clock, clockwise — matches the conic-gradient origin.
 */
export function hueAngle(dx: number, dy: number): number {
	const deg = (Math.atan2(dx, -dy) * 180) / Math.PI
	// The second % 360 keeps rounding from nudging 359.6° up to 360°.
	return Math.round((deg + 360) % 360) % 360
}

/** Per-mille → percent, for readouts. */
export function pct(perMille: number): number {
	return Math.round(perMille / 10)
}

/**
 * White-temp axis. The bulb's datapoint runs 0 = coolest (≈6500 K) to
 * 1000 = warmest (≈2700 K), but the UI shows the Tuya-app orientation:
 * warm on the left, cool/blue on the right. These two helpers convert
 * between the device value (dp) and the slider value (ui).
 */
export const tempUi = (dp: number): number => 1000 - dp
export const tempDp = (ui: number): number => 1000 - ui

/** White-temp track in UI orientation: 0 (warm) → 1000 (cool). Shared by
 * the temp slider and the preset swatches so both ends agree. */
export const TEMP_GRADIENT = 'linear-gradient(90deg, #ffc98a 0%, #e8dcc8 45%, #9ecfef 100%)'

/** Slider temp value → approximate Kelvin (0 ≈ 2700 K warm, 1000 ≈ 6500 K cool). */
export function tempKelvin(ui: number): number {
	return Math.round(2700 + (ui / 1000) * 3800)
}

/**
 * Background CSS for a preset's swatch dot: white presets sample the temp
 * gradient at their position, colour presets show their hue with a
 * val-keyed lightness so a dim preset reads dim.
 */
export function swatchCss(preset: Preset): string {
	if (preset.mode === 'white') {
		// Sample the flipped gradient at the preset's UI-axis position.
		const pos = Math.round((tempUi(preset.temp ?? 500) / 1000) * 100)
		return `${TEMP_GRADIENT} ${pos}% 0% / 300% 100% no-repeat`
	}
	if (preset.hue === undefined || preset.sat === undefined || preset.val === undefined) {
		return 'transparent'
	}
	const sat = Math.round((preset.sat / 1000) * 100)
	const light = 15 + Math.round((preset.val / 1000) * 35)
	return `hsl(${preset.hue} ${sat}% ${light}%)`
}
