import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte'
import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { JobEvent, LumiError, MediaInfo, MediaOption } from '../types'

// jsdom has no Web Animations API; Svelte transitions call element.animate and wait for onfinish.
Element.prototype.animate ??= function () {
  const anim = { onfinish: null as null | (() => void), playState: 'finished', effect: null, cancel() {} }
  queueMicrotask(() => anim.onfinish?.())
  return anim as unknown as Animation
}

const h = vi.hoisted(() => ({
  jobListeners: [] as ((e: JobEvent) => void)[],
  kv: new Map<string, unknown>(),
}))

vi.mock('../api', async (importOriginal) => {
  const real = await importOriginal<typeof import('../api')>()
  return {
    detectSiteLocal: real.detectSiteLocal,
    isLumiError: real.isLumiError,
    normalize: real.normalize,
    download: vi.fn(async () => 'job-1'),
    downloadAll: vi.fn(async () => 'job-all'),
    cancelJob: vi.fn(async () => {}),
    defaultFilename: vi.fn(async (t: string, e: string) => `${t}.${e}`),
    fetchThumbnail: vi.fn(async () => {
      throw { error: { code: 'network' }, detail: 'offline' }
    }),
    toolDownloadSize: vi.fn(async () => 80 * 1024 * 1024),
    installTool: vi.fn(async (tool: string) => ({ tool, installed: true, version: '7.1' })),
    onToolProgress: vi.fn(async () => () => {}),
    onJob: vi.fn(async (cb: (e: JobEvent) => void) => {
      h.jobListeners.push(cb)
      return () => {
        h.jobListeners = h.jobListeners.filter((l) => l !== cb)
      }
    }),
  }
})
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn(async () => {}), readText: vi.fn(async () => '') }))
vi.mock('@tauri-apps/plugin-opener', () => ({ revealItemInDir: vi.fn(async () => {}) }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn(async () => 'D:\\x\\a.mp3'), open: vi.fn(async () => 'D:\\x') }))
vi.mock('@tauri-apps/api/path', () => ({ downloadDir: vi.fn(async () => 'C:\\Downloads') }))
vi.mock('@tauri-apps/plugin-store', () => ({
  load: vi.fn(async () => ({
    get: async (k: string) => h.kv.get(k),
    set: async (k: string, v: unknown) => void h.kv.set(k, v),
    save: async () => {},
  })),
}))

import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager'
import { save } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import * as api from '../api'
import ErrorBanner from '../components/ErrorBanner.svelte'
import LinkInput from '../components/LinkInput.svelte'
import OptionRow from '../components/OptionRow.svelte'
import ResultCard from '../components/ResultCard.svelte'
import ToolPrompt from '../components/ToolPrompt.svelte'
import { language } from '../i18n'
import { toolRequests } from '../tools'

const opt = (o: Partial<MediaOption> & Pick<MediaOption, 'id' | 'kind' | 'label'>): MediaOption => ({
  ext: o.kind === 'video' ? 'mp4' : o.kind === 'image' ? 'jpg' : 'mp3',
  filesize: null,
  directUrl: null,
  note: null,
  ...o,
})

const single: MediaInfo = {
  sourceUrl: 'https://www.tiktok.com/@a/video/1',
  site: 'tiktok',
  engine: 'yt_dlp',
  title: 'Evening chill',
  author: '@a',
  durationSecs: 48,
  thumbnail: 'https://cdn/x.jpg',
  limitedFormats: false,
  items: [
    {
      index: 0,
      thumbnail: null,
      isVideo: true,
      options: [
        opt({ id: '0/audio', kind: 'audio', label: 'MP3', filesize: 1153434 }),
        opt({ id: '0/video:480', kind: 'video', label: '480p' }),
        opt({ id: 'sound', kind: 'original_sound', label: 'Track — Artist', directUrl: 'https://cdn/s.mp3' }),
        opt({ id: '0/video:1080', kind: 'video', label: '1080p', filesize: 8808038 }),
        opt({ id: '0/video:720', kind: 'video', label: '720p' }),
      ],
    },
  ],
}

const multi: MediaInfo = {
  ...single,
  site: 'instagram',
  engine: 'gallery_dl',
  durationSecs: null,
  items: [0, 1, 2].map((i) => ({
    index: i,
    thumbnail: `https://cdn/${i}.jpg`,
    isVideo: i === 2,
    options: [opt({ id: `${i}/image`, kind: 'image', label: 'JPG', directUrl: `https://cdn/${i}.jpg` })],
  })),
}

const getRequests = () => get(toolRequests)

