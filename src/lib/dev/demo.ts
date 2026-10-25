// Dev-only fixtures for looking at the UI without network. Imported only under
// `import.meta.env.DEV` in App.svelte, so production builds drop this file.
import { emit } from '@tauri-apps/api/event'
import type { DemoBackend } from '../api'
import { language } from '../i18n'
import { askForTool } from '../tools'
import type { AccountStatus, LumiError, MediaInfo, Tool, ToolStatus } from '../types'

export interface Demo {
  url: string
  info?: MediaInfo
  error?: LumiError
  busy?: boolean
  /** Open the settings sheet. */
  settings?: boolean
  /** Run the real resolve flow on `url` once the app is ready. */
  resolve?: boolean
  /** Runs shortly after the screen is up (e.g. to raise a tool prompt). */
  after?: () => void
}

const MB = 1024 * 1024

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const tool = (t: Tool, version: string | null): ToolStatus => ({ tool: t, installed: version !== null, version })
const VERSIONS: Record<Tool, string> = {
  yt_dlp: '2025.10.22',
  gallery_dl: '1.30.10',
  ffmpeg: 'N-121583-g4348bde2d2-20251024',
  deno: 'deno 2.5.4 (stable, release, x86_64-pc-windows-msvc)',
}
/** Every tool installed except `missing`. */
const tools = (...missing: Tool[]) =>
  (Object.keys(VERSIONS) as Tool[]).map((t) => tool(t, missing.includes(t) ? null : VERSIONS[t]))

const network: LumiError = { error: { code: 'network' }, detail: 'error sending request: dns error' }

/** A fake install that reports progress through the real event bus for `secs`, or fails partway. */
function install(plan: Partial<Record<Tool, { size: number | null; secs: number; fail?: LumiError }>>) {
  return async (args?: Record<string, unknown>) => {
    const t = args?.tool as Tool
    const { size, secs, fail } = plan[t] ?? { size: 20 * MB, secs: 3 }
    const steps = Math.max(1, Math.round(secs * 5))
    for (let i = 1; i <= steps; i++) {
      await sleep(200)
      if (fail && i >= steps / 3) throw fail
      const downloaded = Math.round((size ?? 24 * MB) * (i / steps))
      await emit('lumi://tool-progress', { tool: t, downloaded, total: size })
    }
    return tool(t, VERSIONS[t])
  }
}

/** Answers these commands in place of the backend; everything else still reaches Rust. */
function backend(handlers: Record<string, (args?: Record<string, unknown>) => unknown>) {
  const fake: DemoBackend = (cmd, args) =>
    cmd in handlers ? Promise.resolve().then(() => handlers[cmd](args)) : undefined
  ;(globalThis as { __lumiDemoBackend?: DemoBackend }).__lumiDemoBackend = fake
}

const accounts = (instagram: boolean, facebook: boolean): AccountStatus[] => [
  { site: 'instagram', connected: instagram },
  { site: 'facebook', connected: facebook },
]

const tiktok: MediaInfo = {
  sourceUrl: 'https://www.tiktok.com/@lumi.music/video/7381234567890123456',
  site: 'tiktok',
  engine: 'yt_dlp',
  title: 'Nhạc chill buổi tối 🎧 nghe là nghiện #lofi #chill',
  author: '@lumi.music',
  durationSecs: 48,
  thumbnail: null,
  limitedFormats: false,
  items: [
    {
      index: 0,
      thumbnail: null,
      isVideo: true,
      options: [
        { id: '0/video:1920', kind: 'video', label: '1080p', ext: 'mp4', filesize: 8.4 * MB, directUrl: 'https://v16.tiktokcdn.com/a.mp4', note: null },
        { id: '0/video:1280', kind: 'video', label: '720p', ext: 'mp4', filesize: 4.6 * MB, directUrl: 'https://v16.tiktokcdn.com/b.mp4', note: null },
        { id: '0/video:854', kind: 'video', label: '480p', ext: 'mp4', filesize: 2.2 * MB, directUrl: 'https://v16.tiktokcdn.com/c.mp4', note: null },
        { id: '0/audio', kind: 'audio', label: 'MP3', ext: 'mp3', filesize: 1.1 * MB, directUrl: 'https://v16.tiktokcdn.com/a.mp4', note: { kind: 'source_is_video' } },
        { id: 'sound', kind: 'original_sound', label: 'Tối nay em đi đâu — Lumi Lofi', ext: 'mp3', filesize: null, directUrl: 'https://sf16.tiktokcdn.com/s.mp3', note: null },
      ],
    },
  ],
}

const instagram: MediaInfo = {
  sourceUrl: 'https://www.instagram.com/p/C8xAbCdEfGh/',
  site: 'instagram',
  engine: 'gallery_dl',
  title: 'Đà Lạt mùa sương — weekend dump',
  author: '@lumi.travel',
  durationSecs: null,
  thumbnail: null,
  limitedFormats: false,
  items: [0, 1, 2, 3, 4].map((i) =>
    i === 3
      ? {
          index: i,
          thumbnail: null,
          isVideo: true,
          options: [
            { id: `${i}/video:1920`, kind: 'video', label: '1080p', ext: 'mp4', filesize: null, directUrl: 'https://scontent.cdninstagram.com/v.mp4', note: null },
            { id: `${i}/audio`, kind: 'audio', label: 'MP3', ext: 'mp3', filesize: null, directUrl: 'https://scontent.cdninstagram.com/v.mp4', note: { kind: 'source_is_video' } },
          ],
        }
      : {
          index: i,
          thumbnail: null,
          isVideo: false,
          options: [{ id: `${i}/image`, kind: 'image', label: 'JPG', ext: 'jpg', filesize: null, directUrl: `https://scontent.cdninstagram.com/${i}.jpg`, note: null }],
        },
  ),
}

