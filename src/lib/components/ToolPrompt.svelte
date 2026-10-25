<script lang="ts">
  import { onMount } from 'svelte'
  import { fade, scale } from 'svelte/transition'
  import * as api from '../api'
  import { formatSize } from '../format'
  import { errorMessage, t, type MessageKey } from '../i18n'
  import { fractionOf, installWithProgress } from '../tools'
  import type { LumiError, Tool, ToolProgress, ToolStatus } from '../types'

  let {
    tool,
    autostart = false,
    oninstalled,
    onlater,
  }: {
    tool: Tool
    /** Start installing at once instead of asking first. */
    autostart?: boolean
    oninstalled: (status: ToolStatus) => void
    onlater: () => void
  } = $props()

  type Phase = { s: 'ask' } | { s: 'installing'; progress: ToolProgress | null } | { s: 'error'; error: LumiError }
  let phase = $state<Phase>({ s: 'ask' })
  /** Download size in bytes; undefined while asking the server, null when unknown. */
  let size = $state<number | null | undefined>(undefined)
  let primary: HTMLButtonElement | undefined = $state()

  const sizeText = $derived(size === undefined ? undefined : formatSize(size))
  const fraction = $derived(phase.s === 'installing' && phase.progress ? fractionOf(phase.progress) : null)
  const pct = $derived(fraction === null ? null : Math.round(fraction * 100))
  const amount = $derived.by(() => {
    if (phase.s !== 'installing' || !phase.progress) return ''
    const got = formatSize(phase.progress.downloaded)
    const total = formatSize(phase.progress.total)
    return total ? `${got} / ${total}` : (got ?? '')
  })

  onMount(() => {
    let live = true
    api.toolDownloadSize(tool).then(
      (n) => live && (size = n),
      () => live && (size = null),
    )
    if (autostart) void install()
    else queueMicrotask(() => primary?.focus())
    return () => {
      live = false
    }
  })

  async function install() {
    if (phase.s === 'installing') return
    phase = { s: 'installing', progress: null }
    try {
      const status = await installWithProgress(tool, (p) => {
        if (phase.s === 'installing') phase = { s: 'installing', progress: p }
      })
      oninstalled(status)
    } catch (e) {
      phase = { s: 'error', error: api.normalize(e) }
    }
  }

  function later() {
    if (phase.s !== 'installing') onlater()
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') later()
  }
</script>

<svelte:window {onkeydown} />

<div class="overlay" in:fade={{ duration: 140 }}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="tool-prompt-title"
    aria-describedby="tool-prompt-why"
    in:scale={{ start: 0.96, duration: 200 }}
  >
    <div class="head">
      <div class="icon" aria-hidden="true">
        <svg viewBox="0 0 24 24"><path d="M12 4.5v10.5M7 10.5l5 5 5-5M5 19.5h14" /></svg>
      </div>
      <div class="text">
        {#if sizeText === undefined}
          <div class="line shimmer" aria-hidden="true"></div>
          <div class="line short shimmer" aria-hidden="true"></div>
        {:else}
          <p class="ask" id="tool-prompt-title">
            {sizeText ? $t('prompt.ask', { tool, size: sizeText }) : $t('prompt.askNoSize', { tool })}
          </p>
        {/if}
        <p class="why" id="tool-prompt-why">{$t(`tool.why.${tool}` as MessageKey)}</p>
      </div>
    </div>

    {#if phase.s === 'installing'}
      <div class="progress">
        <div class="labels">
          <span>{$t(fraction === 1 ? 'prompt.installing' : 'prompt.downloading', { tool })}</span>
          <span class="amount">{amount}</span>
        </div>
        <div
          class="bar"
          class:indeterminate={pct === null}
          role="progressbar"
          aria-label={$t('prompt.downloading', { tool })}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={pct ?? undefined}
        >
          <div class="fill" style:width={pct === null ? undefined : `${pct}%`}></div>
        </div>
      </div>
    {:else}
      {#if phase.s === 'error'}
        <p class="err" role="alert">
          <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 7.5v5.5M12 16.5v.01" /></svg>
          {$t(errorMessage(phase.error.error, false).key, { tool })}
        </p>
      {/if}
      <div class="actions">
        <button type="button" class="btn btn-ghost" onclick={later}>{$t('action.later')}</button>
        <button type="button" class="btn btn-primary" bind:this={primary} onclick={install}>
          {#if phase.s === 'error'}
            {$t('btn.retry')}
          {:else}
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4.5v10.5M7 10.5l5 5 5-5M5 19.5h14" /></svg>
            {$t('action.download')}
          {/if}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: var(--titlebar-h) 0 0 0;
    z-index: 30;
    display: grid;
    place-items: center;
    padding: 20px;
    background: var(--scrim);
  }
  .dialog {
    width: min(380px, 100%);
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px;
    border-radius: var(--radius-lg);
    background: var(--sheet-bg);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(24px) saturate(140%);
    box-shadow:
      0 24px 60px -20px rgba(0, 0, 0, 0.7),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
  }
  .head {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    flex-shrink: 0;
    border-radius: 12px;
    background: linear-gradient(135deg, rgba(143, 188, 255, 0.28), rgba(242, 122, 168, 0.28));
    border: 1px solid var(--glass-border);
  }
  .icon svg {
    width: 18px;
    height: 18px;
    fill: none;
    stroke: #fff;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-top: 1px;
  }
  .ask {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    line-height: 1.4;
  }
  .why {
    margin: 0;
    font-size: 12.5px;
    color: var(--muted);
  }
  .line {
    height: 12px;
    margin: 3px 0;
    border-radius: 6px;
  }
  .line.short {
    width: 55%;
  }
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
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .btn svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .labels {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 12.5px;
  }
  .amount {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .bar {
    position: relative;
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
  .err {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--danger-bg);
    border: 1px solid var(--danger-border);
    font-size: 12.5px;
    color: #ffe1e1;
  }
  .err svg {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    fill: none;
    stroke: var(--danger);
    stroke-width: 2;
    stroke-linecap: round;
  }
</style>
