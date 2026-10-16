// Module-singleton WS store for the daemon socket. The daemon is
// authoritative; we keep a small optimistic overlay so slider drags render
// instantly instead of waiting on a BLE round-trip (the bulb dials in ~2–4 s
// on the first command).
//
// Scar: daemon snapshots are the only truth. A local edit is dropped the
// moment a snapshot echoes its value, or after ECHO_TTL_MS if the bulb
// applied something else (clamping, races) — never hold an overlay entry
// past its echo on the assumption the bulb "missed" the write.

import type { LampField, LampView, Preset } from './types.ts'
import { LAMP_FIELDS } from './types.ts'

export type WsStatus = 'connecting' | 'live' | 'reconnecting'
export type LampLink = 'live' | 'waking' | 'offline'

type ClientMsg =
	| { type: 'set'; lamp: string; dp: 'on'; value: boolean }
	| { type: 'set'; lamp: string; dp: 'bright' | 'temp'; value: number }
	| { type: 'set'; lamp: string; dp: 'colour'; value: { h: number; s: number; v: number } }
	| { type: 'preset'; lamp: string; name: string }
	| { type: 'refresh'; lamp: string }

type ServerMsg =
	| { type: 'hello'; lamps: Record<string, LampView>; presets: Record<string, Preset> }
	| { type: 'lamps'; lamps: Record<string, LampView> }
	| { type: 'error'; error: string }

// One BLE write per 150 ms idle window while a slider is being dragged.
const COALESCE_MS = 150
// How long a sent-but-unechoed edit stays optimistic before the daemon wins.
// Covers the 2–4 s BLE dial-in plus a write retry round.
const ECHO_TTL_MS = 8000
// How long the amber "connecting…" pill stays up after waking a bulb.
const WAKE_TTL_MS = 15000
const ERROR_TTL_MS = 6000

const state = $state({
	status: 'connecting' as WsStatus,
	lamps: {} as Record<string, LampView>,
	presets: {} as Record<string, Preset>,
	error: null as string | null,
})

// lamp → field → last local edit the daemon may not have echoed yet.
type OverlayEntry = { value: number | boolean; sent: boolean; at: number }
const overlay = $state<Record<string, Partial<Record<LampField, OverlayEntry>>>>({})

// lamp → when we queued the first command at a disconnected bulb; drives the
// amber pill while the BLE session dials in. Cleared by the connected echo.
const wake = $state<Record<string, number>>({})

export const wsStatus = (): WsStatus => state.status
export const lampNames = (): string[] => Object.keys(state.lamps).sort()
export const presets = (): Record<string, Preset> => state.presets
export const lastError = (): string | null => state.error

/** Server snapshot merged with any optimistic local edits for this lamp. */
export function lampView(name: string): LampView | undefined {
	const base = state.lamps[name]
	if (base === undefined) return undefined
	const ov = overlay[name]
	if (ov === undefined) return { ...base }
	const merged: LampView = { ...base }
	const writable = merged as Record<LampField, LampView[LampField]>
	for (const field of LAMP_FIELDS) {
		const entry = ov[field]
		if (entry !== undefined) writable[field] = entry.value
	}
	return merged
}

export function lampLink(name: string): LampLink {
	const lamp = state.lamps[name]
	if (lamp?.connected === true) return 'live'
	if (wake[name] !== undefined) return 'waking'
	return 'offline'
}

function markWake(lamp: string): void {
	if (state.lamps[lamp]?.connected === true) return
	wake[lamp] = Date.now()
	// Fall back to "offline" if the bulb never answers; a new command re-arms.
	setTimeout(() => {
		if (state.lamps[lamp]?.connected !== true) delete wake[lamp]
	}, WAKE_TTL_MS)
}

function setOverlay(lamp: string, field: LampField, value: number | boolean): OverlayEntry {
	const ov = overlay[lamp] ?? {}
	const entry: OverlayEntry = { value, sent: false, at: Date.now() }
	ov[field] = entry
	overlay[lamp] = ov
	return entry
}

const timers = new Map<string, ReturnType<typeof setTimeout>>()

function coalesce(lamp: string, dp: string, fire: () => void): void {
	const key = `${lamp}:${dp}`
	const prev = timers.get(key)
	if (prev !== undefined) clearTimeout(prev)
	timers.set(
		key,
		setTimeout(() => {
			timers.delete(key)
			fire()
		}, COALESCE_MS),
	)
}

function cancelCoalesce(lamp: string, dp: string): void {
	const key = `${lamp}:${dp}`
	const prev = timers.get(key)
	if (prev !== undefined) {
		clearTimeout(prev)
		timers.delete(key)
	}
}

export function setPower(lamp: string, on: boolean): void {
	const entry = setOverlay(lamp, 'on', on)
	entry.sent = true
	entry.at = Date.now()
	markWake(lamp)
	send({ type: 'set', lamp, dp: 'on', value: on })
}

export function dragWhite(lamp: string, field: 'brightness' | 'temp', value: number): void {
	setOverlay(lamp, field, value)
	markWake(lamp)
	const dp = field === 'brightness' ? 'bright' : 'temp'
	coalesce(lamp, dp, () => {
		const entry = overlay[lamp]?.[field]
		if (entry === undefined) return
		entry.sent = true
		entry.at = Date.now()
		send({ type: 'set', lamp, dp, value: entry.value as number })
	})
}

