import { mount } from 'svelte'
import './styles/glass.css'
import App from './App.svelte'
import { windowEffect } from './lib/api'
import { initLanguage, syncDocumentLang } from './lib/i18n'

// Not awaited: a slow or failing backend call must never block first paint.
async function applyWindowEffect() {
  let fx = 'none'
  try {
    fx = await windowEffect()
  } catch {
    // Not running inside Tauri (e.g. plain browser): use the fallback backdrop.
  }
  document.documentElement.classList.add(`fx-${fx}`)
}

void applyWindowEffect()
// Awaited so the first frame is already in the chosen language (readLanguage never throws).
await initLanguage()
// Screen readers and spell-check follow the UI language, not index.html's default.
syncDocumentLang(document)

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
