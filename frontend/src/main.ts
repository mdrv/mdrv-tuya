import { mount } from 'svelte'
import App from './App.svelte'
import { applyTheme } from './lib/theme.svelte.ts'
import './styles.css.ts'

applyTheme()

mount(App, { target: document.getElementById('app')! })
