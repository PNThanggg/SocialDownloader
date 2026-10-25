import { derived, writable, type Readable, type Writable } from 'svelte/store'
import type { ErrorCode, Site, Tool } from '../types'
import { en } from './en'
import { readLanguage, writeLanguage, type Language } from './settings'
import { vi, type MessageKey } from './vi'

export type { MessageKey } from './vi'
export type { Language } from './settings'
export type Locale = 'vi' | 'en'

const messages: Record<Locale, Record<MessageKey, string>> = { vi, en }

const SITE_NAMES: Record<Site, string> = {
  tiktok: 'TikTok',
  instagram: 'Instagram',
  youtube: 'YouTube',
  facebook: 'Facebook',
  other: 'Other',
}

const TOOL_NAMES: Record<Tool, string> = {
  yt_dlp: 'yt-dlp',
  gallery_dl: 'gallery-dl',
  ffmpeg: 'ffmpeg',
  deno: 'deno',
}

/** Display name of a tool (`yt-dlp`), the same in every language. */
export const toolName = (tool: Tool): string => TOOL_NAMES[tool]

export function detectLocale(nav: string): Locale {
  return nav.toLowerCase().startsWith('vi') ? 'vi' : 'en'
}

/** The user's choice; 'auto' follows the system language. */
export const language: Writable<Language> = writable('auto')

export const locale: Readable<Locale> = derived(language, ($l) =>
  $l === 'auto' ? detectLocale(typeof navigator === 'undefined' ? 'en' : navigator.language) : $l,
)

/** Keeps `<html lang>` on the active locale (screen readers, spell-check); returns the unsubscribe. */
export function syncDocumentLang(doc: Document): () => void {
  return locale.subscribe((l) => (doc.documentElement.lang = l))
}

export async function initLanguage(): Promise<void> {
  language.set(await readLanguage())
}

export async function setLanguage(lang: Language): Promise<void> {
  language.set(lang)
  await writeLanguage(lang)
}

function display(name: string, value: string): string {
  if (name === 'site') return SITE_NAMES[value as Site] ?? value
  if (name === 'tool') return TOOL_NAMES[value as Tool] ?? value
  return value
}

export type Translate = (key: MessageKey, params?: Record<string, string>) => string

export const t: Readable<Translate> = derived(locale, ($locale): Translate => (key, params) =>
  messages[$locale][key].replace(/\{(\w+)\}/g, (m: string, name: string) =>
    params && name in params ? display(name, params[name]) : m,
  ),
)

export type ErrorAction = 'connect' | 'reconnect' | 'update' | 'retry' | 'details' | 'reinstall'

/** Sites with an in-app login (Settings → Accounts); only they get a Connect button. */
const LOGIN_SITES: readonly Site[] = ['instagram', 'facebook']

/** `during`: what failed, so a vague error names it (reading a link vs. a download). */
export function errorMessage(
  e: ErrorCode,
  connected: boolean,
  during: 'resolve' | 'download' = 'resolve',
): { key: MessageKey; action?: ErrorAction } {
  const key = `err.${e.code}` as const
  switch (e.code) {
    case 'login_required':
      if (!LOGIN_SITES.includes(e.site)) return { key }
      return { key, action: connected ? 'reconnect' : 'connect' }
    case 'extractor_broken':
      return { key, action: 'update' }
    case 'network':
      return { key, action: 'retry' }
    case 'unknown':
      return { key: during === 'download' ? 'err.unknown_download' : key, action: 'details' }
    case 'tool_missing':
      return { key, action: 'reinstall' }
    default:
      return { key }
  }
}