const youtube: MediaInfo = {
  sourceUrl: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
  site: 'youtube',
  engine: 'yt_dlp',
  title: 'Lo-fi hip hop radio — beats to relax/study to (1 hour mix)',
  author: 'Lumi Records',
  durationSecs: 3725,
  thumbnail: null,
  limitedFormats: true,
  items: [
    {
      index: 0,
      thumbnail: null,
      isVideo: true,
      options: [
        ...[1080, 720, 480, 360].map((h, i) => ({
          id: `0/video:${h}`,
          kind: 'video' as const,
          label: `${h}p`,
          ext: 'mp4',
          filesize: [612, 318, 166, 98][i] * MB,
          directUrl: 'https://rr1---sn.googlevideo.com/videoplayback',
          note: h > 360 ? ({ kind: 'no_audio' } as const) : null,
        })),
        { id: '0/audio', kind: 'audio', label: 'MP3', ext: 'mp3', filesize: 58.3 * MB, directUrl: 'https://rr1---sn.googlevideo.com/a', note: { kind: 'source_audio', ext: 'm4a' } },
      ],
    },
  ],
}

/** `name` or `name:vi` / `name:en` to also force the UI language. */
export function demo(spec: string): Demo {
  const [name, lang] = spec.split(':')
  if (lang === 'vi' || lang === 'en') language.set(lang)
  switch (name) {
    case 'setup':
      // yt-dlp finishes quickly, gallery-dl stays in progress long enough to look at.
      backend({
        tools_status: () => tools('yt_dlp', 'gallery_dl'),
        install_tool: install({ yt_dlp: { size: 18.4 * MB, secs: 2 }, gallery_dl: { size: 13.1 * MB, secs: 90 } }),
      })
      return { url: '' }
    case 'setup-error':
      backend({
        tools_status: () => tools('yt_dlp', 'gallery_dl'),
        install_tool: install({ yt_dlp: { size: 18.4 * MB, secs: 1 }, gallery_dl: { size: 13.1 * MB, secs: 3, fail: network } }),
      })
      return { url: '' }
    case 'settings':
    case 'settings-busy':
      backend({
        tools_status: () => tools('ffmpeg'),
        accounts_status: () => accounts(true, false),
        connect_account: () => new Promise(() => {}),
        update_tools: async () => (await sleep(4000), tools('ffmpeg')),
      })
      return {
        url: tiktok.sourceUrl,
        info: tiktok,
        settings: true,
        after:
          name === 'settings-busy'
            ? () => document.querySelector<HTMLButtonElement>('[data-site="facebook"] .btn-primary')?.click()
            : undefined,
      }
    case 'prompt':
      // The real flow: a YouTube link while deno is missing asks first.
      backend({
        tools_status: () => tools('deno'),
        tool_download_size: () => 41.7 * MB,
        install_tool: install({ deno: { size: 41.7 * MB, secs: 60 } }),
        resolve: async () => (await sleep(600), youtube),
      })
      return { url: youtube.sourceUrl, resolve: true }
    case 'limited':
      // A YouTube result read without deno ("Later"): the chip offers to install it.
      backend({ tools_status: () => tools('deno') })
      return { url: youtube.sourceUrl, info: youtube }
    case 'prompt-ffmpeg':
    case 'prompt-nosize':
    case 'prompt-install':
    case 'prompt-error':
      backend({
        tools_status: () => tools('ffmpeg'),
        tool_download_size: () => (name === 'prompt-nosize' ? null : 86.4 * MB),
        install_tool: install({ ffmpeg: { size: 86.4 * MB, secs: name === 'prompt-error' ? 1 : 90, fail: name === 'prompt-error' ? network : undefined } }),
      })
      return {
        url: tiktok.sourceUrl,
        info: tiktok,
        after: () => void askForTool('ffmpeg', name === 'prompt-install' || name === 'prompt-error'),
      }
    case 'instagram':
      return { url: instagram.sourceUrl, info: instagram }
    case 'youtube':
      return { url: youtube.sourceUrl, info: youtube }
    case 'loading':
      return { url: tiktok.sourceUrl, busy: true }
    case 'error':
      return {
        url: 'https://www.instagram.com/p/C8xPrivate/',
        error: { error: { code: 'login_required', site: 'instagram' }, detail: 'ERROR: [Instagram] login required' },
      }
    case 'unknown':
      return {
        url: 'https://www.facebook.com/share/r/abc/',
        error: { error: { code: 'unknown' }, detail: 'Traceback (most recent call last):\n  File "yt_dlp/__main__.py", line 17\nKeyError: \'video\'' },
      }
    default:
      return { url: tiktok.sourceUrl, info: tiktok }
  }
}
