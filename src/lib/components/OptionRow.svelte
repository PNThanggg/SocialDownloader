<script lang="ts">
  import { writeText } from '@tauri-apps/plugin-clipboard-manager'
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount } from 'svelte'
  import { slide } from 'svelte/transition'
  import * as api from '../api'
  import { JobFollower, pickDest, realDeps, viewOf, type JobView } from '../download'
  import { formatSize } from '../format'
  import { t, type ErrorAction, type MessageKey } from '../i18n'
  import { askForTool } from '../tools'
  import type { AccountStatus, LumiError, MediaInfo, MediaOption, OptionKind } from '../types'
  import ErrorBanner from './ErrorBanner.svelte'

  let {
    info,
    opt,
    accounts = [],
    onfix,
  }: {
    info: MediaInfo
    opt: MediaOption
    accounts?: AccountStatus[]
    /** Runs an error button's side effect (connect, update, reinstall); true = retry the download. */
    onfix?: (action: ErrorAction, error: LumiError) => Promise<boolean>
  } = $props()

  let view = $state<JobView>({ s: 'idle' })
  /** The file chosen in Save as; kept so a retry or the ffmpeg install reuses it. */
  let dest: string | null = $state(null)
  let copied = $state(false)
  let cancelling = $state(false)
  /** A short message for a side action (copy, open folder, cancel) that failed; the view stays as it was. */
  let notice = $state<MessageKey | null>(null)
  let noticeTimer: ReturnType<typeof setTimeout> | undefined

  const follower = new JobFollower((e) => {
    if (e.state === 'failed' && needsFfmpeg(e.error)) {
      follower.stop()
      cancelling = false
      void offerFfmpeg()
      return
    }
    view = viewOf(e)
    if (e.state === 'done' || e.state === 'failed' || e.state === 'cancelled') {
      follower.stop()
      cancelling = false
    }
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

  const needsFfmpeg = (e: LumiError) => e.error.code === 'tool_missing' && e.error.tool === 'ffmpeg'

  /** Asks to install ffmpeg; once installed, downloads to the kept dest (no second Save as). */
  async function offerFfmpeg() {
    view = { s: 'busy' }
    if (await askForTool('ffmpeg')) await run()
    else view = { s: 'idle' }
  }

  const TAG_TONE: Record<OptionKind, string> = { video: 'mp4', audio: 'mp3', original_sound: 'mp3', image: 'img' }
  const tag = $derived(opt.kind === 'audio' || opt.kind === 'original_sound' ? 'MP3' : opt.ext.toUpperCase())
  const title = $derived(
    opt.kind === 'audio'
      ? $t('label.audio')
      : opt.kind === 'original_sound'
        ? $t('label.sound')
        : opt.kind === 'image'
          ? $t('label.image')
          : opt.label,
  )
  const size = $derived(formatSize(opt.filesize))

  const noteText = $derived.by((): string | null => {
    const n = opt.note
    if (!n) return null
    const keys: Record<typeof n.kind, MessageKey> = {
      no_audio: 'note.noAudio',
      source_is_video: 'note.sourceIsVideo',
      source_audio: 'note.sourceAudio',
      limited_formats: 'note.limited',
    }
    return $t(keys[n.kind], n.kind === 'source_audio' ? { ext: n.ext } : undefined)
  })
  const copyTip = $derived([$t('note.expiry'), noteText].filter(Boolean).join('\n'))

  const connected = $derived.by(() => {
    if (view.s !== 'error') return false
    const e = view.error.error
    const site = e.code === 'login_required' ? e.site : info.site
    return accounts.some((a) => a.site === site && a.connected)
  })
  const pct = $derived(view.s === 'running' && view.fraction !== null ? Math.round(view.fraction * 100) : null)

  async function copy() {
    if (!opt.directUrl) return
    try {
      await writeText(opt.directUrl)
      copied = true
      setTimeout(() => (copied = false), 1600)
    } catch {
      flash('notice.copyFailed')
    }
  }

  async function download() {
    view = { s: 'busy' }
    try {
      const chosen = await pickDest(await realDeps(), info, opt)
      if (chosen === null) {
        view = { s: 'idle' }
        return
      }
      dest = chosen
      await run()
    } catch (e) {
      view = { s: 'error', error: api.normalize(e) }
    }
  }

  /** Starts the job for the kept `dest`. */
  async function run() {
    if (dest === null) return
    await subscribed
    follower.arm()
    view = { s: 'running', fraction: null }
    try {
      follower.follow(await api.download(info.sourceUrl, opt.id, dest))
    } catch (e) {
      follower.stop()
      const error = api.normalize(e)
      if (needsFfmpeg(error)) await offerFfmpeg()
      else view = { s: 'error', error }
    }
  }

  async function cancel() {
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

  async function onerror(action: ErrorAction) {
    if (view.s !== 'error') return
    const error = view.error
    try {
      const again = action === 'retry' || ((await onfix?.(action, error)) ?? false)
      // No dest yet means Save as itself failed, so start over from the dialog.
      if (again) await (dest === null ? download() : run())
    } catch (e) {
      view = { s: 'error', error: api.normalize(e) }
    }
  }

  async function reveal(path: string) {
    try {
      await revealItemInDir(path)
    } catch {
      flash('notice.revealFailed')
    }
  }
</script>

<div class="row" data-option={opt.id}>
  <div class="line">
    <span class="tag {TAG_TONE[opt.kind]}">{tag}</span>
    <div class="info">
      <div class="top">
        <span class="title">{title}</span>
        {#if size}<span class="size">{size}</span>{/if}
      </div>
      {#if opt.kind === 'original_sound'}
        <span class="sub" title={opt.label}>{opt.label}</span>
      {/if}
    </div>

    <div class="actions">
      {#if view.s === 'running' || view.s === 'processing'}
        <div class="job">
          <div
            class="bar"
            class:indeterminate={pct === null}
            role="progressbar"
            aria-label={title}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={pct ?? undefined}
          >
            <div class="fill" style:width={pct === null ? undefined : `${pct}%`}></div>
          </div>
          <span class="pct">{view.s === 'processing' ? $t('status.processing') : pct === null ? '' : `${pct}%`}</span>
        </div>
        <button
          type="button"
          class="btn btn-ghost icon-btn"
          title={$t('action.cancel')}
          aria-label={$t('action.cancel')}
          disabled={cancelling}
          onclick={cancel}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
        </button>
      {:else}
        {#if opt.directUrl}
          <button type="button" class="btn btn-ghost" class:ok={copied} title={copyTip} onclick={copy}>
            {#if copied}
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
              {$t('action.copied')}
            {:else}
              <svg viewBox="0 0 24 24" aria-hidden="true"
                ><rect x="8.5" y="8.5" width="11" height="11" rx="2.5" /><path d="M15.5 8.5V6.5a2 2 0 0 0-2-2h-7a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h2" /></svg
              >
              {$t('action.copy')}
            {/if}
          </button>
        {/if}
        {#if view.s === 'done'}
          {@const path = view.path}
          <button type="button" class="btn btn-primary" title={path} onclick={() => reveal(path)}>
            <svg viewBox="0 0 24 24" aria-hidden="true"
              ><path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" /></svg
            >
            {$t('action.openFolder')}
          </button>
          <button
            type="button"
            class="btn btn-ghost icon-btn"
            title={$t('action.download')}
            aria-label={$t('action.download')}
            onclick={download}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4.5v10.5M7 10.5l5 5 5-5M5 19.5h14" /></svg>
          </button>
        {:else}
          <button type="button" class="btn btn-primary" disabled={view.s === 'busy'} onclick={download}>
            {#if view.s === 'busy'}
              <span class="spinner"></span>
            {:else}
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4.5v10.5M7 10.5l5 5 5-5M5 19.5h14" /></svg>
            {/if}
            {$t('action.download')}
          </button>
        {/if}
      {/if}
    </div>
  </div>

  {#if view.s === 'error'}
    <div class="err">
      <ErrorBanner error={view.error} {connected} compact onaction={onerror} />
    </div>
  {/if}
  {#if notice}
    <p class="notice" role="status" transition:slide={{ duration: 160 }}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 7.5v5.5M12 16.5v.01" /></svg>
      {$t(notice)}
    </p>
  {/if}
</div>

<style>
  .row {
    padding: 8px 8px 8px 10px;
    border-radius: 11px;
    background: var(--glass-bg-soft);
    border: 1px solid var(--glass-border-soft);
    transition: background-color 0.15s var(--ease);
  }
  .row:hover {
    background: rgba(255, 255, 255, 0.075);
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 32px;
  }
  .tag {
    flex-shrink: 0;
    min-width: 42px;
    padding: 3px 7px;
    border-radius: 6px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-align: center;
  }
  .tag.mp3 {
    color: var(--tag-mp3);
    background: var(--tag-mp3-bg);
  }
  .tag.mp4 {
    color: var(--tag-mp4);
    background: var(--tag-mp4-bg);
  }
  .tag.img {
    color: var(--tag-img);
    background: var(--tag-img-bg);
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .title {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .size {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .btn svg,
  .icon-btn svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .icon-btn {
    width: 30px;
    padding: 0;
  }
  .btn.ok {
    color: var(--tag-mp3);
  }
  .job {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 150px;
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
    background: linear-gradient(90deg, #ffffffcc, #ffffff);
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
    white-space: nowrap;
  }
  .err {
    margin-top: 8px;
  }
  .notice {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 6px 2px 0;
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
</style>
