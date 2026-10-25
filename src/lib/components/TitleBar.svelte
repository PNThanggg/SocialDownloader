<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { t } from '../i18n'

  let { onsettings }: { onsettings?: () => void } = $props()

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
    <button type="button" title={$t('titlebar.settings')} aria-label={$t('titlebar.settings')} onclick={() => onsettings?.()}>⚙</button>
    <button type="button" title={$t('titlebar.minimize')} aria-label={$t('titlebar.minimize')} onclick={() => win()?.minimize()}>–</button>
    <button type="button" class="close" title={$t('titlebar.close')} aria-label={$t('titlebar.close')} onclick={() => win()?.close()}>×</button>
  </div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 36px;
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
    font-size: 16px;
    cursor: pointer;
  }
  button:hover {
    background: var(--glass-bg);
    color: var(--text);
  }
  button.close:hover {
    background: #e5484d;
    color: #fff;
  }
</style>
