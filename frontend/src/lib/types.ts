// Wire types mirroring src/app.rs LampView and src/config.rs Preset (serde).

export const LAMP_FIELDS = ['on', 'brightness', 'temp', 'hue', 'sat', 'val'] as const

export type LampField = (typeof LAMP_FIELDS)[number]

export interface LampView {
	name: string
	connected: boolean
	on: boolean | null
	/** 0 white · 1 colour · 2 scene */
	mode: 0 | 1 | 2 | null
	/** 10–1000 */
	brightness: number | null
	/** 0–1000 device value; higher = yellower. 0 = coolest ≈6500 K,
	 * 1000 = warmest ≈2700 K (verified against the physical bulb). */
	temp: number | null
	/** 0–360 */
	hue: number | null
	/** 0–1000 */
	sat: number | null
	/** 0–1000 */
	val: number | null
	/** Device-side countdown timer, seconds remaining. */
	countdown: number | null
}

export interface Preset {
	mode: 'white' | 'colour'
	temp?: number
	brightness?: number
	hue?: number
	sat?: number
	val?: number
}