function emit(e: Partial<JobEvent> & Pick<JobEvent, 'state'>) {
  const ev = { jobId: 'job-1', url: single.sourceUrl, optionId: '0/audio', ...e } as JobEvent
  h.jobListeners.forEach((l) => l(ev))
}

beforeEach(() => {
  language.set('en')
  h.kv.clear()
  toolRequests.set([])
  vi.clearAllMocks()
})

afterEach(() => {
  h.jobListeners = []
})

describe('ResultCard', () => {
  it('single item renders rows in order: videos desc, MP3, then Original sound under MP3', () => {
    const { container } = render(ResultCard, { info: single })
    const ids = [...container.querySelectorAll('[data-option]')].map((el) => el.getAttribute('data-option'))
    expect(ids).toEqual(['0/video:1080', '0/video:720', '0/video:480', '0/audio', 'sound'])
    expect(screen.getByText(/Original sound/)).toBeTruthy()
    expect(screen.getByText(/Track — Artist/)).toBeTruthy()
    expect(screen.getByText('@a · 0:48')).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Download all' })).toBeNull()
  })

  it('multi item renders ImageGrid and Download all', async () => {
    const { container } = render(ResultCard, { info: multi })
    expect(container.querySelectorAll('.grid [role="option"]').length).toBe(3)
    expect(screen.getByRole('button', { name: 'Download all' })).toBeTruthy()
    // The first item is open by default; clicking another cell opens that item's options.
    const visible = () => [...container.querySelectorAll('.rows:not([hidden]) [data-option]')].map((el) => el.getAttribute('data-option'))
    expect(visible()).toEqual(['0/image'])
    await fireEvent.click(container.querySelectorAll('.grid [role="option"]')[1])
    expect(visible()).toEqual(['1/image'])
  })

  it('Download all Cancel is disabled while cancelling and surfaces a failure', async () => {
    let fail!: (e: unknown) => void
    vi.mocked(api.cancelJob).mockReturnValueOnce(new Promise((_, rej) => (fail = rej)))
    render(ResultCard, { info: multi })
    await fireEvent.click(screen.getByRole('button', { name: 'Download all' }))
    await waitFor(() => expect(api.downloadAll).toHaveBeenCalledWith(multi.sourceUrl, 'D:\\x'))
    const cancel = (await screen.findByRole('button', { name: 'Cancel' })) as HTMLButtonElement
    await fireEvent.click(cancel)
    expect(api.cancelJob).toHaveBeenCalledWith('job-all')
    expect(cancel.disabled).toBe(true)
    fail(new Error('nope'))
    await screen.findByText("Couldn't cancel the download.")
    expect(cancel.disabled).toBe(false)
  })

  it('Download all offers ffmpeg and reuses the chosen folder', async () => {
    vi.mocked(api.downloadAll).mockRejectedValueOnce({ error: { code: 'tool_missing', tool: 'ffmpeg' }, detail: '' })
    render(ResultCard, { info: multi })
    await fireEvent.click(screen.getByRole('button', { name: 'Download all' }))
    await waitFor(() => expect(getRequests().map((r) => r.tool)).toEqual(['ffmpeg']))
    getRequests()[0].settle({ tool: 'ffmpeg', installed: true, version: '7.1' })
    await waitFor(() => expect(api.downloadAll).toHaveBeenCalledTimes(2))
    expect(vi.mocked(api.downloadAll).mock.calls[1]).toEqual([multi.sourceUrl, 'D:\\x'])
  })
})

