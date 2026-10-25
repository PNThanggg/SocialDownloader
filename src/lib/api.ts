import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AccountStatus,
  JobEvent,
  LumiError,
  MediaInfo,
  Site,
  Tool,
  ToolProgress,
  ToolStatus,
} from './types'

export function isLumiError(e: unknown): e is LumiError {
  if (typeof e !== 'object' || e === null) return false
  const err = (e as { error?: unknown }).error
  return typeof err === 'object' && err !== null && typeof (err as { code?: unknown }).code === 'string'
}

/** Any thrown value as a LumiError (unknown + its text when it is not one already). */
export function normalize(e: unknown): LumiError {
  if (isLumiError(e)) return e
  return { error: { code: 'unknown' }, detail: String(e) }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    throw normalize(e)
  }
}

export const resolve = (url: string) => call<MediaInfo>('resolve', { url })
export const download = (url: string, optionId: string, dest: string) =>
  call<string>('download', { url, optionId, dest })
export const downloadAll = (url: string, destDir: string) => call<string>('download_all', { url, destDir })
export const cancelJob = (jobId: string) => call<void>('cancel_job', { jobId })

export const toolsStatus = () => call<ToolStatus[]>('tools_status')
export const installTool = (tool: Tool) => call<ToolStatus>('install_tool', { tool })
export const toolDownloadSize = (tool: Tool) => call<number | null>('tool_download_size', { tool })
export const updateTools = () => call<ToolStatus[]>('update_tools')

export const accountsStatus = () => call<AccountStatus[]>('accounts_status')
export const connectAccount = (site: Site) => call<AccountStatus>('connect_account', { site })
export const disconnectAccount = (site: Site) => call<void>('disconnect_account', { site })

export const fetchThumbnail = (url: string, site: Site) => call<string>('fetch_thumbnail', { url, site })
export const defaultFilename = (title: string, ext: string) => call<string>('default_filename', { title, ext })
export const windowEffect = () => call<'mica' | 'acrylic' | 'none'>('window_effect')

export const onJob = (cb: (e: JobEvent) => void): Promise<UnlistenFn> =>
  listen<JobEvent>('lumi://job', (ev) => cb(ev.payload))
export const onToolProgress = (cb: (p: ToolProgress) => void): Promise<UnlistenFn> =>
  listen<ToolProgress>('lumi://tool-progress', (ev) => cb(ev.payload))

const SITE_HOSTS: [string, Site][] = [
  ['tiktok.com', 'tiktok'],
  ['instagram.com', 'instagram'],
  ['youtube.com', 'youtube'],
  ['youtu.be', 'youtube'],
  ['facebook.com', 'facebook'],
  ['fb.watch', 'facebook'],
]

/** JS mirror of Rust `detect_site`; only used to decide whether to offer deno before resolving. */
export function detectSiteLocal(url: string): Site | null {
  let u: URL
  try {
    u = new URL(url.trim())
  } catch {
    return null
  }
  if (u.protocol !== 'http:' && u.protocol !== 'https:') return null
  const host = u.hostname.toLowerCase()
  const hit = SITE_HOSTS.find(([h]) => host === h || host.endsWith(`.${h}`))
  return hit ? hit[1] : 'other'
}
