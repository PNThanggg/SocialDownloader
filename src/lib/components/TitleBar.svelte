<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { t } from '../i18n'

  let {
    onsettings,
    settingsOpen = false,
  }: {
    /** Shows the gear button; omitted while the setup screen runs. */
    onsettings?: () => void
    settingsOpen?: boolean
  } = $props()

  // Outside Tauri (plain browser dev) there is no window to control.
  function win() {
    try {
      return getCurrentWindow()
    } catch {
      return null
    }
  }
</script>

<header class="titlebar" data-tauri-drag-region>
  <span class="name" data-tauri-drag-region>Lumi</span>
  <div class="controls">
    {#if onsettings}
    <button
      type="button"
      class:on={settingsOpen}
      title={$t('titlebar.settings')}
      aria-label={$t('titlebar.settings')}
      aria-pressed={settingsOpen}
      onclick={() => onsettings?.()}
    >
      <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" /></svg>
    </button>
    {/if}
    <button type="button" title={$t('titlebar.minimize')} aria-label={$t('titlebar.minimize')} onclick={() => win()?.minimize()}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 12h12" /></svg>
    </button>
    <button type="button" class="close" title={$t('titlebar.close')} aria-label={$t('titlebar.close')} onclick={() => win()?.close()}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>
    </button>
  </div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: var(--titlebar-h);
    flex-shrink: 0;
    padding-left: 14px;
    user-select: none;
  }
  .name {
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .controls {
    display: flex;
    height: 100%;
  }
  button {
    width: 44px;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--muted);
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  button:hover {
    background: var(--glass-bg);
    color: var(--text);
  }
  button.on {
    color: var(--text);
  }
  button.on svg {
    transform: rotate(45deg);
  }
  svg {
    transition: transform 0.3s var(--ease);
  }
  button.close:hover {
    background: #e5484d;
    color: #fff;
  }
</style>
