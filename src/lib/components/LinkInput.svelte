<script lang="ts">
  import { readText } from '@tauri-apps/plugin-clipboard-manager'
  import { onMount } from 'svelte'
  import { fly } from 'svelte/transition'
  import { detectSiteLocal } from '../api'
  import { t } from '../i18n'

  let {
    value = $bindable(''),
    busy = false,
    disabled = false,
    lastResolved = '',
    onsubmit,
  }: {
    value?: string
    busy?: boolean
    /** Nothing can run yet (tools still being checked); typing still works. */
    disabled?: boolean
    /** The link shown in the result card; never suggested again. */
    lastResolved?: string
    onsubmit: () => void
  } = $props()

  let suggestion: string | null = $state(null)
  let dismissed = ''
  let input: HTMLInputElement | undefined = $state()

  const canRun = $derived(!busy && !disabled && value.trim() !== '')

  function submit() {
    if (!canRun) return
    suggestion = null
    onsubmit()
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      submit()
    }
  }

  /** Offers the clipboard's link when it is a single supported-site URL we have not just used. */
  async function checkClipboard() {
    let text: string
    try {
      text = ((await readText()) ?? '').trim()
    } catch {
      return
    }
    const site = text.length > 2048 || /\s/.test(text) ? null : detectSiteLocal(text)
    const fresh = text !== lastResolved.trim() && text !== value.trim() && text !== dismissed
    suggestion = site && site !== 'other' && fresh ? text : null
  }

  function accept() {
    if (!suggestion || disabled) return
    value = suggestion
    suggestion = null
    input?.focus()
  }

  function dismiss() {
    dismissed = suggestion ?? ''
    suggestion = null
  }

  /** `www.tiktok.com/@a/video/123…` for the chip. */
  function short(url: string): string {
    const s = url.replace(/^https?:\/\//, '')
    return s.length > 38 ? `${s.slice(0, 37)}…` : s
  }

  onMount(() => {
    void checkClipboard()
    const onfocus = () => void checkClipboard()
    window.addEventListener('focus', onfocus)
    return () => window.removeEventListener('focus', onfocus)
  })
</script>

<div class="wrap">
  <div class="field glass" class:busy>
    <svg class="lead" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M10 14a4.5 4.5 0 0 0 6.4 0l3-3a4.5 4.5 0 0 0-6.4-6.4l-1 1" />
      <path d="M14 10a4.5 4.5 0 0 0-6.4 0l-3 3a4.5 4.5 0 0 0 6.4 6.4l1-1" />
    </svg>
    <input
      bind:this={input}
      bind:value
      type="text"
      inputmode="url"
      spellcheck="false"
      autocomplete="off"
      aria-label={$t('hero.title')}
      placeholder={$t('hero.placeholder')}
      {onkeydown}
    />
    <button type="button" class="btn btn-primary go" disabled={!canRun} onclick={submit}>
      {#if busy}<span class="spinner"></span>{/if}
      {$t('action.resolve')}
    </button>
  </div>

  {#if suggestion}
    <div class="chip glass" transition:fly={{ y: -4, duration: 180 }}>
      <button type="button" class="take" {disabled} onclick={accept}>
        <svg viewBox="0 0 24 24" aria-hidden="true"
          ><rect x="6" y="4.5" width="12" height="16" rx="2.5" /><path d="M9.5 4.5h5v2.5h-5z" /></svg
        >
        <span>{$t('clip.suggest')}</span>
        <span class="url">{short(suggestion)}</span>
      </button>
      <button type="button" class="x" title={$t('action.dismiss')} aria-label={$t('action.dismiss')} onclick={dismiss}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>
      </button>
    </div>
  {/if}
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    width: 100%;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 48px;
    padding: 0 6px 0 14px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.07);
    border-color: rgba(255, 255, 255, 0.15);
    transition:
      border-color 0.15s var(--ease),
      background-color 0.15s var(--ease),
      box-shadow 0.15s var(--ease);
  }
  .field:focus-within {
    border-color: rgba(255, 255, 255, 0.32);
    background: rgba(255, 255, 255, 0.1);
    box-shadow: 0 0 0 4px rgba(255, 255, 255, 0.05);
  }
  .lead {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    fill: none;
    stroke: var(--faint);
    stroke-width: 2;
    stroke-linecap: round;
  }
  .field:focus-within .lead {
    stroke: var(--muted);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    outline: none;
    background: transparent;
    font-size: 14px;
  }
  input::placeholder {
    color: var(--faint);
  }
  .go {
    height: 36px;
    padding: 0 16px;
    border-radius: 10px;
    font-size: 13px;
  }
  .chip {
    display: flex;
    align-items: center;
    max-width: 100%;
    padding: 2px;
    border-radius: 20px;
    background: rgba(255, 255, 255, 0.07);
  }
  .chip button {
    border: 0;
    background: transparent;
    display: flex;
    align-items: center;
    gap: 7px;
    border-radius: 18px;
    transition: background-color 0.15s var(--ease);
  }
  .chip button:hover {
    background: rgba(255, 255, 255, 0.08);
  }
  .take {
    min-width: 0;
    padding: 5px 8px 5px 10px;
    font-size: 12.5px;
  }
  .take span {
    white-space: nowrap;
  }
  .url {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--muted);
    font-size: 12px;
  }
  .chip svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.9;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .x {
    padding: 6px;
    color: var(--muted);
  }
</style>
