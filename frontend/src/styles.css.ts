// All styling lives here (vanilla-extract); .svelte files carry zero <style>
// blocks. Dark is the default palette on :root; <html data-theme="light">
// swaps the same contract to the light values.

import { createGlobalTheme, createGlobalThemeContract, globalStyle, keyframes, style } from '@vanilla-extract/css'
import { recipe } from '@vanilla-extract/recipes'

export const vars = createGlobalThemeContract({
	color: {
		bg: null,
		surface: null,
		surfaceAlt: null,
		border: null,
		text: null,
		textMuted: null,
		accent: null,
		accentSoft: null,
		live: null,
		warn: null,
		dead: null,
		error: null,
		errorSoft: null,
	},
	font: {
		body: null,
		mono: null,
	},
})

const fonts = {
	body: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
	mono: "ui-monospace, 'SF Mono', 'Cascadia Code', Menlo, Consolas, monospace",
}

createGlobalTheme(':root', vars, {
	color: {
		bg: '#0a0a0f',
		surface: '#15151d',
		surfaceAlt: '#23232e',
		border: '#272736',
		text: '#f2f3f7',
		textMuted: '#8f93a8',
		accent: '#4da3ff',
		accentSoft: 'rgba(77, 163, 255, 0.16)',
		live: '#3ddc84',
		warn: '#ffb648',
		dead: '#5c6070',
		error: '#ff8b8b',
		errorSoft: 'rgba(255, 110, 110, 0.12)',
	},
	font: fonts,
})

createGlobalTheme('[data-theme="light"]', vars, {
	color: {
		bg: '#eef0f5',
		surface: '#ffffff',
		surfaceAlt: '#e2e6ef',
		border: '#d8dde9',
		text: '#191b22',
		textMuted: '#666c80',
		accent: '#2b7de0',
		accentSoft: 'rgba(43, 125, 224, 0.13)',
		live: '#1fa855',
		warn: '#b26a00',
		dead: '#9298a8',
		error: '#bb2d2d',
		errorSoft: 'rgba(187, 45, 45, 0.09)',
	},
	font: fonts,
})

globalStyle('*', {
	boxSizing: 'border-box',
})

globalStyle(':root', {
	colorScheme: 'dark',
})

globalStyle('[data-theme="light"]', {
	colorScheme: 'light',
})

globalStyle('html', {
	WebkitTextSizeAdjust: '100%',
})

globalStyle('body', {
	margin: 0,
	minHeight: '100dvh',
	background: vars.color.bg,
	color: vars.color.text,
	fontFamily: vars.font.body,
	lineHeight: 1.45,
	WebkitTapHighlightColor: 'transparent',
	WebkitFontSmoothing: 'antialiased',
})

globalStyle('button', {
	fontFamily: 'inherit',
	fontSize: 'inherit',
})

/**
 * Inline styles in .svelte can't go through the vanilla-extract compiler, so
 * dynamic rules (slider track gradients) need raw `var()` references. Contract
 * values may present as '--name', 'name' or 'var(--name)' depending on the
 * compiler output — normalize all three.
 */
export const cssVar = (name: string): string => {
	if (name.startsWith('var(')) return name
	return `var(--${name.startsWith('--') ? name.slice(2) : name})`
}

const pulse = keyframes({
	'0%, 100%': { opacity: 1 },
	'50%': { opacity: 0.35 },
})

export const page = style({
	minHeight: '100dvh',
	maxWidth: '28rem',
	margin: '0 auto',
	padding: 'calc(env(safe-area-inset-top, 0px) + 12px) 16px calc(env(safe-area-inset-bottom, 0px) + 24px)',
	display: 'flex',
	flexDirection: 'column',
	gap: '14px',
})

export const bar = style({
	display: 'flex',
	alignItems: 'center',
	gap: '10px',
})

export const barTitle = style({
	margin: 0,
	flex: 1,
	fontSize: '1.05rem',
	fontWeight: 700,
	letterSpacing: '-0.01em',
	fontVariantNumeric: 'tabular-nums',
})

export const wsTag = style({
	display: 'inline-flex',
	alignItems: 'center',
	gap: '6px',
	fontSize: '0.78rem',
	color: vars.color.textMuted,
	whiteSpace: 'nowrap',
})

export const wsDot = recipe({
	base: {
		width: '8px',
		height: '8px',
		borderRadius: '50%',
		flexShrink: 0,
	},
	variants: {
		status: {
			live: { background: vars.color.live },
			connecting: { background: vars.color.warn, animation: `${pulse} 1.2s ease-in-out infinite` },
			reconnecting: { background: vars.color.warn, animation: `${pulse} 1.2s ease-in-out infinite` },
		},
	},
})

