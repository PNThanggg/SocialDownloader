import type { open, save } from '@tauri-apps/plugin-dialog'
import type { Store } from '@tauri-apps/plugin-store'
import type * as Api from './api'
import type { JobEvent, LumiError, MediaInfo, MediaOption } from './types'

const LAST_DIR = 'lastDir'

/** Everything the Save-as flow touches, injected so it can be tested without Tauri. */
export interface DownloadDeps {
  save: typeof save
  open: typeof open
  store: Pick<Store, 'get' | 'set' | 'save'>
  api: Pick<typeof Api, 'download' | 'downloadAll' | 'defaultFilename'>
  /** First-run default folder (Downloads). */
  downloadDir: () => Promise<string>
}

/** The real dependencies; loaded lazily so tests and plain-browser dev never touch Tauri. */
export async function realDeps(): Promise<DownloadDeps> {
  const [dialog, path, api, { settingsStore }] = await Promise.all([
    import('@tauri-apps/plugin-dialog'),
    import('@tauri-apps/api/path'),
    import('./api'),
    import('./i18n/settings'),
  ])
  return { save: dialog.save, open: dialog.open, store: await settingsStore(), api, downloadDir: path.downloadDir }
}

/** Windows join; never doubles a trailing separator. */
export function joinPath(dir: string, name: string): string {
  return /[\\/]$/.test(dir) ? dir + name : `${dir}\\${name}`
}

/** Folder of a file path; a drive root keeps its backslash (`E:\a.mp3` → `E:\`). */
export function parentDir(path: string): string | null {
  const i = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'))
  if (i < 0) return null
  const dir = path.slice(0, i)
  return /^[a-zA-Z]:$/.test(dir) ? `${dir}\\` : dir
}

async function startDir(deps: DownloadDeps): Promise<string | undefined> {
  const last = await deps.store.get<unknown>(LAST_DIR)
  if (typeof last === 'string' && last) return last
  try {
    return await deps.downloadDir()
  } catch {
    return undefined
  }
}

async function rememberDir(deps: DownloadDeps, dir: string): Promise<void> {
  await deps.store.set(LAST_DIR, dir)
  await deps.store.save()
}

/** Item index encoded in an option id (`2/image` → 2); null for `sound`. */
function itemOf(opt: MediaOption): number | null {
  const n = Number.parseInt(opt.id.split('/')[0], 10)
  return Number.isNaN(n) ? null : n
}

function titleFor(info: MediaInfo, opt: MediaOption): string {
  if (opt.kind === 'original_sound') return opt.label
  const item = itemOf(opt)
  return info.items.length > 1 && item !== null ? `${info.title} ${item + 1}` : info.title
}

/** Opens Save as with the remembered folder and a cleaned file name. null = cancelled. */
export async function pickDest(deps: DownloadDeps, info: MediaInfo, opt: MediaOption): Promise<string | null> {
  const name = await deps.api.defaultFilename(titleFor(info, opt), opt.ext)
  const dir = await startDir(deps)
  const dest = await deps.save({
    defaultPath: dir ? joinPath(dir, name) : name,
    filters: [{ name: opt.ext.toUpperCase(), extensions: [opt.ext] }],
  })
  if (!dest) return null
  const chosenDir = parentDir(dest)
  if (chosenDir) await rememberDir(deps, chosenDir)
  return dest
}

/** Save as, then start the job. Resolves to the job id, or null when the dialog was cancelled. */
export async function startDownload(deps: DownloadDeps, info: MediaInfo, opt: MediaOption): Promise<string | null> {
  const dest = await pickDest(deps, info, opt)
  return dest === null ? null : deps.api.download(info.sourceUrl, opt.id, dest)
}

/** Folder picker for "Download all", starting at the remembered folder. null = cancelled. */
export async function pickDir(deps: DownloadDeps): Promise<string | null> {
  const dir = await deps.open({ directory: true, defaultPath: await startDir(deps) })
  if (typeof dir !== 'string' || !dir) return null
  await rememberDir(deps, dir)
  return dir
}

/** Folder picker, then download every item into it. null = cancelled. */
export async function startDownloadAll(deps: DownloadDeps, info: MediaInfo): Promise<string | null> {
  const dir = await pickDir(deps)
  return dir === null ? null : deps.api.downloadAll(info.sourceUrl, dir)
}

/** What a download control shows. */
export type JobView =
  | { s: 'idle' }
  | { s: 'busy' } // dialog open or the job is being created
  | { s: 'running'; fraction: number | null }
  | { s: 'processing' }
  | { s: 'done'; path: string }
  | { s: 'error'; error: LumiError }

/** Maps a job event to the view it implies; `cancelled` returns to idle. */
export function viewOf(e: JobEvent): JobView {
  switch (e.state) {
    case 'started':
      return { s: 'running', fraction: null }
    case 'progress':
      return { s: 'running', fraction: e.fraction }
    case 'postprocessing':
      return { s: 'processing' }
    case 'done':
      return { s: 'done', path: e.path }
    case 'failed':
      return { s: 'error', error: e.error }
    case 'cancelled':
      return { s: 'idle' }
  }
}

/**
 * Follows one job by id. Events can arrive before `download` resolves with the id,
 * so while armed they are buffered and replayed once the id is known.
 */
export class JobFollower {
  private id: string | null = null
  private armed = false
  private buffer: JobEvent[] = []

  constructor(private readonly onEvent: (e: JobEvent) => void) {}

  /** Call just before starting a job. */
  arm(): void {
    this.id = null
    this.armed = true
    this.buffer = []
  }

  follow(jobId: string): void {
    this.id = jobId
    this.armed = false
    const early = this.buffer.filter((e) => e.jobId === jobId)
    this.buffer = []
    early.forEach(this.onEvent)
  }

  stop(): void {
    this.id = null
    this.armed = false
    this.buffer = []
  }

  get jobId(): string | null {
    return this.id
  }

  push(e: JobEvent): void {
    if (this.armed) {
      if (this.buffer.length < 64) this.buffer.push(e)
    } else if (this.id !== null && e.jobId === this.id) {
      this.onEvent(e)
    }
  }
}
