import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte'
import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { AccountStatus, LumiError, MediaInfo, Tool, ToolProgress, ToolStatus, ToolUpdateReport } from '../types'

// jsdom has no Web Animations API; Svelte transitions call element.animate and wait for onfinish.
Element.prototype.animate ??= function () {
  const anim = { onfinish: null as null | (() => void), playState: 'finished', effect: null, cancel() {} }
  queueMicrotask(() => anim.onfinish?.())
  return anim as unknown as Animation
}

const MB = 1024 * 1024

const h = vi.hoisted(() => ({
  progress: [] as ((p: ToolProgress) => void)[],
  kv: new Map<string, unknown>(),
}))

vi.mock('../api', async (importOriginal) => {
  const real = await importOriginal<typeof import('../api')>()
  return {
    detectSiteLocal: real.detectSiteLocal,
    isLumiError: real.isLumiError,
    normalize: real.normalize,
    resolve: vi.fn(),
    download: vi.fn(async () => 'job-1'),
    downloadAll: vi.fn(async () => 'job-all'),
    cancelJob: vi.fn(async () => {}),
    defaultFilename: vi.fn(async (t: string, e: string) => `${t}.${e}`),
    fetchThumbnail: vi.fn(async () => {
      throw { error: { code: 'network' }, detail: 'offline' }
    }),
    onJob: vi.fn(async () => () => {}),
    toolsStatus: vi.fn(),
    installTool: vi.fn(),
    toolDownloadSize: vi.fn(async () => 80 * MB),
    updateTools: vi.fn(),
    onToolProgress: vi.fn(async (cb: (p: ToolProgress) => void) => {
      h.progress.push(cb)
      return () => {
        h.progress = h.progress.filter((l) => l !== cb)
      }
    }),
    accountsStatus: vi.fn(async () => []),
    connectAccount: vi.fn(),
    disconnectAccount: vi.fn(async () => {}),
  }
})
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ minimize: vi.fn(), close: vi.fn() }) }))
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

import App from '../../App.svelte'
import { accountActions, accountErrors, accounts, connectAccount } from '../accounts'
import * as api from '../api'
import SettingsPanel from '../components/SettingsPanel.svelte'
import SetupScreen from '../components/SetupScreen.svelte'
import ToolPrompt from '../components/ToolPrompt.svelte'
import { language } from '../i18n'
import { askForTool, toolRequests, toolStatuses } from '../tools'

const status = (tool: Tool, installed = true, version: string | null = installed ? '1.0' : null): ToolStatus => ({
  tool,
  installed,
  version,
})

function progress(p: ToolProgress) {
  h.progress.forEach((l) => l(p))
}

