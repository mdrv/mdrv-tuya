// Plain (non-vanilla-extract) module: .css.ts files may only export
// serializable style values, so this helper lives apart from styles.css.ts.

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
