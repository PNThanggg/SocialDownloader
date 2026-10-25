import { mount } from 'svelte'
import { invoke } from '@tauri-apps/api/core'
import './styles/glass.css'
import App from './App.svelte'
import { initLanguage } from './lib/i18n'

const app = mount(App, {
  target: document.getElementById('app')!,
})

// After mount, so a slow or failing backend call never blocks first paint.
async function applyWindowEffect() {
  let fx = 'none'
  try {
    fx = await invoke<string>('window_effect')
  } catch {
    // Not running inside Tauri (e.g. plain browser): use the fallback backdrop.
  }
  document.documentElement.classList.add(`fx-${fx}`)
}

void applyWindowEffect()
void initLanguage()

export default app
