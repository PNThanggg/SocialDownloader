<script lang="ts">
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount } from 'svelte'
  import { slide } from 'svelte/transition'
  import * as api from '../api'
  import { JobFollower, pickDir, realDeps, viewOf, type JobView } from '../download'
  import { formatDuration } from '../format'
  import { t, type ErrorAction, type MessageKey } from '../i18n'
  import { askForTool } from '../tools'
  import type { AccountStatus, LumiError, MediaInfo, MediaOption, OptionKind } from '../types'
  import ErrorBanner from './ErrorBanner.svelte'
  import ImageGrid from './ImageGrid.svelte'
  import OptionRow from './OptionRow.svelte'

  let {
    info = null,
    loading = false,
    accounts = [],
    onfix,
    oninstalldeno,
  }: {
    info?: MediaInfo | null
    /** Shows the skeleton instead of `info`. */
    loading?: boolean
    accounts?: AccountStatus[]
    onfix?: (action: ErrorAction, error: LumiError) => Promise<boolean>
    /** Shown next to "Not all qualities available" when deno can still be installed. */
    oninstalldeno?: () => unknown
  } = $props()

  let installingDeno = $state(false)
  async function installDeno() {
    installingDeno = true
    try {
      await oninstalldeno?.()
    } finally {
      installingDeno = false
    }
  }

  let thumb: string | null = $state(null)
  let selected = $state(0)

  $effect(() => {
    const url = info?.thumbnail
    const site = info?.site
    thumb = null
    if (!url || !site) return
    let live = true
    api.fetchThumbnail(url, site).then(
      (src) => live && (thumb = src),
      () => {},
    )
    return () => {
      live = false
    }
  })

  const RANK: Record<OptionKind, number> = { video: 0, audio: 1, original_sound: 2, image: 3 }
  /** Quality from the label (`1080p`), falling back to the height in the id. */
  const quality = (o: MediaOption) => Number.parseInt(o.label, 10) || Number(/video:(\d+)/.exec(o.id)?.[1] ?? 0)
  /** Videos (best first), the video's MP3, the original sound right under it, then images. */
  const ordered = (opts: MediaOption[]) =>
    [...opts].sort((a, b) => RANK[a.kind] - RANK[b.kind] || quality(b) - quality(a))

  const multi = $derived((info?.items.length ?? 0) > 1)
  /** The item whose options are listed (the first one until a grid cell is clicked). */
  const shown = $derived(info?.items.find((i) => i.index === selected)?.index ?? info?.items[0]?.index ?? 0)
  const meta = $derived(
    info ? [info.author, formatDuration(info.durationSecs)].filter(Boolean).join(' · ') : '',
  )

  // "Download all" job.
  let all = $state<JobView>({ s: 'idle' })
  /** The folder chosen for "Download all"; kept so a retry or the ffmpeg install reuses it. */
  let allDir: string | null = null
  let cancelling = $state(false)
  let notice = $state<MessageKey | null>(null)
  let noticeTimer: ReturnType<typeof setTimeout> | undefined

  const needsFfmpeg = (e: LumiError) => e.error.code === 'tool_missing' && e.error.tool === 'ffmpeg'

  const follower = new JobFollower((e) => {
    if (e.state === 'done' || e.state === 'failed' || e.state === 'cancelled') {
      follower.stop()
      cancelling = false
    }
    if (e.state === 'failed' && needsFfmpeg(e.error)) void offerFfmpeg()
    else all = viewOf(e)
  })
  let subscribed: Promise<unknown> = Promise.resolve()
  onMount(() => {
    const unlisten = api.onJob((e) => follower.push(e))
    subscribed = unlisten
    return () => {
      follower.stop()
      clearTimeout(noticeTimer)
      void unlisten.then((off) => off()).catch(() => {})
    }
  })

  function flash(key: MessageKey) {
    notice = key
    clearTimeout(noticeTimer)
    noticeTimer = setTimeout(() => (notice = null), 3500)
  }

  async function downloadAll() {
    if (!info) return
    all = { s: 'busy' }
    try {
      const dir = await pickDir(await realDeps())
      if (dir === null) {
        all = { s: 'idle' }
        return
      }
      allDir = dir
      await runAll()
    } catch (e) {
      all = { s: 'error', error: api.normalize(e) }
    }
  }

  /** Starts "Download all" into the kept folder. */
  async function runAll() {
    if (!info || allDir === null) return
    await subscribed
    follower.arm()
    all = { s: 'running', fraction: null }
    try {
      follower.follow(await api.downloadAll(info.sourceUrl, allDir))
    } catch (e) {
      follower.stop()
      const error = api.normalize(e)
      if (needsFfmpeg(error)) await offerFfmpeg()
      else all = { s: 'error', error }
    }
  }

  async function offerFfmpeg() {
    all = { s: 'busy' }
    if (await askForTool('ffmpeg')) await runAll()
    else all = { s: 'idle' }
  }

  async function cancelAll() {
    const id = follower.jobId
    if (!id) return
    cancelling = true
    try {
      await api.cancelJob(id)
    } catch {
      cancelling = false
      flash('notice.cancelFailed')
    }
  }

  async function revealAll(path: string) {
    try {
      await revealItemInDir(path)
    } catch {
      flash('notice.revealFailed')
    }
  }

  async function onAllError(action: ErrorAction) {
    if (all.s !== 'error') return
    const error = all.error
    try {
      if (action === 'retry' || (await onfix?.(action, error))) await (allDir === null ? downloadAll() : runAll())
    } catch (e) {
      all = { s: 'error', error: api.normalize(e) }
    }
  }

  const allPct = $derived(all.s === 'running' && all.fraction !== null ? Math.round(all.fraction * 100) : null)
