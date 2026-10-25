import { mount } from 'svelte'
import { invoke } from '@tauri-apps/api/core'
import './styles/glass.css'
import App from './App.svelte'

async function applyWindowEffect() {
  let fx = 'none'
  try {
    fx = await invoke<string>('window_effect')
  } catch {
    // Not running inside Tauri (e.g. plain browser): use the fallback backdrop.
  }
  document.documentElement.classList.add(`fx-${fx}`)
}

await applyWindowEffect()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
