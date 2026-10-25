<script lang="ts" module>
  import type { MessageKey } from '../i18n'

  /** A short message that clears itself, for side actions (copy, open folder, cancel) that failed. */
  export class NoticeState {
    key = $state<MessageKey | null>(null)
    private timer: ReturnType<typeof setTimeout> | undefined

    flash(key: MessageKey, ms = 3500) {
      this.key = key
      clearTimeout(this.timer)
      this.timer = setTimeout(() => (this.key = null), ms)
    }

    /** Call when the owner unmounts. */
    dispose() {
      clearTimeout(this.timer)
    }
  }
</script>

<script lang="ts">
  import { slide } from 'svelte/transition'
  import { t } from '../i18n'

  let { notice }: { notice: NoticeState } = $props()
</script>

{#if notice.key}
  <p class="notice" role="status" transition:slide={{ duration: 160 }}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 7.5v5.5M12 16.5v.01" /></svg>
    {$t(notice.key)}
  </p>
{/if}

<style>
  .notice {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    /* Owners set the gap above it, e.g. `<Notice --notice-space="6px 2px 0" />`. */
    padding: var(--notice-space, 0);
    font-size: 12px;
    color: var(--danger);
  }
  svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
</style>
