import { beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

const store = new Map<string, unknown>()
vi.mock('@tauri-apps/plugin-store', () => ({
  load: vi.fn(async () => ({
    get: async (k: string) => store.get(k),
    set: async (k: string, v: unknown) => void store.set(k, v),
    save: async () => {},
  })),
}))

import { detectLocale, initLanguage, language, locale, setLanguage, syncDocumentLang, t } from '../i18n'
import { en } from '../i18n/en'
import { vi as viMessages } from '../i18n/vi'

describe('messages', () => {
  it('vi and en have the same keys', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(viMessages).sort())
  })
  it('has no empty values', () => {
    for (const m of [en, viMessages]) for (const [k, v] of Object.entries(m)) expect(v, k).not.toBe('')
  })
})

describe('syncDocumentLang', () => {
  it('keeps <html lang> on the active locale', () => {
    language.set('vi')
    const off = syncDocumentLang(document)
    expect(document.documentElement.lang).toBe('vi')
    language.set('en')
    expect(document.documentElement.lang).toBe('en')
    off()
    language.set('vi')
    expect(document.documentElement.lang).toBe('en')
  })
})

describe('detectLocale', () => {
  it('follows the Windows language', () => {
    expect(detectLocale('vi-VN')).toBe('vi')
    expect(detectLocale('vi')).toBe('vi')
    expect(detectLocale('en-US')).toBe('en')
    expect(detectLocale('fr')).toBe('en')
  })
})

describe('t', () => {
  beforeEach(() => {
    store.clear()
    language.set('en')
  })

  it('interpolates site display names', () => {
    expect(get(t)('err.login_required', { site: 'instagram' })).toBe('This post requires signing in to Instagram.')
    expect(get(t)('btn.connect', { site: 'tiktok' })).toBe('Connect TikTok')
    expect(get(t)('btn.reconnect', { site: 'youtube' })).toBe('Reconnect YouTube')
    expect(get(t)('btn.connect', { site: 'facebook' })).toBe('Connect Facebook')
  })

  it('interpolates tool display names', () => {
    expect(get(t)('err.tool_missing', { tool: 'yt_dlp' })).toBe('The yt-dlp tool is missing.')
    expect(get(t)('err.tool_missing', { tool: 'gallery_dl' })).toBe('The gallery-dl tool is missing.')
  })

  it('interpolates other params verbatim', () => {
    expect(get(t)('note.sourceAudio', { ext: 'm4a' })).toBe('Copies the source audio stream (m4a)')
  })

  it('switches with the language', () => {
    language.set('vi')
    expect(get(locale)).toBe('vi')
    expect(get(t)('hero.title')).toBe('Dán link vào đây')
    language.set('en')
    expect(get(t)('hero.title')).toBe('Paste a link here')
  })

  it('auto follows navigator.language', () => {
    language.set('auto')
    expect(get(locale)).toBe(detectLocale(navigator.language))
  })

  it('persists the language and restores it', async () => {
    await setLanguage('vi')
    expect(store.get('language')).toBe('vi')
    language.set('en')
    await initLanguage()
    expect(get(language)).toBe('vi')
  })

  it('ignores a garbage stored language', async () => {
    store.set('language', 'klingon')
    language.set('en')
    await initLanguage()
    expect(get(language)).toBe('auto')
  })
})
