const MB = 1024 * 1024

/** `8.4 MB` (one decimal, MB even below 1 MB), `1.5 GB` from 1024 MB; null when unknown. */
export function formatSize(bytes: number | null): string | null {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return null
  const mb = bytes / MB
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb.toFixed(1)} MB`
}

const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/** `5 minutes ago` / `yesterday` within a week, else the date (`26/10/2026`), in `locale`. */
export function formatWhen(at: number, now: number, locale: string): string {
  const ago = Math.max(0, now - at)
  if (ago >= 7 * DAY) return new Date(at).toLocaleDateString(locale)
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
  if (ago < HOUR) return rtf.format(-Math.floor(ago / MINUTE), 'minute')
  if (ago < DAY) return rtf.format(-Math.floor(ago / HOUR), 'hour')
  return rtf.format(-Math.floor(ago / DAY), 'day')
}

/** `m:ss`, or `h:mm:ss` from one hour; null when missing. */
export function formatDuration(secs: number | null): string | null {
  if (secs === null || !Number.isFinite(secs) || secs < 0) return null
  const total = Math.round(secs)
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = String(total % 60).padStart(2, '0')
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`
}
