// Pure colour/space math shared by the ring and readouts. Plain TS (no
// runes) so bun test can exercise it directly.

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
