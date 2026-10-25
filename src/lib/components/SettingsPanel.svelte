<script lang="ts">
  import { onMount } from 'svelte'
  import { fade, fly } from 'svelte/transition'
  import { get } from 'svelte/store'
  import { accountActions, accountErrors, accounts, connectAccount, isConnected, refreshAccounts, signOut } from '../accounts'
  import * as api from '../api'
  import { language, setLanguage, t, toolName, type ErrorAction, type Language, type MessageKey } from '../i18n'
  import { toolRequests, toolStatuses } from '../tools'
  import type { LumiError, Site, Tool } from '../types'
  import ErrorBanner from './ErrorBanner.svelte'

  let { onclose }: { onclose: () => void } = $props()

  const SITES: { site: Site; name: string }[] = [
    { site: 'instagram', name: 'Instagram' },
    { site: 'facebook', name: 'Facebook' },
  ]
  const TOOLS: Tool[] = ['yt_dlp', 'gallery_dl', 'ffmpeg', 'deno']
  const LANGUAGES: { value: Language; key: MessageKey }[] = [
    { value: 'auto', key: 'lang.auto' },
    { value: 'vi', key: 'lang.vi' },
    { value: 'en', key: 'lang.en' },
  ]

  let updating = $state(false)
  let updated = $state(false)
  let updateError = $state<LumiError | null>(null)
  let updatedTimer: ReturnType<typeof setTimeout> | undefined

  /** `deno 2.1.4 (stable, …)` → `2.1.4`: the name is already in the row and the build notes are noise. */
  function versionOf(tool: Tool): string | null | undefined {
    const s = $toolStatuses?.find((x) => x.tool === tool)
    if (!s) return undefined
    if (!s.installed) return null
    const v = (s.version ?? '')
      .replace(new RegExp(`^${toolName(tool)}\\s+`, 'i'), '')
      .replace(/\s*\(.*\)\s*$/, '')
    return v || '—'
  }

  onMount(() => {
    // A Connect still waiting on its login window keeps its state; the list is reloaded after it.
    void refreshAccounts()
    api.toolsStatus().then(
      (list) => toolStatuses.set(list),
      () => {},
    )
    return () => clearTimeout(updatedTimer)
  })

  // Failures land in `accountErrors`; nothing else to do with them here.
  const connect = (site: Site) => void connectAccount(site).catch(() => {})
  const disconnect = (site: Site) => void signOut(site).catch(() => {})

  async function update() {
    if (updating) return
    updating = true
    updated = false
    updateError = null
    clearTimeout(updatedTimer)
    try {
      toolStatuses.set(await api.updateTools())
      updated = true
      updatedTimer = setTimeout(() => (updated = false), 2500)
    } catch (e) {
      updateError = api.normalize(e)
    } finally {
      updating = false
    }
  }

  function onUpdateError(action: ErrorAction) {
    if (action === 'retry' || action === 'update') return update()
  }

  function onkeydown(e: KeyboardEvent) {
    // A tool prompt above the sheet takes this Escape for itself.
    if (e.key === 'Escape' && get(toolRequests).length === 0) onclose()
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (Escape closes too) -->
<div class="backdrop" onclick={onclose} transition:fade={{ duration: 180 }}></div>

<div
  class="sheet"
  role="dialog"
  aria-modal="true"
  aria-labelledby="settings-title"
  transition:fly={{ x: 360, duration: 280, opacity: 1 }}
>
  <header>
    <h2 id="settings-title">{$t('titlebar.settings')}</h2>
    <button type="button" class="close" title={$t('titlebar.close')} aria-label={$t('titlebar.close')} onclick={onclose}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>
    </button>
  </header>

  <div class="body">
    <section>
      <h3>{$t('settings.accounts')}</h3>
      <div class="group">
        {#each SITES as { site, name } (site)}
          {@const on = isConnected($accounts, site)}
          {@const doing = $accountActions[site]?.kind}
          {@const err = $accountErrors[site]}
          <div class="item account" data-site={site}>
            <span class="logo {site}" aria-hidden="true">
              {#if site === 'instagram'}
                <svg viewBox="0 0 24 24"
                  ><rect x="4" y="4" width="16" height="16" rx="4.5" /><circle cx="12" cy="12" r="3.6" /><path d="M16.6 7.4v.01" /></svg
                >
              {:else}
                <svg viewBox="0 0 24 24"><path d="M14.5 8.5h2V5.5h-2.4c-2.3 0-3.6 1.5-3.6 3.8v2H8.5v3h2v6h3v-6h2.3l.5-3h-2.8V9.6c0-.7.3-1.1 1-1.1z" /></svg>
              {/if}
            </span>
            <div class="what">
              <span class="name">{name}</span>
              <span class="status" class:on class:waiting={doing === 'connect'}>
                <span class="led" aria-hidden="true"></span>
                {$t(doing === 'connect' ? 'account.waiting' : on ? 'account.connected' : 'account.disconnected')}
              </span>
            </div>
            {#if doing === 'connect'}
              <!-- Resolves when the login window closes; nothing to cancel from here. -->
              <button type="button" class="btn btn-ghost icon-btn" disabled aria-label={$t('account.waiting')}>
                <span class="spinner"></span>
              </button>
            {:else if on}
              <button type="button" class="btn btn-ghost" disabled={doing === 'signout'} onclick={() => disconnect(site)}>
                {#if doing === 'signout'}<span class="spinner"></span>{/if}
                {$t('account.signOut')}
              </button>
            {:else}
              <button type="button" class="btn btn-primary" onclick={() => connect(site)}>{$t('account.connect')}</button>
            {/if}
          </div>
          {#if err}
            <div class="item-error">
              <ErrorBanner error={err} compact />
            </div>
          {/if}
        {/each}
      </div>
    </section>

    <section>
      <div class="section-head">
        <h3>{$t('settings.tools')}</h3>
        {#if updated}
          <span class="ok" in:fade={{ duration: 150 }}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
            {$t('settings.updated')}
          </span>
        {/if}
        <button type="button" class="btn btn-ghost" disabled={updating} onclick={update}>
          {#if updating}
            <span class="spinner"></span>
          {:else}
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v3.8h-3.8" /></svg>
          {/if}
          {$t('btn.update')}
        </button>
      </div>
      <div class="group">
        {#each TOOLS as tool (tool)}
          {@const v = versionOf(tool)}
          <div class="item tool">
            <span class="name">{toolName(tool)}</span>
            {#if v === undefined}
              <span class="version shimmer" aria-hidden="true"></span>
            {:else if v === null}
              <span class="version missing">{$t('tool.notInstalled')}</span>
            {:else}
              <span class="version" title={v}>{v}</span>
            {/if}
          </div>
        {/each}
      </div>
      {#if updateError}
        <div class="item-error">
          <ErrorBanner error={updateError} compact onaction={onUpdateError} />
        </div>
      {/if}
    </section>

    <section>
      <h3>{$t('settings.language')}</h3>
      <div class="segmented" role="radiogroup" aria-label={$t('settings.language')}>
        {#each LANGUAGES as { value, key } (value)}
          <label class:checked={$language === value}>
            <input
              type="radio"
              name="lumi-language"
              {value}
              checked={$language === value}
              onchange={() => void setLanguage(value)}
            />
            <span>{$t(key)}</span>
          </label>
        {/each}
      </div>
    </section>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: var(--titlebar-h) 0 0 0;
    z-index: 20;
    background: var(--scrim);
  }
  .sheet {
    position: fixed;
    top: var(--titlebar-h);
    right: 0;
    bottom: 0;
    z-index: 21;
    width: min(340px, calc(100% - 48px));
    display: flex;
    flex-direction: column;
    background: var(--sheet-bg);
    border-left: 1px solid var(--glass-border);
    border-top-left-radius: var(--radius-lg);
    border-top: 1px solid var(--glass-border-soft);
    backdrop-filter: blur(24px) saturate(140%);
    box-shadow: -24px 0 60px -24px rgba(0, 0, 0, 0.7);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 10px 6px 18px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 650;
  }
  .close {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
  }
  .close:hover {
    background: var(--glass-bg);
    color: var(--text);
  }
  .close svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 6px 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  h3 {
    margin: 0 0 8px 2px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .section-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .section-head h3 {
    flex: 1;
    margin-bottom: 0;
  }
  .ok {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--tag-mp3);
  }
  .ok svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .group {
    display: flex;
    flex-direction: column;
    border-radius: var(--radius);
    background: var(--glass-bg-soft);
    border: 1px solid var(--glass-border-soft);
    overflow: hidden;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    padding: 8px 10px 8px 12px;
  }
  .item + .item,
  .item-error + .item {
    border-top: 1px solid var(--glass-border-soft);
  }
  .item-error {
    padding: 0 10px 10px;
  }
  .group + .item-error {
    padding: 8px 0 0;
  }
  .logo {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    border-radius: 9px;
  }
  .logo.instagram {
    background: linear-gradient(135deg, #7c5cff 0%, #e1306c 55%, #ffb347 100%);
  }
  .logo.facebook {
    background: #1877f2;
  }
  .logo svg {
    width: 17px;
    height: 17px;
  }
  .logo.instagram svg {
    fill: none;
    stroke: #fff;
    stroke-width: 1.9;
    stroke-linecap: round;
  }
  .logo.facebook svg {
    fill: #fff;
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .name {
    font-size: 13px;
    font-weight: 600;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }
  .led {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--faint);
  }
  .status.on {
    color: var(--tag-mp3);
  }
  .status {
    white-space: nowrap;
  }
  .status.waiting {
    color: var(--text);
  }
  .status.waiting .led {
    background: var(--tag-mp4);
    animation: blink 1s var(--ease) infinite alternate;
  }
  @keyframes blink {
    to {
      opacity: 0.3;
    }
  }
  .icon-btn {
    width: 30px;
    padding: 0;
  }
  .status.on .led {
    background: var(--tag-mp3);
    box-shadow: 0 0 6px var(--tag-mp3);
  }
  .tool {
    min-height: 38px;
    padding-right: 12px;
  }
  .tool .name {
    flex: 1;
  }
  .version {
    max-width: 60%;
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .version.missing {
    color: var(--faint);
  }
  .version.shimmer {
    width: 64px;
    height: 10px;
    border-radius: 5px;
    background: linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.06) 0%,
      rgba(255, 255, 255, 0.13) 50%,
      rgba(255, 255, 255, 0.06) 100%
    );
    background-size: 200% 100%;
    animation: shimmer 1.4s linear infinite;
  }
  .btn svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .segmented {
    display: flex;
    padding: 3px;
    gap: 3px;
    border-radius: 10px;
    background: var(--glass-bg-soft);
    border: 1px solid var(--glass-border-soft);
  }
  .segmented label {
    position: relative;
    flex: 1;
    display: grid;
    place-items: center;
    height: 30px;
    border-radius: 7px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--muted);
    cursor: pointer;
    transition:
      background-color 0.15s var(--ease),
      color 0.15s var(--ease);
  }
  .segmented label:hover {
    color: var(--text);
  }
  .segmented label.checked {
    background: var(--primary-bg);
    color: var(--primary-fg);
  }
  .segmented input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .segmented label:has(input:focus-visible) {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
