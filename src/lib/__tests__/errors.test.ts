import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/plugin-store', () => ({ load: vi.fn() }))

import { errorMessage } from '../i18n'
import type { ErrorCode } from '../types'

describe('errorMessage', () => {
  it('maps every code to its err.* key', () => {
    const codes: ErrorCode[] = [
      { code: 'unsupported_url' },
      { code: 'login_required', site: 'instagram' },
      { code: 'private_or_removed' },
      { code: 'extractor_broken' },
      { code: 'network' },
      { code: 'rate_limited' },
      { code: 'youtube_bot_check' },
      { code: 'playlist_not_supported' },
      { code: 'tool_missing', tool: 'ffmpeg' },
      { code: 'already_running' },
      { code: 'unknown' },
    ]
    for (const c of codes) expect(errorMessage(c, false).key).toBe(`err.${c.code}`)
  })

  it('maps actions', () => {
    const login: ErrorCode = { code: 'login_required', site: 'instagram' }
    expect(errorMessage(login, false).action).toBe('connect')
    expect(errorMessage(login, true).action).toBe('reconnect')
    expect(errorMessage({ code: 'extractor_broken' }, false).action).toBe('update')
    expect(errorMessage({ code: 'network' }, false).action).toBe('retry')
    expect(errorMessage({ code: 'unknown' }, false).action).toBe('details')
    expect(errorMessage({ code: 'tool_missing', tool: 'deno' }, false).action).toBe('reinstall')
  })

  it('names a download in the vague message', () => {
    expect(errorMessage({ code: 'unknown' }, false, 'download')).toEqual({ key: 'err.unknown_download', action: 'details' })
    expect(errorMessage({ code: 'unknown' }, false, 'resolve').key).toBe('err.unknown')
    expect(errorMessage({ code: 'network' }, false, 'download').key).toBe('err.network')
  })

  it('offers Connect only for sites with an in-app login', () => {
    expect(errorMessage({ code: 'login_required', site: 'facebook' }, false).action).toBe('connect')
    for (const site of ['tiktok', 'youtube', 'other'] as const) {
      expect(errorMessage({ code: 'login_required', site }, false).action, site).toBeUndefined()
    }
  })

  it('has no action for the rest', () => {
    for (const code of [
      'unsupported_url',
      'private_or_removed',
      'rate_limited',
      'youtube_bot_check',
      'playlist_not_supported',
      'already_running',
    ] as const) {
      expect(errorMessage({ code }, false).action).toBeUndefined()
    }
  })
})
