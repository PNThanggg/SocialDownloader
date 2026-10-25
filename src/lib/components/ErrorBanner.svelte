<script lang="ts">
  import { errorMessage, t, type ErrorAction, type MessageKey } from '../i18n'
  import type { LumiError } from '../types'

  let {
    error,
    connected = false,
    compact = false,
    during = 'resolve',
    onaction,
  }: {
    error: LumiError
    connected?: boolean
    compact?: boolean
    /** What failed: reading the link (default) or a download. */
    during?: 'resolve' | 'download'
    /** Called for every action except `details`, which only expands the raw log. */
    onaction?: (action: ErrorAction) => unknown
  } = $props()

  const mapped = $derived(errorMessage(error.error, connected, during))
  const params = $derived.by((): Record<string, string> => {
    const e = error.error
    if (e.code === 'login_required') return { site: e.site }
    if (e.code === 'tool_missing') return { tool: e.tool }
    return {}
  })
  const BUTTON: Record<Exclude<ErrorAction, 'details'>, MessageKey> = {
    connect: 'btn.connect',
    reconnect: 'btn.reconnect',
    update: 'btn.update',
    retry: 'btn.retry',
    reinstall: 'btn.reinstall',
  }

  let working = $state(false)

  async function run(action: Exclude<ErrorAction, 'details'>) {
    working = true
    try {
      await onaction?.(action)
    } finally {
      working = false
    }
  }
</script>

<div class="banner" class:compact class:with-details={mapped.action === 'details'} role="alert">
  <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
    <circle cx="12" cy="12" r="9" /><path d="M12 7.5v5.5M12 16.5v.01" />
  </svg>
  <div class="body">
    <p class="msg">{$t(mapped.key, params)}</p>
    {#if mapped.action === 'details'}
      <details>
        <summary>{$t('btn.details')}</summary>
        <pre>{error.detail}</pre>
      </details>
    {/if}
  </div>
  {#if mapped.action && mapped.action !== 'details'}
    {@const action = mapped.action}
    <button type="button" class="btn btn-ghost" disabled={working} onclick={() => run(action)}>
      {#if working}<span class="spinner"></span>{/if}
      {$t(BUTTON[action], params)}
    </button>
  {/if}
</div>

<style>
  .banner {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--danger-bg);
    border: 1px solid var(--danger-border);
    backdrop-filter: blur(var(--blur));
  }
  .banner.compact {
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    gap: 8px;
  }
  .icon {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    align-self: center;
    fill: none;
    stroke: var(--danger);
    stroke-width: 2;
    stroke-linecap: round;
  }
  .with-details .icon {
    align-self: flex-start;
    margin-top: 1px;
  }
  .compact .icon {
    width: 16px;
    height: 16px;
  }
  .body {
    flex: 1;
    min-width: 0;
    align-self: center;
  }
  .msg {
    margin: 0;
    font-size: 13px;
    color: #ffe1e1;
  }
  .compact .msg {
    font-size: 12.5px;
  }
  details {
    margin-top: 6px;
  }
  summary {
    cursor: pointer;
    font-size: 12px;
    color: var(--muted);
    width: fit-content;
  }
  summary:hover {
    color: var(--text);
  }
  pre {
    margin: 6px 0 0;
    max-height: 160px;
    overflow: auto;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.28);
    color: var(--muted);
    font: 11px/1.45 'Cascadia Mono', Consolas, monospace;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
  .btn {
    flex-shrink: 0;
    align-self: center;
  }
</style>
