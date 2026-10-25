import { get, writable } from 'svelte/store'
import * as api from './api'
import type { Tool, ToolProgress, ToolStatus } from './types'

/** Tools Lumi cannot read any link without; the setup screen installs them. */
export const CORE_TOOLS: readonly Tool[] = ['yt_dlp', 'gallery_dl']

/** Last known status of every tool; null until the first `toolsStatus` answer. */
export const toolStatuses = writable<ToolStatus[] | null>(null)

/** true/false from the last known status; null when it is not known yet. */
export function isInstalled(list: ToolStatus[] | null, tool: Tool): boolean | null {
  return list?.find((s) => s.tool === tool)?.installed ?? null
}

function remember(status: ToolStatus) {
  toolStatuses.update((list) =>
    list?.some((s) => s.tool === status.tool) ? list.map((s) => (s.tool === status.tool ? status : s)) : [...(list ?? []), status],
  )
}

/** Share of the download done, or null while the total size is unknown. */
export function fractionOf(p: Pick<ToolProgress, 'downloaded' | 'total'>): number | null {
  return p.total ? Math.min(1, p.downloaded / p.total) : null
}

/** Installs `tool`, reporting its download progress; the store learns the new status. */
export async function installWithProgress(tool: Tool, onprogress?: (p: ToolProgress) => void): Promise<ToolStatus> {
  const off = await api
    .onToolProgress((p) => {
      if (p.tool === tool) onprogress?.(p)
    })
    .catch(() => null)
  try {
    const status = await api.installTool(tool)
    remember(status)
    return status
  } finally {
    off?.()
  }
}

/** One "Lumi needs to download X" question; `settle` closes it (null = Later). */
export interface ToolRequest {
  tool: Tool
  /** Skip the question and start installing right away (the user already asked for it). */
  autostart: boolean
  promise: Promise<ToolStatus | null>
  settle: (status: ToolStatus | null) => void
}

/** Open questions, oldest first; App shows the first one as a ToolPrompt. */
export const toolRequests = writable<ToolRequest[]>([])

/**
 * Asks the user to install `tool`. Resolves with its status once installed, or null on Later.
 * Asking again for a tool that is already being asked about joins that question.
 */
export function askForTool(tool: Tool, autostart = false): Promise<ToolStatus | null> {
  const open = get(toolRequests).find((r) => r.tool === tool)
  if (open) return open.promise
  let done!: (status: ToolStatus | null) => void
  const promise = new Promise<ToolStatus | null>((resolve) => (done = resolve))
  const req: ToolRequest = {
    tool,
    autostart,
    promise,
    settle: (status) => {
      toolRequests.update((q) => q.filter((r) => r !== req))
      done(status)
    },
  }
  toolRequests.update((q) => [...q, req])
  return promise
}
