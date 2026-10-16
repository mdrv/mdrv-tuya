import { svelte } from '@sveltejs/vite-plugin-svelte'
import { vanillaExtractPlugin } from '@vanilla-extract/vite-plugin'
import { defineConfig } from 'vite'
import { viteSingleFile } from 'vite-plugin-singlefile'

// The daemon embeds dist/index.html verbatim (include_str!), so the build
// MUST be one self-contained file with zero external asset references —
// viteSingleFile inlines every JS/CSS chunk into the HTML.
export default defineConfig({
	plugins: [vanillaExtractPlugin(), svelte(), viteSingleFile()],
	server: {
		port: 5276,
		proxy: {
			'/ws': {
				target: 'http://localhost:4141',
				ws: true,
			},
		},
	},
	build: {
		outDir: 'dist',
		emptyOutDir: true,
	},
})