/** A promise plus the functions that settle it. */
function deferred<T>() {
  let resolve!: (v: T) => void
  let reject!: (e: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

const network: LumiError = { error: { code: 'network' }, detail: 'offline' }

beforeEach(() => {
  language.set('en')
  h.kv.clear()
  h.progress = []
  toolRequests.set([])
  toolStatuses.set(null)
  accounts.set([])
  accountActions.set({})
  accountErrors.set({})
  vi.clearAllMocks()
  vi.mocked(api.toolDownloadSize).mockResolvedValue(80 * MB)
})

afterEach(() => {
  toolRequests.set([])
})

describe('SetupScreen', () => {
  it('installs the missing tools in order and shows progress', async () => {
    const gates = { yt_dlp: deferred<ToolStatus>(), gallery_dl: deferred<ToolStatus>() } as Record<Tool, ReturnType<typeof deferred<ToolStatus>>>
    vi.mocked(api.installTool).mockImplementation((tool) => gates[tool].promise)
    const onready = vi.fn()
    render(SetupScreen, { missing: ['yt_dlp', 'gallery_dl'], onready })

    expect(screen.getByText('Getting ready…')).toBeTruthy()
    await waitFor(() => expect(api.installTool).toHaveBeenCalledWith('yt_dlp'))
    progress({ tool: 'yt_dlp', downloaded: 42, total: 100 })
    await waitFor(() => expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('42'))
    // Events for another tool never move this bar.
    progress({ tool: 'ffmpeg', downloaded: 90, total: 100 })
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('42')
    expect(api.installTool).toHaveBeenCalledTimes(1)

    gates.yt_dlp.resolve(status('yt_dlp'))
    await waitFor(() => expect(api.installTool).toHaveBeenCalledWith('gallery_dl'))
    progress({ tool: 'gallery_dl', downloaded: 5, total: null })
    await waitFor(() => expect(screen.getByRole('progressbar').classList.contains('indeterminate')).toBe(true))
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBeNull()
    expect(onready).not.toHaveBeenCalled()

    gates.gallery_dl.resolve(status('gallery_dl'))
    await waitFor(() => expect(onready).toHaveBeenCalledTimes(1))
    expect(vi.mocked(api.installTool).mock.calls.map((c) => c[0])).toEqual(['yt_dlp', 'gallery_dl'])
  })

  it('shows the network error with Retry and resumes from the failed tool', async () => {
    vi.mocked(api.installTool)
      .mockResolvedValueOnce(status('yt_dlp'))
      .mockRejectedValueOnce(network)
      .mockResolvedValueOnce(status('gallery_dl'))
    const onready = vi.fn()
    render(SetupScreen, { missing: ['yt_dlp', 'gallery_dl'], onready })

    await screen.findByText('No network connection.')
    expect(onready).not.toHaveBeenCalled()
    await fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
    await waitFor(() => expect(onready).toHaveBeenCalledTimes(1))
    expect(vi.mocked(api.installTool).mock.calls.map((c) => c[0])).toEqual(['yt_dlp', 'gallery_dl', 'gallery_dl'])
    expect(screen.queryByText('No network connection.')).toBeNull()
  })

  it('is in Vietnamese when the language is vi', () => {
    language.set('vi')
    vi.mocked(api.installTool).mockReturnValue(new Promise(() => {}))
    render(SetupScreen, { missing: ['yt_dlp'], onready: vi.fn() })
    expect(screen.getByText('Đang chuẩn bị…')).toBeTruthy()
  })
})

describe('ToolPrompt', () => {
  const props = () => ({ tool: 'ffmpeg' as Tool, oninstalled: vi.fn(), onlater: vi.fn() })

  it('states the size in English and Vietnamese', async () => {
    render(ToolPrompt, props())
    await screen.findByText('Lumi needs to download ffmpeg (~80.0 MB) once.')
    language.set('vi')
    await screen.findByText('Cần tải thêm ffmpeg (~80.0 MB) một lần.')
  })

  it('leaves the size out when it is unknown, in both languages', async () => {
    vi.mocked(api.toolDownloadSize).mockResolvedValue(null)
    render(ToolPrompt, props())
    await screen.findByText('Lumi needs to download ffmpeg once.')
    language.set('vi')
    await screen.findByText('Cần tải thêm ffmpeg một lần.')
  })

  it('leaves the size out when asking for it fails', async () => {
    vi.mocked(api.toolDownloadSize).mockRejectedValue(network)
    render(ToolPrompt, { ...props(), tool: 'deno' })
    await screen.findByText('Lumi needs to download deno once.')
  })

  it('installs with progress, then reports the new status', async () => {
    const gate = deferred<ToolStatus>()
    vi.mocked(api.installTool).mockReturnValue(gate.promise)
    const p = props()
    render(ToolPrompt, p)
    await fireEvent.click(screen.getByRole('button', { name: 'Download' }))
    expect(api.installTool).toHaveBeenCalledWith('ffmpeg')
    await waitFor(() => expect(h.progress.length).toBe(1))
    progress({ tool: 'ffmpeg', downloaded: 25, total: 100 })
    await waitFor(() => expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('25'))
    expect(screen.queryByRole('button', { name: 'Later' })).toBeNull()
    gate.resolve(status('ffmpeg', true, '7.1'))
    await waitFor(() => expect(p.oninstalled).toHaveBeenCalledWith(status('ffmpeg', true, '7.1')))
    expect(h.progress.length).toBe(0)
  })

  it('shows a failed install with Retry; Later and Escape decline', async () => {
    vi.mocked(api.installTool).mockRejectedValueOnce(network)
    const p = props()
    render(ToolPrompt, p)
    await fireEvent.click(screen.getByRole('button', { name: 'Download' }))
    await screen.findByText('No network connection.')
    expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy()
    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(p.onlater).toHaveBeenCalledTimes(1)
    await fireEvent.click(screen.getByRole('button', { name: 'Later' }))
    expect(p.onlater).toHaveBeenCalledTimes(2)
  })
})

describe('App deno prompt', () => {
  const youtube: MediaInfo = {
    sourceUrl: 'https://www.youtube.com/watch?v=abc',
    site: 'youtube',
    engine: 'yt_dlp',
    title: 'Lo-fi mix',
    author: 'Lumi',
    durationSecs: 60,
    thumbnail: null,
    limitedFormats: true,
    items: [{ index: 0, thumbnail: null, isVideo: true, options: [] }],
  }

  async function startApp(denoInstalled: boolean) {
    vi.mocked(api.toolsStatus).mockResolvedValue([
      status('yt_dlp'),
      status('gallery_dl'),
      status('ffmpeg'),
      status('deno', denoInstalled),
    ])
    vi.mocked(api.resolve).mockImplementation(async (url) => ({ ...youtube, sourceUrl: url, limitedFormats: !denoInstalled }))
    render(App)
    await waitFor(() => expect(get(toolStatuses)).not.toBeNull())
  }

  async function submit(url: string) {
    const input = screen.getByRole('textbox')
    await fireEvent.input(input, { target: { value: url } })
    await fireEvent.click(screen.getByRole('button', { name: 'Get links' }))
  }

  it('asks before the first YouTube link; Later resolves anyway and is not asked again', async () => {
    await startApp(false)
    await submit('https://www.youtube.com/watch?v=abc')
    const dialog = await screen.findByRole('dialog')
    await within(dialog).findByText('Lumi needs to download deno (~80.0 MB) once.')
    expect(api.resolve).not.toHaveBeenCalled()

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Later' }))
    await waitFor(() => expect(api.resolve).toHaveBeenCalledTimes(1))
    await screen.findByText('Not all qualities available')
    expect(screen.getByRole('button', { name: 'Get all qualities' })).toBeTruthy()

    await submit('https://youtu.be/xyz')
    await waitFor(() => expect(api.resolve).toHaveBeenCalledTimes(2))
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(api.installTool).not.toHaveBeenCalled()
  })

  it('does not ask for other sites', async () => {
    await startApp(false)
    vi.mocked(api.resolve).mockResolvedValue({ ...youtube, site: 'tiktok', limitedFormats: false })
    await submit('https://www.tiktok.com/@a/video/1')
    await waitFor(() => expect(api.resolve).toHaveBeenCalledTimes(1))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('does not ask when deno is installed', async () => {
    await startApp(true)
    await submit('https://www.youtube.com/watch?v=abc')
    await waitFor(() => expect(api.resolve).toHaveBeenCalledTimes(1))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('installs deno when accepted, then resolves', async () => {
    await startApp(false)
    vi.mocked(api.installTool).mockResolvedValue(status('deno'))
    await submit('https://www.youtube.com/watch?v=abc')
    const dialog = await screen.findByRole('dialog')
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Download' }))
    await waitFor(() => expect(api.resolve).toHaveBeenCalledTimes(1))
    expect(api.installTool).toHaveBeenCalledWith('deno')
    expect(vi.mocked(api.installTool).mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(api.resolve).mock.invocationCallOrder[0])
  })

  it('the limited-formats button installs deno and resolves again', async () => {
    await startApp(false)
    await submit('https://www.youtube.com/watch?v=abc')
    await fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Later' }))
    await screen.findByText('Not all qualities available')

    vi.mocked(api.installTool).mockResolvedValue(status('deno'))
    vi.mocked(api.resolve).mockResolvedValue({ ...youtube, limitedFormats: false })
    await fireEvent.click(screen.getByRole('button', { name: 'Get all qualities' }))
    await waitFor(() => expect(api.resolve).toHaveBeenCalledTimes(2))
    expect(api.installTool).toHaveBeenCalledWith('deno')
    expect(vi.mocked(api.resolve).mock.calls[1][0]).toBe('https://www.youtube.com/watch?v=abc')
    await waitFor(() => expect(screen.queryByText('Not all qualities available')).toBeNull())
  })

  it('shows the setup screen first when yt-dlp is missing', async () => {
    vi.mocked(api.toolsStatus).mockResolvedValue([status('yt_dlp', false), status('gallery_dl'), status('ffmpeg'), status('deno')])
    vi.mocked(api.installTool).mockResolvedValue(status('yt_dlp'))
    render(App)
    await screen.findByText('Getting ready…')
    await screen.findByText('Paste a link here')
    expect(vi.mocked(api.installTool).mock.calls.map((c) => c[0])).toEqual(['yt_dlp'])
  })
})

describe('App update action', () => {
  const broken: LumiError = { error: { code: 'extractor_broken' }, detail: 'Unable to extract' }

  async function failOnce() {
    vi.mocked(api.toolsStatus).mockResolvedValue([status('yt_dlp'), status('gallery_dl'), status('ffmpeg'), status('deno')])
    vi.mocked(api.resolve).mockRejectedValue(broken)
    render(App)
    await waitFor(() => expect(get(toolStatuses)).not.toBeNull())
    await fireEvent.input(screen.getByRole('textbox'), { target: { value: 'https://www.tiktok.com/@a/video/1' } })
    await fireEvent.click(screen.getByRole('button', { name: 'Get links' }))
    await screen.findByText('This site just changed, try updating the tools.')
  }

  it('updates the tools, then reads the link again', async () => {
    await failOnce()
    vi.mocked(api.updateTools).mockResolvedValue({ statuses: [status('yt_dlp', true, '2026.1.1')], errors: [] })
    await fireEvent.click(screen.getByRole('button', { name: 'Update' }))
    await waitFor(() => expect(api.resolve).toHaveBeenCalledTimes(2))
  })

  it('shows why the update failed instead of retrying', async () => {
    await failOnce()
    vi.mocked(api.updateTools).mockResolvedValue({ statuses: [status('yt_dlp')], errors: [{ tool: 'yt_dlp', error: network }] })
    await fireEvent.click(screen.getByRole('button', { name: 'Update' }))
    await screen.findByText('No network connection.')
    expect(api.resolve).toHaveBeenCalledTimes(1)
  })
})

describe('App startup', () => {
  it('cannot read a link until the tool check is done, and setup leaves nothing stale', async () => {
    const check = deferred<ToolStatus[]>()
    vi.mocked(api.toolsStatus).mockReturnValue(check.promise)
    vi.mocked(api.installTool).mockResolvedValue(status('yt_dlp'))
    vi.mocked(api.resolve).mockRejectedValue({ error: { code: 'tool_missing', tool: 'yt_dlp' }, detail: '' })
    render(App)

    const input = screen.getByRole('textbox')
    await fireEvent.input(input, { target: { value: 'https://www.youtube.com/watch?v=abc' } })
    const go = screen.getByRole('button', { name: 'Get links' }) as HTMLButtonElement
    expect(go.disabled).toBe(true)
    await fireEvent.keyDown(input, { key: 'Enter' })
    await fireEvent.click(go)
    expect(api.resolve).not.toHaveBeenCalled()

    check.resolve([status('yt_dlp', false), status('gallery_dl'), status('ffmpeg'), status('deno')])
    await screen.findByText('Getting ready…')
    await screen.findByText('Paste a link here')
    expect(screen.queryByText('The yt-dlp tool is missing.')).toBeNull()
    expect(screen.queryByRole('alert')).toBeNull()
    expect(api.resolve).not.toHaveBeenCalled()
    expect((screen.getByRole('button', { name: 'Get links' }) as HTMLButtonElement).disabled).toBe(false)
  })
})

describe('SettingsPanel', () => {
  const signedIn: AccountStatus[] = [
    { site: 'instagram', connected: false },
    { site: 'facebook', connected: true },
  ]

  beforeEach(() => {
    vi.mocked(api.accountsStatus).mockResolvedValue(signedIn)
    vi.mocked(api.toolsStatus).mockResolvedValue([
      status('yt_dlp', true, '2025.10.22'),
      status('gallery_dl', true, '1.30.10'),
      status('ffmpeg', false),
      status('deno', true, 'deno 2.1.4 (stable)'),
    ])
  })

  it('switches the language and the text follows', async () => {
    render(SettingsPanel, { onclose: vi.fn() })
    expect(screen.getByRole('heading', { name: 'Settings' })).toBeTruthy()
    await fireEvent.click(screen.getByRole('radio', { name: 'Tiếng Việt' }))
    expect(get(language)).toBe('vi')
    await screen.findByRole('heading', { name: 'Cài đặt' })
    expect(screen.getByText('Ngôn ngữ')).toBeTruthy()
    await fireEvent.click(screen.getByRole('radio', { name: 'English' }))
    await screen.findByRole('heading', { name: 'Settings' })
    await fireEvent.click(screen.getByRole('radio', { name: 'Automatic' }))
    expect(get(language)).toBe('auto')
  })

  const rowOf = async (site: string) => (await screen.findByText(site)).closest('[data-site]') as HTMLElement

  it('shows a busy state while connectAccount is pending', async () => {
    const login = deferred<AccountStatus>()
    vi.mocked(api.connectAccount).mockReturnValue(login.promise)
    render(SettingsPanel, { onclose: vi.fn() })
    const row = await rowOf('Instagram')
    await within(row).findByText('Not connected')
    await fireEvent.click(within(row).getByRole('button', { name: 'Connect' }))
    expect(api.connectAccount).toHaveBeenCalledWith('instagram')
    const waiting = within(row).getByRole('button', { name: /Waiting for sign-in/ }) as HTMLButtonElement
    expect(waiting.disabled).toBe(true)

    login.resolve({ site: 'instagram', connected: true })
    await within(row).findByText('Connected')
    expect(within(row).getByRole('button', { name: 'Sign out' })).toBeTruthy()
    // The shared list (error banners read it too) has the new state.
    expect(get(accounts)).toEqual([
      { site: 'instagram', connected: true },
      { site: 'facebook', connected: true },
    ])
  })

  it('keeps a pending Connect across closing and reopening the panel', async () => {
    const login = deferred<AccountStatus>()
    vi.mocked(api.connectAccount).mockReturnValue(login.promise)
    const first = render(SettingsPanel, { onclose: vi.fn() })
    await within(await rowOf('Instagram')).findByText('Not connected')
    await fireEvent.click(within(await rowOf('Instagram')).getByRole('button', { name: 'Connect' }))
    first.unmount()

    render(SettingsPanel, { onclose: vi.fn() })
    await waitFor(() => expect(api.accountsStatus).toHaveBeenCalledTimes(2))
    const row = await rowOf('Instagram')
    expect(within(row).getByRole('button', { name: /Waiting for sign-in/ })).toBeTruthy()
    expect(within(row).queryByRole('button', { name: 'Connect' })).toBeNull()

    login.resolve({ site: 'instagram', connected: true })
    await within(row).findByText('Connected')
    expect(api.connectAccount).toHaveBeenCalledTimes(1)
  })

  it('joins a pending Connect instead of opening a second login', async () => {
    const login = deferred<AccountStatus>()
    vi.mocked(api.connectAccount).mockReturnValue(login.promise)
    const a = connectAccount('instagram')
    const b = connectAccount('instagram')
    login.resolve({ site: 'instagram', connected: true })
    expect(await a).toEqual(await b)
    expect(api.connectAccount).toHaveBeenCalledTimes(1)
    expect(get(accountActions)).toEqual({})
  })

  it('leaves Escape to a tool prompt shown above it', async () => {
    const onclose = vi.fn()
    render(SettingsPanel, { onclose })
    void askForTool('ffmpeg')
    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(onclose).not.toHaveBeenCalled()
    get(toolRequests)[0].settle(null)
    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(onclose).toHaveBeenCalledTimes(1)
  })

  it('signs out', async () => {
    render(SettingsPanel, { onclose: vi.fn() })
    const row = (await screen.findByText('Facebook')).closest('[data-site]') as HTMLElement
    await fireEvent.click(await within(row).findByRole('button', { name: 'Sign out' }))
    expect(api.disconnectAccount).toHaveBeenCalledWith('facebook')
    await within(row).findByText('Not connected')
  })

  it('lists tool versions and updates them', async () => {
    const update = deferred<ToolUpdateReport>()
    vi.mocked(api.updateTools).mockReturnValue(update.promise)
    render(SettingsPanel, { onclose: vi.fn() })
    await screen.findByText('2025.10.22')
    expect(screen.getByText('1.30.10')).toBeTruthy()
    expect(screen.getByText('2.1.4')).toBeTruthy()
    expect(screen.getByText('Not installed')).toBeTruthy()

    const btn = screen.getByRole('button', { name: 'Update' }) as HTMLButtonElement
    await fireEvent.click(btn)
    expect(btn.disabled).toBe(true)
    expect(btn.querySelector('.spinner')).toBeTruthy()
    update.resolve({
      statuses: [status('yt_dlp', true, '2025.11.01'), status('gallery_dl', true, '1.31.0'), status('ffmpeg', false), status('deno', true, 'deno 2.2.0')],
      errors: [],
    })
    await screen.findByText('2025.11.01')
    expect(btn.disabled).toBe(false)
    expect(screen.getByText('Updated')).toBeTruthy()
    expect(get(toolStatuses)?.find((s) => s.tool === 'yt_dlp')?.version).toBe('2025.11.01')
  })

  it('reports the tool that failed while the others still update', async () => {
    vi.mocked(api.updateTools).mockResolvedValue({
      statuses: [status('yt_dlp', true, '2025.10.22'), status('gallery_dl', true, '1.31.0'), status('ffmpeg', false), status('deno', true, 'deno 2.1.4')],
      errors: [{ tool: 'yt_dlp', error: network }],
    })
    const { container } = render(SettingsPanel, { onclose: vi.fn() })
    await screen.findByText('1.30.10')
    await fireEvent.click(screen.getByRole('button', { name: 'Update' }))
    await screen.findByText('1.31.0')
    const err = container.querySelector('[data-tool="yt_dlp"]') as HTMLElement
    expect(within(err).getByText('yt-dlp')).toBeTruthy()
    expect(within(err).getByText('No network connection.')).toBeTruthy()
    expect(screen.queryByText('Updated')).toBeNull()
  })

  it('shows a failed update inline', async () => {
    vi.mocked(api.updateTools).mockRejectedValue(network)
    render(SettingsPanel, { onclose: vi.fn() })
    await fireEvent.click(screen.getByRole('button', { name: 'Update' }))
    await screen.findByText('No network connection.')
  })

  it('closes on Escape and on the backdrop', async () => {
    const onclose = vi.fn()
    const { container } = render(SettingsPanel, { onclose })
    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(onclose).toHaveBeenCalledTimes(1)
    await fireEvent.click(container.querySelector('.backdrop')!)
    expect(onclose).toHaveBeenCalledTimes(2)
  })
})