describe('OptionRow', () => {
  it('hides Copy when directUrl is null', () => {
    render(OptionRow, { info: single, opt: opt({ id: '0/audio', kind: 'audio', label: 'MP3' }) })
    expect(screen.queryByRole('button', { name: /Copy/ })).toBeNull()
    expect(screen.getByRole('button', { name: /Download/ })).toBeTruthy()
  })

  it('shows Copy with the expiry tooltip when directUrl exists', () => {
    render(OptionRow, {
      info: single,
      opt: opt({ id: '0/audio', kind: 'audio', label: 'MP3', directUrl: 'https://cdn/a.m4a', note: { kind: 'source_audio', ext: 'm4a' } }),
    })
    const copy = screen.getByRole('button', { name: /Copy/ })
    expect(copy.getAttribute('title')).toContain('may expire within hours')
    expect(copy.getAttribute('title')).toContain('(m4a)')
  })

  it('shows progress then Open folder on a done event', async () => {
    render(OptionRow, { info: single, opt: opt({ id: '0/audio', kind: 'audio', label: 'MP3' }) })
    await fireEvent.click(screen.getByRole('button', { name: /Download/ }))
    await waitFor(() => expect(api.download).toHaveBeenCalledWith(single.sourceUrl, '0/audio', 'D:\\x\\a.mp3'))

    emit({ state: 'progress', fraction: 0.42 })
    const bar = await screen.findByRole('progressbar')
    expect(bar.getAttribute('aria-valuenow')).toBe('42')
    expect(screen.getByText('42%')).toBeTruthy()
    expect(screen.getByRole('button', { name: /Cancel/ })).toBeTruthy()

    emit({ jobId: 'other-job', state: 'done', path: 'X:\\nope.mp3' })
    expect(screen.queryByRole('button', { name: /Open folder/ })).toBeNull()

    emit({ state: 'done', path: 'D:\\x\\a.mp3' })
    const openBtn = await screen.findByRole('button', { name: /Open folder/ })
    expect(screen.queryByRole('progressbar')).toBeNull()
    await fireEvent.click(openBtn)
    expect(revealItemInDir).toHaveBeenCalledWith('D:\\x\\a.mp3')
  })

  it('shows a failed job inline and keeps the dest for a retry', async () => {
    render(OptionRow, { info: single, opt: opt({ id: '0/audio', kind: 'audio', label: 'MP3' }) })
    await fireEvent.click(screen.getByRole('button', { name: /Download/ }))
    await waitFor(() => expect(api.download).toHaveBeenCalledTimes(1))
    emit({ state: 'failed', error: { error: { code: 'network' }, detail: 'x' } })
    await screen.findByText('No network connection.')
    await fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
    await waitFor(() => expect(api.download).toHaveBeenCalledTimes(2))
    expect(vi.mocked(api.download).mock.calls[1]).toEqual([single.sourceUrl, '0/audio', 'D:\\x\\a.mp3'])
  })

  it('offers ffmpeg when it is missing, then downloads to the kept dest without a second Save as', async () => {
    const ffmpegMissing: LumiError = { error: { code: 'tool_missing', tool: 'ffmpeg' }, detail: 'ffmpeg is not installed' }
    vi.mocked(api.download).mockRejectedValueOnce(ffmpegMissing)
    render(OptionRow, { info: single, opt: opt({ id: '0/audio', kind: 'audio', label: 'MP3' }) })
    await fireEvent.click(screen.getByRole('button', { name: /Download/ }))
    await waitFor(() => expect(getRequests().map((r) => r.tool)).toEqual(['ffmpeg']))
    expect(screen.queryByText('The ffmpeg tool is missing.')).toBeNull()

    const req = getRequests()[0]
    render(ToolPrompt, { tool: req.tool, oninstalled: req.settle, onlater: () => req.settle(null) })
    const dialog = await screen.findByRole('dialog')
    await within(dialog).findByText('Lumi needs to download ffmpeg (~80.0 MB) once.')
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Download' }))

    expect(api.installTool).toHaveBeenCalledWith('ffmpeg')
    await waitFor(() => expect(api.download).toHaveBeenCalledTimes(2))
    expect(vi.mocked(api.download).mock.calls[1]).toEqual([single.sourceUrl, '0/audio', 'D:\\x\\a.mp3'])
    expect(save).toHaveBeenCalledTimes(1)
  })

  it('offers ffmpeg when a running job fails for it', async () => {
    render(OptionRow, { info: single, opt: opt({ id: '0/audio', kind: 'audio', label: 'MP3' }) })
    await fireEvent.click(screen.getByRole('button', { name: /Download/ }))
    await waitFor(() => expect(api.download).toHaveBeenCalledTimes(1))
    emit({ state: 'failed', error: { error: { code: 'tool_missing', tool: 'ffmpeg' }, detail: 'x' } })
    await waitFor(() => expect(getRequests().map((r) => r.tool)).toEqual(['ffmpeg']))
    getRequests()[0].settle({ tool: 'ffmpeg', installed: true, version: '7.1' })
    await waitFor(() => expect(api.download).toHaveBeenCalledTimes(2))
    expect(vi.mocked(api.download).mock.calls[1]).toEqual([single.sourceUrl, '0/audio', 'D:\\x\\a.mp3'])
    expect(save).toHaveBeenCalledTimes(1)
  })

  it('returns to idle when ffmpeg is declined', async () => {
    vi.mocked(api.download).mockRejectedValueOnce({ error: { code: 'tool_missing', tool: 'ffmpeg' }, detail: '' })
    render(OptionRow, { info: single, opt: opt({ id: '0/audio', kind: 'audio', label: 'MP3' }) })
    await fireEvent.click(screen.getByRole('button', { name: /Download/ }))
    await waitFor(() => expect(getRequests().length).toBe(1))
    getRequests()[0].settle(null)
    await waitFor(() => expect((screen.getByRole('button', { name: /Download/ }) as HTMLButtonElement).disabled).toBe(false))
    expect(api.download).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('a copy failure shows a short notice and never offers a download Retry', async () => {
    vi.mocked(writeText).mockRejectedValueOnce(new Error('denied'))
    render(OptionRow, { info: single, opt: opt({ id: 'sound', kind: 'original_sound', label: 'T', directUrl: 'https://cdn/s.mp3' }) })
    await fireEvent.click(screen.getByRole('button', { name: /Copy/ }))
    await screen.findByText("Couldn't copy the link.")
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull()
    expect(screen.queryByRole('alert')).toBeNull()
    expect(screen.getByRole('button', { name: /Download/ })).toBeTruthy()
  })

  it('a reveal failure keeps the row done', async () => {
    vi.mocked(revealItemInDir).mockRejectedValueOnce(new Error('gone'))
    render(OptionRow, { info: single, opt: opt({ id: '0/audio', kind: 'audio', label: 'MP3' }) })
    await fireEvent.click(screen.getByRole('button', { name: /Download/ }))
    await waitFor(() => expect(api.download).toHaveBeenCalledTimes(1))
    emit({ state: 'done', path: 'D:\\x\\a.mp3' })
    await fireEvent.click(await screen.findByRole('button', { name: /Open folder/ }))
    await screen.findByText("Couldn't open the folder.")
    expect(screen.getByRole('button', { name: /Open folder/ })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull()
  })
})

describe('ErrorBanner', () => {
  const login: LumiError = { error: { code: 'login_required', site: 'instagram' }, detail: 'raw log' }

  it('renders the mapped message and action in English', async () => {
    const onaction = vi.fn()
    render(ErrorBanner, { error: login, connected: false, onaction })
    expect(screen.getByText('This post requires signing in to Instagram.')).toBeTruthy()
    await fireEvent.click(screen.getByRole('button', { name: 'Connect Instagram' }))
    expect(onaction).toHaveBeenCalledWith('connect')
    expect(screen.queryByText('raw log')).toBeNull()
  })

  it('renders the mapped message and action in Vietnamese', () => {
    language.set('vi')
    render(ErrorBanner, { error: login, connected: true })
    expect(screen.getByText('Bài này cần đăng nhập Instagram.')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Kết nối lại Instagram' })).toBeTruthy()
  })

  it('puts the raw detail behind Show details', () => {
    const { container } = render(ErrorBanner, { error: { error: { code: 'unknown' }, detail: 'Traceback…' } })
    expect(screen.getByText('Something went wrong reading this link.')).toBeTruthy()
    const details = container.querySelector('details')!
    expect(details.open).toBe(false)
    expect(details.querySelector('summary')?.textContent).toContain('Show details')
    expect(details.textContent).toContain('Traceback…')
  })
})

describe('LinkInput', () => {
  it('suggests a copied link on focus and only fills it in', async () => {
    vi.mocked(readText).mockResolvedValue('https://vt.tiktok.com/ZSabc/')
    const onsubmit = vi.fn()
    render(LinkInput, { value: '', busy: false, lastResolved: '', onsubmit })
    window.dispatchEvent(new Event('focus'))
    const chip = await screen.findByRole('button', { name: /Paste the link you just copied\?/ })
    await fireEvent.click(chip)
    expect((screen.getByRole('textbox') as HTMLInputElement).value).toBe('https://vt.tiktok.com/ZSabc/')
    expect(onsubmit).not.toHaveBeenCalled()
  })

  it('ignores the clipboard when it holds the last resolved link or no supported link', async () => {
    vi.mocked(readText).mockResolvedValue('https://vt.tiktok.com/ZSabc/')
    render(LinkInput, { value: '', busy: false, lastResolved: 'https://vt.tiktok.com/ZSabc/', onsubmit: vi.fn() })
    window.dispatchEvent(new Event('focus'))
    await new Promise((r) => setTimeout(r, 10))
    expect(screen.queryByRole('button', { name: /Paste the link/ })).toBeNull()
  })

  it('runs on Enter', async () => {
    const onsubmit = vi.fn()
    render(LinkInput, { value: 'https://youtu.be/x', busy: false, lastResolved: '', onsubmit })
    await fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Enter' })
    expect(onsubmit).toHaveBeenCalledTimes(1)
  })
})
