<script lang="ts">
  import { onMount } from 'svelte'
  import { fly } from 'svelte/transition'
  import * as api from '../api'
  import { formatSize } from '../format'
  import { errorMessage, t, toolName } from '../i18n'
  import { fractionOf, installWithProgress } from '../tools'
  import type { LumiError, Tool, ToolProgress } from '../types'

  let {
    missing,
    onready,
  }: {
    /** Tools to install, in order. */
    missing: Tool[]
    onready: () => void
  } = $props()

  /** Index of the tool being installed; equals `missing.length` when all are done. */
  let current = $state(0)
  let progress = $state<ToolProgress | null>(null)
  let error = $state<LumiError | null>(null)
  let running = false

  const fraction = $derived(progress ? fractionOf(progress) : null)
  const pct = $derived(fraction === null ? null : Math.round(fraction * 100))
  const amount = $derived.by(() => {
    if (!progress) return ''
    const got = formatSize(progress.downloaded)
    const total = formatSize(progress.total)
    return total ? `${got} / ${total}` : (got ?? '')
  })

  /** Installs from the current tool on; a failure stops there so Retry resumes it. */
  async function run() {
    if (running) return
    running = true
    error = null
    try {
      while (current < missing.length) {
        progress = null
        const tool = missing[current]
        await installWithProgress(tool, (p) => (progress = p))
        current += 1
      }
    } catch (e) {
      error = api.normalize(e)
      return
    } finally {
      running = false
    }
    onready()
  }

  onMount(() => {
    void run()
  })
</script>

<section class="setup" aria-busy={error === null}>
  <div class="mark" class:pulse={error === null} aria-hidden="true">
    <svg viewBox="0 0 32 32">
      <defs>
        <linearGradient id="lumi-setup-g" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#8fbcff" />
          <stop offset="1" stop-color="#f27aa8" />
        </linearGradient>
      </defs>
      <path
        fill="url(#lumi-setup-g)"
        d="M16 3.5c.9 5.6 3.1 8.9 7.6 10.4l1.9.6-1.9.6C19.1 16.6 16.9 19.9 16 25.5c-.9-5.6-3.1-8.9-7.6-10.4l-1.9-.6 1.9-.6C12.9 12.4 15.1 9.1 16 3.5z"
      />
      <circle cx="24.5" cy="24" r="2.2" fill="url(#lumi-setup-g)" opacity=".85" />
    </svg>
  </div>
  <h1>{$t('setup.title')}</h1>
  <p class="sub">{$t('setup.sub')}</p>

  <ul class="tools glass">
    {#each missing as tool, i (tool)}
      {@const st = i < current ? 'done' : i === current ? (error ? 'failed' : 'active') : 'waiting'}
      <li class={st}>
        <div class="line">
          <span class="dot" aria-hidden="true">
            {#if st === 'done'}
              <svg viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
            {:else if st === 'active'}
              <span class="spinner"></span>
            {:else if st === 'failed'}
              <svg viewBox="0 0 24 24"><path d="M12 7.5v5.5M12 16.5v.01" /></svg>
            {/if}
          </span>
          <span class="name">{toolName(tool)}</span>
          <span class="state">
            {#if st === 'done'}
              {$t('setup.done')}
            {:else if st === 'active'}
              {amount}
            {:else if st === 'waiting'}
              {$t('setup.waiting')}
            {/if}
          </span>
        </div>
        {#if st === 'active'}
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
        {/if}
      </li>
    {/each}
  </ul>

  {#if error}
    <div class="error" role="alert" in:fly={{ y: 6, duration: 200 }}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 7.5v5.5M12 16.5v.01" /></svg>
      <p>{$t(errorMessage(error.error, false).key, { tool: missing[current] })}</p>
      <button type="button" class="btn btn-primary" onclick={run}>{$t('btn.retry')}</button>
    </div>
  {/if}
</section>

<style>
  .setup {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: calc(20vh - 30px) 20px 20px;
    text-align: center;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    margin-bottom: 18px;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(var(--blur));
    box-shadow:
      0 10px 30px -12px rgba(120, 150, 255, 0.45),
      inset 0 1px 0 rgba(255, 255, 255, 0.12);
  }
  .mark.pulse {
    animation: glow 2.4s var(--ease) infinite;
  }
  @keyframes glow {
    50% {
      box-shadow:
        0 12px 38px -10px rgba(242, 122, 168, 0.55),
        inset 0 1px 0 rgba(255, 255, 255, 0.12);
    }
  }
  .mark svg {
    width: 30px;
    height: 30px;
  }
  h1 {
    margin: 0 0 6px;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .sub {
    margin: 0;
    max-width: 320px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .tools {
    width: 100%;
    max-width: 360px;
    margin: 24px 0 0;
    padding: 6px;
    list-style: none;
    border-radius: var(--radius-lg);
    text-align: left;
  }
  li {
    padding: 10px 10px;
    border-radius: 11px;
  }
  li.active,
  li.failed {
    background: var(--glass-bg-soft);
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .dot {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    border-radius: 50%;
    border: 1px solid var(--glass-border);
    color: var(--muted);
  }
  .done .dot {
    border-color: transparent;
    background: var(--tag-mp3-bg);
    color: var(--tag-mp3);
  }
  .failed .dot {
    border-color: var(--danger-border);
    background: var(--danger-bg);
    color: var(--danger);
  }
  .dot svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .dot .spinner {
    width: 12px;
    height: 12px;
    border-width: 1.8px;
    color: var(--text);
  }
  .name {
    flex: 1;
    font-size: 13px;
    font-weight: 600;
  }
  .waiting .name {
    color: var(--faint);
  }
  .state {
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .waiting .state {
    color: var(--faint);
  }
  .bar {
    position: relative;
    height: 6px;
    margin: 10px 0 2px 32px;
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
  .error {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    max-width: 360px;
    margin-top: 12px;
    padding: 10px 10px 10px 14px;
    border-radius: var(--radius);
    background: var(--danger-bg);
    border: 1px solid var(--danger-border);
    backdrop-filter: blur(var(--blur));
    text-align: left;
  }
  .error svg {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    fill: none;
    stroke: var(--danger);
    stroke-width: 2;
    stroke-linecap: round;
  }
  .error p {
    flex: 1;
    margin: 0;
    font-size: 13px;
    color: #ffe1e1;
  }
</style>