</script>

{#if loading}
  <div class="card glass skeleton" aria-busy="true">
    <div class="head">
      <div class="thumb shimmer"></div>
      <div class="text">
        <div class="line shimmer" style:width="78%"></div>
        <div class="line shimmer" style:width="52%"></div>
        <div class="line short shimmer" style:width="30%"></div>
      </div>
    </div>
    <div class="rows">
      {#each [0, 1, 2] as i (i)}
        <div class="row-skel shimmer" style:animation-delay="{i * 0.12}s"></div>
      {/each}
    </div>
  </div>
{:else if info}
  <article class="card glass">
    <header class="head">
      <div class="thumb" class:fallback={!thumb}>
        {#if thumb}
          <img src={thumb} alt="" draggable="false" />
        {:else}
          <svg viewBox="0 0 24 24" aria-hidden="true"
            ><path d="M9 18V6l10-2v12" /><circle cx="6.5" cy="18" r="2.5" /><circle cx="16.5" cy="16" r="2.5" /></svg
          >
        {/if}
      </div>
      <div class="text">
        <h2 title={info.title}>{info.title || $t('label.untitled')}</h2>
        {#if meta}<p class="meta">{meta}</p>{/if}
        {#if info.limitedFormats}
          <div class="limited">
            <span class="chip">
              <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 8v.01" /></svg>
              {$t('note.limited')}
            </span>
            {#if oninstalldeno}
              <button type="button" class="chip-btn" disabled={installingDeno} onclick={installDeno}>
                {#if installingDeno}<span class="spinner"></span>{/if}
                {$t('btn.getAllQualities')}
              </button>
            {/if}
          </div>
        {/if}
      </div>
    </header>

    {#if multi}
      <div class="multi-bar">
        <span class="count">{$t('label.items', { n: String(info.items.length) })}</span>
        <div class="all">
          {#if all.s === 'running' || all.s === 'processing'}
            <div class="bar" class:indeterminate={allPct === null} role="progressbar" aria-label={$t('action.downloadAll')} aria-valuemin={0} aria-valuemax={100} aria-valuenow={allPct ?? undefined}>
              <div class="fill" style:width={allPct === null ? undefined : `${allPct}%`}></div>
            </div>
            <span class="pct">{allPct === null ? '' : `${allPct}%`}</span>
            <button type="button" class="btn btn-ghost" disabled={cancelling} onclick={cancelAll}>
              {$t('action.cancel')}
            </button>
          {:else if all.s === 'done'}
            {@const path = all.path}
            <button type="button" class="btn btn-ghost" onclick={() => revealAll(path)}>
              <svg viewBox="0 0 24 24" aria-hidden="true"
                ><path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" /></svg
              >
              {$t('action.openFolder')}
            </button>
          {/if}
          {#if all.s !== 'running' && all.s !== 'processing'}
            <button type="button" class="btn btn-primary" disabled={all.s === 'busy'} onclick={downloadAll}>
              {#if all.s === 'busy'}
                <span class="spinner"></span>
              {:else}
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4.5v10.5M7 10.5l5 5 5-5M5 19.5h14" /></svg>
              {/if}
              {$t('action.downloadAll')}
            </button>
          {/if}
        </div>
      </div>
      {#if all.s === 'error'}
        <ErrorBanner error={all.error} compact onaction={onAllError} />
      {/if}
      {#if notice}
        <p class="notice" role="status" transition:slide={{ duration: 160 }}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 7.5v5.5M12 16.5v.01" /></svg>
          {$t(notice)}
        </p>
      {/if}
      <ImageGrid items={info.items} site={info.site} selected={shown} onselect={(i) => (selected = i)} />
    {/if}

    <!-- Every item's rows stay mounted (hidden when not selected) so a running download keeps its progress. -->
    {#each info.items as item (item.index)}
      <div class="rows" hidden={item.index !== shown}>
        {#each ordered(item.options) as o (o.id)}
          <OptionRow {info} opt={o} {accounts} {onfix} />
        {/each}
      </div>
    {/each}
  </article>
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px;
    border-radius: var(--radius-lg);
    box-shadow: 0 10px 40px -18px rgba(0, 0, 0, 0.6);
  }
  .head {
    display: flex;
    gap: 14px;
    align-items: center;
  }
  .thumb {
    width: 64px;
    height: 84px;
    flex-shrink: 0;
    border-radius: 10px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.06);
  }
  .thumb.fallback {
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, #4dabf7, #e64980);
  }
  .thumb svg {
    width: 26px;
    height: 26px;
    fill: none;
    stroke: rgba(255, 255, 255, 0.9);
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    line-height: 1.35;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .meta {
    margin: 0;
    font-size: 12.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    width: fit-content;
    margin-top: 2px;
    padding: 2px 8px;
    border-radius: 20px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid var(--glass-border-soft);
    font-size: 11.5px;
    color: var(--muted);
  }
  .limited {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
  }
  .limited .chip {
    margin-top: 0;
  }
  .chip-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 22px;
    padding: 0 9px;
    border-radius: 20px;
    border: 1px solid var(--glass-border);
    background: rgba(255, 255, 255, 0.14);
    font-size: 11.5px;
    font-weight: 600;
    transition: background-color 0.15s var(--ease);
  }
  .chip-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.22);
  }
  .chip-btn:disabled {
    opacity: 0.6;
  }
  .chip-btn .spinner {
    width: 11px;
    height: 11px;
    border-width: 1.6px;
  }
  .notice {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: -4px 0 0;
    font-size: 12px;
    color: var(--danger);
  }
  .notice svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
  .chip svg {
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .rows[hidden] {
    display: none;
  }
  .multi-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--glass-border-soft);
  }
  .count {
    flex-shrink: 0;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--muted);
  }
  .all {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    justify-content: flex-end;
  }
  .btn svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .bar {
    position: relative;
    flex: 1;
    height: 6px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.12);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: #fff;
    transition: width 0.3s var(--ease);
  }
  .bar.indeterminate .fill {
    position: absolute;
    width: 40%;
    animation: slide 1.2s var(--ease) infinite;
  }
  @keyframes slide {
    from {
      left: -40%;
    }
    to {
      left: 100%;
    }
  }
  .pct {
    min-width: 34px;
    font-size: 12px;
    color: var(--muted);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* Skeleton */
  .shimmer {
    background: linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.06) 0%,
      rgba(255, 255, 255, 0.13) 50%,
      rgba(255, 255, 255, 0.06) 100%
    );
    background-size: 200% 100%;
    animation: shimmer 1.4s linear infinite;
  }
  .skeleton .line {
    height: 12px;
    border-radius: 6px;
  }
  .skeleton .line.short {
    height: 10px;
  }
  .skeleton .text {
    gap: 9px;
  }
  .row-skel {
    height: 48px;
    border-radius: 11px;
  }
</style>
