import { describe, expect, it } from 'vitest'
import { formatDuration, formatSize, formatWhen } from '../format'

describe('formatWhen', () => {
  const now = Date.UTC(2026, 9, 26, 12, 0)
  const min = 60_000
  it('is relative within a week', () => {
    expect(formatWhen(now - 10_000, now, 'en')).toBe('this minute')
    expect(formatWhen(now - 5 * min, now, 'en')).toBe('5 minutes ago')
    expect(formatWhen(now - 3 * 60 * min, now, 'en')).toBe('3 hours ago')
    expect(formatWhen(now - 26 * 60 * min, now, 'en')).toBe('yesterday')
    expect(formatWhen(now - 5 * min, now, 'vi')).toBe('5 phút trước')
  })
  it('is a date from a week back, and never in the future', () => {
    const old = now - 8 * 24 * 60 * min
    expect(formatWhen(old, now, 'en')).toBe(new Date(old).toLocaleDateString('en'))
    expect(formatWhen(now + 5 * min, now, 'en')).toBe('this minute')
  })
})

describe('formatSize', () => {
  it('uses MB with one decimal', () => {
    expect(formatSize(8.4 * 1024 * 1024)).toBe('8.4 MB')
    expect(formatSize(1024 * 1024)).toBe('1.0 MB')
    expect(formatSize(400 * 1024)).toBe('0.4 MB')
  })
  it('switches to GB from 1024 MB', () => {
    expect(formatSize(1.5 * 1024 * 1024 * 1024)).toBe('1.5 GB')
  })
  it('omits unknown sizes', () => {
    expect(formatSize(null)).toBeNull()
  })
})

describe('formatDuration', () => {
  it('formats m:ss', () => {
    expect(formatDuration(48)).toBe('0:48')
    expect(formatDuration(65.4)).toBe('1:05')
    expect(formatDuration(3599)).toBe('59:59')
  })
  it('formats h:mm:ss over an hour', () => {
    expect(formatDuration(3600)).toBe('1:00:00')
    expect(formatDuration(3725)).toBe('1:02:05')
  })
  it('omits missing or invalid durations', () => {
    expect(formatDuration(null)).toBeNull()
    expect(formatDuration(-1)).toBeNull()
    expect(formatDuration(Number.NaN)).toBeNull()
  })
})