export function commitWhite(lamp: string, field: 'brightness' | 'temp', value: number): void {
	const dp = field === 'brightness' ? 'bright' : 'temp'
	cancelCoalesce(lamp, dp)
	const entry = setOverlay(lamp, field, value)
	entry.sent = true
	entry.at = Date.now()
	markWake(lamp)
	send({ type: 'set', lamp, dp, value })
}

export function dragColour(lamp: string, hue: number, sat: number, val: number): void {
	setOverlay(lamp, 'hue', hue)
	setOverlay(lamp, 'sat', sat)
	setOverlay(lamp, 'val', val)
	markWake(lamp)
	// Colour writes flip work_mode + push HSV: coalesce so a hue sweep is a
	// handful of frames, not a flood the retries have to clean up.
	coalesce(lamp, 'colour', () => sendCurrentColour(lamp))
}

export function commitColour(lamp: string, hue: number, sat: number, val: number): void {
	cancelCoalesce(lamp, 'colour')
	setOverlay(lamp, 'hue', hue)
	setOverlay(lamp, 'sat', sat)
	setOverlay(lamp, 'val', val)
	markWake(lamp)
	sendCurrentColour(lamp)
}

function sendCurrentColour(lamp: string): void {
	const ov = overlay[lamp]
	const h = ov?.hue
	const s = ov?.sat
	const v = ov?.val
	if (h === undefined || s === undefined || v === undefined) return
	for (const entry of [h, s, v]) {
		entry.sent = true
		entry.at = Date.now()
	}
	// Overlay shares one entry type with the boolean power DP; colour slots
	// only ever hold numbers.
	const num = (v: number | boolean): number => v as number
	send({
		type: 'set',
		lamp,
		dp: 'colour',
		value: { h: Math.round(num(h.value)), s: Math.round(num(s.value)), v: Math.round(num(v.value)) },
	})
}

export function sendPreset(lamp: string, name: string): void {
	// A preset rewrites every DP at once — drop local edits so the echoed
	// snapshot repaints cleanly instead of fighting the overlay.
	delete overlay[lamp]
	markWake(lamp)
	send({ type: 'preset', lamp, name })
}

/** Fold a full snapshot in, retiring overlay entries that echoed or expired. */
function absorb(lamps: Record<string, LampView>): void {
	state.lamps = lamps
	for (const [name, server] of Object.entries(lamps)) {
		if (server.connected) delete wake[name]
		const ov = overlay[name]
		if (ov === undefined) continue
		for (const field of LAMP_FIELDS) {
			const entry = ov[field]
			if (entry === undefined) continue
			const echo = server[field]
			const echoed = echo !== null && echo === entry.value
			const stale = entry.sent && Date.now() - entry.at > ECHO_TTL_MS
			if (echoed || stale) delete ov[field]
		}
		if (Object.keys(ov).length === 0) delete overlay[name]
	}
}

let ws: WebSocket | undefined
let stopped = true
let attempt = 0
let retryTimer: ReturnType<typeof setTimeout> | undefined
// Commands typed while the socket is down, replayed on open (bounded — a
// stale toggle from minutes ago is dropped rather than replayed).
const outbox: string[] = []

export function startWs(): void {
	stopped = false
	attempt = 0
	connect()
}

export function stopWs(): void {
	stopped = true
	if (retryTimer !== undefined) {
		clearTimeout(retryTimer)
		retryTimer = undefined
	}
	const socket = ws
	ws = undefined
	socket?.close()
	for (const timer of timers.values()) clearTimeout(timer)
	timers.clear()
}

function connect(): void {
	if (stopped) return
	state.status = 'connecting'
	const proto = location.protocol === 'https:' ? 'wss' : 'ws'
	const socket = new WebSocket(`${proto}://${location.host}/ws`)
	ws = socket
	socket.addEventListener('open', () => {
		if (socket !== ws) return
		attempt = 0
		state.status = 'live'
		while (outbox.length > 0) socket.send(outbox.shift()!)
	})
	socket.addEventListener('message', (e) => {
		if (socket !== ws) return
		handleMessage(typeof e.data === 'string' ? e.data : String(e.data))
	})
	socket.addEventListener('close', () => {
		if (socket !== ws) return
		ws = undefined
		// Exponential backoff 1s·2ⁿ capped at 10s; reset on a live open.
		if (stopped || retryTimer !== undefined) return
		state.status = 'reconnecting'
		retryTimer = setTimeout(() => {
			retryTimer = undefined
			connect()
		}, Math.min(1000 * 2 ** attempt++, 10_000))
	})
	socket.addEventListener('error', () => socket.close())
}

function send(msg: ClientMsg): void {
	const text = JSON.stringify(msg)
	if (ws?.readyState === WebSocket.OPEN) {
		ws.send(text)
	} else if (outbox.length < 20) {
		outbox.push(text)
	}
}

function handleMessage(text: string): void {
	let msg: ServerMsg
	try {
		msg = JSON.parse(text) as ServerMsg
	} catch {
		return
	}
	switch (msg.type) {
		case 'hello':
			state.presets = msg.presets
			absorb(msg.lamps)
			break
		case 'lamps':
			absorb(msg.lamps)
			break
		case 'error':
			state.error = msg.error
			setTimeout(() => {
				if (state.error === msg.error) state.error = null
			}, ERROR_TTL_MS)
			break
	}
}
