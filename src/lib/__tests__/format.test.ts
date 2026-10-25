import { describe, expect, it } from 'vitest'
import { formatDuration, formatSize } from '../format'

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
