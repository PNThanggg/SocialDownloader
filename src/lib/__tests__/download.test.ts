import { describe, expect, it, vi } from 'vitest'
import { JobFollower, joinPath, parentDir, pickDest, startDownload, startDownloadAll, type DownloadDeps } from '../download'
import type { JobEvent, MediaInfo, MediaOption } from '../types'

const mp3: MediaOption = {
  id: '0/audio',
  kind: 'audio',
  label: 'MP3',
  ext: 'mp3',
  filesize: null,
  directUrl: null,
  note: null,
}

const info: MediaInfo = {
  sourceUrl: 'https://www.tiktok.com/@a/video/1',
  site: 'tiktok',
  engine: 'yt_dlp',
  title: 'Song',
  author: '@a',
  durationSecs: 48,
  thumbnail: null,
  items: [{ index: 0, thumbnail: null, isVideo: true, options: [mp3] }],
  limitedFormats: false,
}

function deps(over: { lastDir?: string; saveResult?: string | null; openResult?: string | null } = {}) {
  const kv = new Map<string, unknown>()
  if (over.lastDir !== undefined) kv.set('lastDir', over.lastDir)
  const store = {
    get: vi.fn(async (k: string) => kv.get(k)),
    set: vi.fn(async (k: string, v: unknown) => void kv.set(k, v)),
    save: vi.fn(async () => {}),
  }
  const api = {
    download: vi.fn(async () => 'job-1'),
    downloadAll: vi.fn(async () => 'job-all'),
    defaultFilename: vi.fn(async (title: string, ext: string) => `${title}.${ext}`),
  }
  const d = {
    save: vi.fn(async () => (over.saveResult === undefined ? 'D:\\Music\\Song.mp3' : over.saveResult)),
    open: vi.fn(async () => (over.openResult === undefined ? 'E:\\Pics' : over.openResult)),
    store,
    api,
    downloadDir: vi.fn(async () => 'C:\\Users\\me\\Downloads\\'),
  }
  return { d: d as unknown as DownloadDeps, store, api, save: d.save, open: d.open }
}

describe('startDownload', () => {
  it('cancelled dialog returns null and does not call api.download', async () => {
    const { d, api, store } = deps({ saveResult: null })
    expect(await startDownload(d, info, mp3)).toBeNull()
    expect(api.download).not.toHaveBeenCalled()
    expect(store.set).not.toHaveBeenCalled()
  })

  it('uses lastDir and defaultFilename for defaultPath', async () => {
    const { d, save, api } = deps({ lastDir: 'D:\\Music' })
    await startDownload(d, info, mp3)
    expect(api.defaultFilename).toHaveBeenCalledWith('Song', 'mp3')
    expect(save).toHaveBeenCalledWith({
      defaultPath: 'D:\\Music\\Song.mp3',
      filters: [{ name: 'MP3', extensions: ['mp3'] }],
    })
  })

  it('saves new lastDir after choosing', async () => {
    const { d, store, api } = deps({ saveResult: 'E:\\x\\a.mp3' })
    expect(await startDownload(d, info, mp3)).toBe('job-1')
    expect(store.set).toHaveBeenCalledWith('lastDir', 'E:\\x')
    expect(store.save).toHaveBeenCalled()
    expect(api.download).toHaveBeenCalledWith(info.sourceUrl, '0/audio', 'E:\\x\\a.mp3')
  })

  it('falls back to the Downloads folder without doubling its trailing separator', async () => {
    const { d, save } = deps()
    await pickDest(d, info, mp3)
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ defaultPath: 'C:\\Users\\me\\Downloads\\Song.mp3' }))
  })

  it('names the original sound after its track', async () => {
    const { d, api } = deps({ lastDir: 'D:\\Music' })
    const sound: MediaOption = { ...mp3, id: 'sound', kind: 'original_sound', label: 'Track — Artist' }
    await pickDest(d, info, sound)
    expect(api.defaultFilename).toHaveBeenCalledWith('Track — Artist', 'mp3')
  })

  it('numbers items of a multi-item post', async () => {
    const { d, api, save } = deps({ lastDir: 'D:\\Pics' })
    const img = (i: number): MediaOption => ({ ...mp3, id: `${i}/image`, kind: 'image', label: 'JPG', ext: 'jpg' })
    const multi: MediaInfo = {
      ...info,
      items: [0, 1].map((i) => ({ index: i, thumbnail: null, isVideo: false, options: [img(i)] })),
    }
    await pickDest(d, multi, img(1))
    expect(api.defaultFilename).toHaveBeenCalledWith('Song 2', 'jpg')
    expect(save).toHaveBeenCalledWith({ defaultPath: 'D:\\Pics\\Song 2.jpg', filters: [{ name: 'JPG', extensions: ['jpg'] }] })
  })
})

describe('startDownloadAll', () => {
  it('downloadAll uses directory picker', async () => {
    const { d, open, save, api, store } = deps({ lastDir: 'D:\\Music', openResult: 'E:\\Pics' })
    expect(await startDownloadAll(d, info)).toBe('job-all')
    expect(open).toHaveBeenCalledWith({ directory: true, defaultPath: 'D:\\Music' })
    expect(save).not.toHaveBeenCalled()
    expect(api.downloadAll).toHaveBeenCalledWith(info.sourceUrl, 'E:\\Pics')
    expect(store.set).toHaveBeenCalledWith('lastDir', 'E:\\Pics')
  })

  it('returns null when the folder picker is cancelled', async () => {
    const { d, api } = deps({ openResult: null })
    expect(await startDownloadAll(d, info)).toBeNull()
    expect(api.downloadAll).not.toHaveBeenCalled()
  })
})

describe('path helpers', () => {
  it('joins with one backslash', () => {
    expect(joinPath('D:\\Music', 'a.mp3')).toBe('D:\\Music\\a.mp3')
    expect(joinPath('D:\\', 'a.mp3')).toBe('D:\\a.mp3')
    expect(joinPath('D:\\Music/', 'a.mp3')).toBe('D:\\Music/a.mp3')
  })

  it('takes the parent folder, keeping a drive root', () => {
    expect(parentDir('E:\\x\\a.mp3')).toBe('E:\\x')
    expect(parentDir('E:\\a.mp3')).toBe('E:\\')
    expect(parentDir('a.mp3')).toBeNull()
  })
})

describe('JobFollower', () => {
  const ev = (jobId: string, fraction: number): JobEvent => ({ jobId, url: 'u', optionId: 'o', state: 'progress', fraction })

  it('replays events that arrived before the job id was known', () => {
    const seen: JobEvent[] = []
    const f = new JobFollower((e) => seen.push(e))
    f.push(ev('a', 0.1)) // not armed: dropped
    f.arm()
    f.push(ev('b', 0.2))
    f.push(ev('c', 0.3))
    f.follow('b')
    f.push(ev('c', 0.4))
    f.push(ev('b', 0.5))
    expect(seen.map((e) => e.jobId === 'b' && e.state === 'progress' && e.fraction)).toEqual([0.2, 0.5])
  })
})
