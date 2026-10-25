import { load, type Store } from '@tauri-apps/plugin-store'

export type Language = 'auto' | 'vi' | 'en'

const FILE = 'settings.json'
const KEY = 'language'

/** The app's settings store (language, lastDir). Throws outside Tauri. */
export const settingsStore = (): Promise<Store> => load(FILE, { defaults: {} })

export function isLanguage(v: unknown): v is Language {
  return v === 'auto' || v === 'vi' || v === 'en'
}

/** Stored language, or 'auto' when unset, invalid, or the store is unavailable (plain browser). */
export async function readLanguage(): Promise<Language> {
  try {
    const store = await settingsStore()
    const v = await store.get<unknown>(KEY)
    return isLanguage(v) ? v : 'auto'
  } catch {
    return 'auto'
  }
}

export async function writeLanguage(lang: Language): Promise<void> {
  try {
    const store = await settingsStore()
    await store.set(KEY, lang)
    await store.save()
  } catch {
    // Not persisted outside Tauri; the in-memory choice still applies.
  }
}
