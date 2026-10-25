<script lang="ts">
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount } from 'svelte'
  import { get } from 'svelte/store'
  import { fade, fly } from 'svelte/transition'
  import * as api from '../api'
  import { formatWhen } from '../format'
  import { locale, t } from '../i18n'
  import { toolRequests } from '../tools'
  import type { HistoryEntry } from '../types'
  import Notice, { NoticeState } from './Notice.svelte'

  let {
    onclose,
    onagain,
    busy = false,
  }: {
    onclose: () => void
    /** "Download again": read the entry's link afresh. */
    onagain: (url: string) => void
    /** A link is being read; "Download again" waits for it. */
    busy?: boolean
  } = $props()

  /** null until the first load. */
  let entries = $state<HistoryEntry[] | null>(null)
  /** "Clear all" asks for a second click instead of a dialog. */
  let confirming = $state(false)
  let confirmTimer: ReturnType<typeof setTimeout> | undefined
  let now = $state(Date.now())
  const notice = new NoticeState()

  const TONES: Record<string, string> = {
    mp3: 'mp3', m4a: 'mp3', opus: 'mp3', webm: 'mp4', mp4: 'mp4', mkv: 'mp4', mov: 'mp4',
    jpg: 'img', jpeg: 'img', png: 'img', webp: 'img', gif: 'img', heic: 'img',
  }

  async function load() {
    try {
      entries = await api.historyList()
    } catch {
      entries ??= []
      notice.flash('notice.historyFailed')
    }
  }

  onMount(() => {
    void load()
    // A download that finishes while the sheet is open shows up right away.
    // Outside Tauri (plain browser dev) there is no event bus.
    const unlisten = api.onHistory(() => void load()).catch(() => null)
    const tick = setInterval(() => (now = Date.now()), 60_000)
    return () => {
      void unlisten.then((u) => u?.())
      clearInterval(tick)
      clearTimeout(confirmTimer)
      notice.dispose()
    }
  })

  /** The link without its scheme and `www.`, for a compact second line. */
  function shortUrl(url: string): string {
    return url.replace(/^https?:\/\/(www\.)?/i, '')
  }

  async function remove(url: string) {
    try {
      await api.historyRemove(url)
      entries = entries?.filter((e) => e.url !== url) ?? null
    } catch {
      notice.flash('notice.historyFailed')
    }
  }

  async function clearAll() {
    if (!confirming) {
      confirming = true
      clearTimeout(confirmTimer)
      confirmTimer = setTimeout(() => (confirming = false), 3000)
      return
    }
    confirming = false
    clearTimeout(confirmTimer)
    try {
      await api.historyClear()
      entries = []
    } catch {
      notice.flash('notice.historyFailed')
    }
  }

  async function reveal(path: string) {
    try {
      await revealItemInDir(path)
    } catch {
      // Usually the file was moved or deleted since.
      notice.flash('notice.revealFailed')
    }
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
  aria-labelledby="history-title"
  transition:fly={{ x: 360, duration: 280, opacity: 1 }}
>
  <header>
    <h2 id="history-title">{$t('titlebar.history')}</h2>
    {#if entries && entries.length > 0}
      <button type="button" class="btn btn-ghost clear" class:confirming onclick={clearAll}>
        {$t(confirming ? 'history.clearConfirm' : 'history.clear')}
      </button>
    {/if}
    <button type="button" class="close" title={$t('titlebar.close')} aria-label={$t('titlebar.close')} onclick={onclose}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>
    </button>
  </header>

  <div class="body">
    <Notice {notice} --notice-space="0 2px 8px" />
    {#if entries === null}
      <div class="group" aria-hidden="true">
        {#each [0, 1, 2] as i (i)}
          <div class="entry"><span class="shimmer"></span><span class="shimmer short"></span></div>
        {/each}
      </div>
    {:else if entries.length === 0}
      <p class="empty">{$t('history.empty')}</p>
    {:else}
      <ul class="group">
        {#each entries as e (e.url)}
          <li class="entry" data-url={e.url}>
            <span class="title" title={e.title || e.url}>{e.title || $t('label.untitled')}</span>
            <span class="link" title={e.url}>{shortUrl(e.url)}</span>
            <div class="foot">
              <span class="tags">
                {#each e.formats as f (f)}
                  <span class="tag {TONES[f] ?? ''}">{f === 'all' ? $t('history.all') : f.toUpperCase()}</span>
                {/each}
              </span>
              <span class="when">{formatWhen(e.at, now, $locale)}</span>
              <button
                type="button"
                class="icon"
                title={$t('action.openFolder')}
                aria-label={$t('action.openFolder')}
                onclick={() => reveal(e.path)}
              >
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" /></svg>
              </button>
              <button
                type="button"
                class="icon"
                title={$t('history.remove')}
                aria-label={$t('history.remove')}
                onclick={() => remove(e.url)}
              >
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12" /></svg>
              </button>
              <button type="button" class="btn btn-primary again" disabled={busy} onclick={() => onagain(e.url)}>
                {$t('history.again')}
              </button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
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
    width: min(380px, calc(100% - 48px));
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
    gap: 6px;
    padding: 14px 10px 6px 18px;
  }
  h2 {
    flex: 1;
    margin: 0;
    font-size: 16px;
    font-weight: 650;
  }
  .clear {
    height: 28px;
    font-size: 12px;
  }
  .clear.confirming {
    color: var(--danger);
  }
  .close,
  .icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
  }
  .close:hover,
  .icon:hover {
    background: var(--glass-bg);
    color: var(--text);
  }
  .close svg,
  .icon svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 6px 18px 20px;
  }
  .empty {
    margin: 24px 4px;
    font-size: 13px;
    color: var(--muted);
    text-align: center;
  }
  .group {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border-radius: var(--radius);
    background: var(--glass-bg-soft);
    border: 1px solid var(--glass-border-soft);
    overflow: hidden;
  }
  .entry {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 10px 8px 12px;
  }
  .entry + .entry {
    border-top: 1px solid var(--glass-border-soft);
  }
  .title {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    font-size: 13px;
    font-weight: 600;
    word-break: break-word;
  }
  .link {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 11.5px;
    color: var(--faint);
    user-select: text;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 4px;
  }
  .tags {
    display: flex;
    gap: 4px;
    min-width: 0;
    overflow: hidden;
  }
  .tag {
    flex-shrink: 0;
    padding: 2px 6px;
    border-radius: 6px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--muted);
    background: var(--glass-bg);
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
  .when {
    flex: 1;
    margin-left: 4px;
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .again {
    height: 28px;
    margin-left: 2px;
    font-size: 12px;
  }
  .shimmer {
    display: block;
    width: 80%;
    height: 10px;
    margin: 4px 0;
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
  .shimmer.short {
    width: 45%;
  }
</style>
