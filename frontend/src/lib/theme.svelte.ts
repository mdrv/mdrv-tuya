export type Theme = 'dark' | 'light'

const KEY = 'mdrv-tuya-theme'

// Dark is the product default; only an explicit stored choice overrides it
// (the OS preference is deliberately ignored — this is a lamp remote).
function readStored(): Theme {
	try {
		const saved = localStorage.getItem(KEY)
		if (saved === 'light' || saved === 'dark') return saved
	} catch {
		// private mode etc. — theme still applies for this page life
	}
	return 'dark'
}

let state = $state<Theme>(readStored())

export function theme(): Theme {
	return state
}

export function toggleTheme(): void {
	state = state === 'dark' ? 'light' : 'dark'
	applyTheme()
	try {
		localStorage.setItem(KEY, state)
	} catch {
		// ignore — theme still applies for this page life
	}
}

/** Mirrors the palette onto <html data-theme='light'>; dark is the unmarked default. */
export function applyTheme(): void {
	if (state === 'light') document.documentElement.dataset.theme = 'light'
	else delete document.documentElement.dataset.theme
}
