const MB = 1024 * 1024

/** `8.4 MB` (one decimal, MB even below 1 MB), `1.5 GB` from 1024 MB; null when unknown. */
export function formatSize(bytes: number | null): string | null {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return null
  const mb = bytes / MB
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb.toFixed(1)} MB`
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