export const themeToggle = style({
	width: '36px',
	height: '36px',
	borderRadius: '50%',
	border: `1px solid ${vars.color.border}`,
	background: vars.color.surface,
	color: vars.color.text,
	fontSize: '1rem',
	lineHeight: 1,
	display: 'grid',
	placeItems: 'center',
	cursor: 'pointer',
	selectors: {
		'&:active': { transform: 'scale(0.93)' },
	},
})

export const errorStrip = style({
	padding: '8px 12px',
	borderRadius: '12px',
	border: `1px solid ${vars.color.error}`,
	background: vars.color.errorSoft,
	color: vars.color.error,
	fontSize: '0.8rem',
	fontFamily: vars.font.mono,
	wordBreak: 'break-word',
})

export const emptyNote = style({
	margin: 'auto',
	color: vars.color.textMuted,
	fontSize: '0.9rem',
})

export const column = style({
	display: 'flex',
	flexDirection: 'column',
	gap: '14px',
})

export const card = style({
	background: vars.color.surface,
	border: `1px solid ${vars.color.border}`,
	borderRadius: '18px',
	padding: '16px',
	display: 'flex',
	flexDirection: 'column',
	gap: '14px',
})

export const cardHead = style({
	display: 'flex',
	alignItems: 'center',
	gap: '10px',
})

export const lampName = style({
	margin: 0,
	flex: 1,
	minWidth: 0,
	fontSize: '1.02rem',
	fontWeight: 650,
	overflow: 'hidden',
	textOverflow: 'ellipsis',
	whiteSpace: 'nowrap',
})

export const cardBody = recipe({
	base: {
		display: 'flex',
		flexDirection: 'column',
		gap: '12px',
	},
	variants: {
		// Disconnected bulbs keep every control usable (a tap wakes the BLE
		// session); the dimming just signals "not live right now".
		dim: {
			true: { opacity: 0.55 },
			false: { opacity: 1 },
		},
	},
})

export const tabbar = style({
	display: 'flex',
	gap: '4px',
	padding: '4px',
	borderRadius: '13px',
	background: vars.color.bg,
	border: `1px solid ${vars.color.border}`,
})

export const tab = recipe({
	base: {
		flex: 1,
		padding: '8px 0',
		borderRadius: '10px',
		border: 'none',
		background: 'transparent',
		color: vars.color.textMuted,
		fontSize: '0.85rem',
		fontWeight: 600,
		cursor: 'pointer',
		transition: 'background 0.15s ease, color 0.15s ease',
	},
	variants: {
		active: {
			true: { background: vars.color.accentSoft, color: vars.color.accent },
			false: {},
		},
	},
})

export const sliderGroup = style({
	display: 'flex',
	flexDirection: 'column',
	gap: '10px',
})

export const sliderWrap = style({
	display: 'flex',
	flexDirection: 'column',
	gap: '2px',
})

export const sliderHead = style({
	display: 'flex',
	justifyContent: 'space-between',
	alignItems: 'baseline',
	gap: '8px',
	fontSize: '0.8rem',
	fontWeight: 550,
	color: vars.color.textMuted,
})

export const sliderValue = style({
	fontFamily: vars.font.mono,
	fontVariantNumeric: 'tabular-nums',
	fontSize: '0.82rem',
	color: vars.color.text,
})

export const slider = style({
	appearance: 'none',
	WebkitAppearance: 'none',
	width: '100%',
	height: '26px',
	margin: 0,
	background: 'transparent',
	cursor: 'pointer',
	touchAction: 'pan-y',
	selectors: {
		'&::-webkit-slider-runnable-track': {
			height: '12px',
			borderRadius: '999px',
			backgroundImage: 'var(--slider-image)',
		},
		'&::-webkit-slider-thumb': {
			WebkitAppearance: 'none',
			appearance: 'none',
			width: '22px',
			height: '22px',
			marginTop: '-5px',
			borderRadius: '50%',
			background: '#fff',
			border: 'none',
			boxShadow: '0 1px 4px rgba(0, 0, 0, 0.45)',
		},
		'&::-moz-range-track': {
			height: '12px',
			borderRadius: '999px',
			backgroundImage: 'var(--slider-image)',
		},
		'&::-moz-range-thumb': {
			width: '22px',
			height: '22px',
			borderRadius: '50%',
			background: '#fff',
			border: 'none',
			boxShadow: '0 1px 4px rgba(0, 0, 0, 0.45)',
		},
		'&:focus-visible': {
			outline: `2px solid ${vars.color.accent}`,
			outlineOffset: '2px',
		},
	},
})

