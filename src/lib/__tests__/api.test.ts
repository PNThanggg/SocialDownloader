import { beforeEach, describe, expect, it, vi } from 'vitest'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }))

import { listen } from '@tauri-apps/api/event'
import { detectSiteLocal, download, installTool, isLumiError, onJob, onToolProgress, resolve } from '../api'

describe('detectSiteLocal', () => {
  it('mirrors the Rust detect_site variants', () => {
    const cases: [string, string][] = [
      ['https://vt.tiktok.com/ZSabc/', 'tiktok'],
      ['https://www.tiktok.com/@a/video/1?is_from_webapp=1', 'tiktok'],
      ['https://www.tiktok.com/@a/photo/7', 'tiktok'],
      ['https://www.instagram.com/reel/C8x/?igsh=abc', 'instagram'],
      ['https://instagram.com/p/C8x/', 'instagram'],
      ['https://youtu.be/dQw4w9WgXcQ?si=x', 'youtube'],
      ['https://m.youtube.com/shorts/abc', 'youtube'],
      ['https://fb.watch/abc/', 'facebook'],
      ['https://m.facebook.com/watch/?v=1', 'facebook'],
      ['https://web.facebook.com/share/r/abc/', 'facebook'],
      ['https://vimeo.com/1', 'other'],
    ]
    for (const [u, s] of cases) expect(detectSiteLocal(u), u).toBe(s)
    expect(detectSiteLocal('not a url')).toBeNull()
    expect(detectSiteLocal('  https://youtu.be/x  ')).toBe('youtube')
  })

  it('rejects lookalikes and non-http', () => {
    expect(detectSiteLocal('https://nottiktok.com/x')).toBe('other')
    expect(detectSiteLocal('https://tiktok.com.evil.io/x')).toBe('other')
    expect(detectSiteLocal('ftp://www.tiktok.com/x')).toBeNull()
    expect(detectSiteLocal('file:///C:/x.mp4')).toBeNull()
  })
})

describe('invoke wrappers', () => {
  beforeEach(() => {
    invokeMock.mockReset()
  })

  it('passes camelCase args through', async () => {
    invokeMock.mockResolvedValue('job1')
    await download('u', 'opt', 'D:\\x.mp4')
    expect(invokeMock).toHaveBeenCalledWith('download', { url: 'u', optionId: 'opt', dest: 'D:\\x.mp4' })
  })

  it('passes a LumiError rejection through', async () => {
    const err = { error: { code: 'network' }, detail: 'boom' }
    invokeMock.mockRejectedValue(err)
    await expect(resolve('u')).rejects.toEqual(err)
  })

  it('normalizes other rejections to unknown', async () => {
    invokeMock.mockRejectedValue('plain string')
    const e = await installTool('ffmpeg').catch((x: unknown) => x)
    expect(isLumiError(e)).toBe(true)
    expect(e).toEqual({ error: { code: 'unknown' }, detail: 'plain string' })
  })

  it('isLumiError rejects junk', () => {
    expect(isLumiError(null)).toBe(false)
    expect(isLumiError({ error: 'x' })).toBe(false)
    expect(isLumiError(new Error('x'))).toBe(false)
  })
})

describe('events', () => {
  it('subscribes to the lumi:// channels and unwraps payloads', async () => {
    const cb = vi.fn()
    await onJob(cb)
    await onToolProgress(cb)
    const calls = vi.mocked(listen).mock.calls
    expect(calls.map((c) => c[0])).toEqual(['lumi://job', 'lumi://tool-progress'])
    ;(calls[0][1] as (e: { payload: unknown }) => void)({ payload: { jobId: 'j' } })
    expect(cb).toHaveBeenCalledWith({ jobId: 'j' })
  })
})