export const chipRow = style({
	display: 'flex',
	flexWrap: 'wrap',
	gap: '8px',
})

export const chip = recipe({
	base: {
		padding: '7px 14px',
		borderRadius: '999px',
		border: `1px solid ${vars.color.border}`,
		background: vars.color.surfaceAlt,
		color: vars.color.text,
		fontSize: '0.82rem',
		fontWeight: 550,
		cursor: 'pointer',
		transition: 'border-color 0.15s ease, background 0.15s ease',
		selectors: {
			'&:active': {
				background: vars.color.accentSoft,
				borderColor: vars.color.accent,
			},
			'&:focus-visible': {
				outline: `2px solid ${vars.color.accent}`,
				outlineOffset: '2px',
			},
		},
	},
})

// Static hue wheel: conic-gradient starts at 12 o'clock and runs clockwise,
// which is exactly the angle convention hueFromPointer uses.
export const hueRing = style({
	position: 'relative',
	width: 'min(100%, 264px)',
	aspectRatio: '1',
	margin: '0 auto',
	borderRadius: '50%',
	background:
		'conic-gradient(from 0deg, hsl(0 100% 50%), hsl(60 100% 50%), hsl(120 100% 50%), hsl(180 100% 50%), hsl(240 100% 50%), hsl(300 100% 50%), hsl(360 100% 50%))',
	boxShadow: 'inset 0 0 0 1px rgba(255, 255, 255, 0.08)',
	touchAction: 'none',
	cursor: 'pointer',
	selectors: {
		'&:focus-visible': {
			outline: `2px solid ${vars.color.accent}`,
			outlineOffset: '3px',
		},
	},
})

export const hueCenter = style({
	position: 'absolute',
	inset: '12%',
	borderRadius: '50%',
	display: 'flex',
	flexDirection: 'column',
	alignItems: 'center',
	justifyContent: 'center',
	gap: '2px',
	color: '#fff',
	textShadow: '0 1px 6px rgba(0, 0, 0, 0.55)',
	pointerEvents: 'none',
})

export const hueReadout = style({
	fontFamily: vars.font.mono,
	fontVariantNumeric: 'tabular-nums',
	fontSize: '1.9rem',
	fontWeight: 650,
	lineHeight: 1,
})

export const hueSub = style({
	fontFamily: vars.font.mono,
	fontVariantNumeric: 'tabular-nums',
	fontSize: '0.78rem',
	opacity: 0.92,
})

export const hueThumb = style({
	position: 'absolute',
	width: '24px',
	height: '24px',
	borderRadius: '50%',
	transform: 'translate(-50%, -50%)',
	border: '3px solid rgba(255, 255, 255, 0.95)',
	boxShadow: '0 1px 6px rgba(0, 0, 0, 0.5)',
	pointerEvents: 'none',
})

export const powerButton = recipe({
	base: {
		width: '46px',
		height: '46px',
		flexShrink: 0,
		borderRadius: '50%',
		display: 'grid',
		placeItems: 'center',
		border: `1.5px solid ${vars.color.border}`,
		background: vars.color.surfaceAlt,
		color: vars.color.textMuted,
		cursor: 'pointer',
		transition: 'background 0.15s ease, color 0.15s ease, border-color 0.15s ease',
		selectors: {
			'&:active': { transform: 'scale(0.94)' },
			'&:focus-visible': {
				outline: `2px solid ${vars.color.accent}`,
				outlineOffset: '2px',
			},
		},
	},
	variants: {
		on: {
			true: {
				background: vars.color.accent,
				borderColor: vars.color.accent,
				color: '#fff',
			},
			false: {},
		},
	},
})

export const powerIcon = style({
	display: 'block',
})

export const pill = recipe({
	base: {
		display: 'inline-flex',
		alignItems: 'center',
		gap: '6px',
		padding: '3px 10px 3px 8px',
		borderRadius: '999px',
		border: `1px solid ${vars.color.border}`,
		background: vars.color.surfaceAlt,
		color: vars.color.textMuted,
		fontSize: '0.72rem',
		fontWeight: 550,
		letterSpacing: '0.02em',
		whiteSpace: 'nowrap',
		flexShrink: 0,
	},
	variants: {
		status: {
			live: { color: vars.color.text },
			waking: {},
			offline: {},
		},
	},
})

export const pillDot = recipe({
	base: {
		width: '8px',
		height: '8px',
		borderRadius: '50%',
		flexShrink: 0,
	},
	variants: {
		status: {
			live: { background: vars.color.live },
			waking: { background: vars.color.warn, animation: `${pulse} 1.2s ease-in-out infinite` },
			offline: { background: vars.color.dead },
		},
	},
})
