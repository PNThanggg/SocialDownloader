<script lang="ts">
  import * as api from '../api'
  import type { MediaItem, Site } from '../types'

  let {
    items,
    site,
    selected,
    onselect,
  }: {
    items: MediaItem[]
    site: Site
    selected: number
    onselect: (index: number) => void
  } = $props()

  /** Fetched data URLs by item index; a missing entry shows the gradient tile. */
  let thumbs: Record<number, string> = $state({})
  // Plain (non-reactive) so the effect never re-runs on its own bookkeeping.
  const requested = new Set<number>()

  $effect(() => {
    for (const item of items) {
      if (!item.thumbnail || requested.has(item.index)) continue
      const index = item.index
      requested.add(index)
      api.fetchThumbnail(item.thumbnail, site).then(
        (src) => (thumbs[index] = src),
        () => {},
      )
    }
  })

  const TILES = [
    'linear-gradient(135deg, #f58529, #dd2a7b)',
    'linear-gradient(135deg, #8134af, #515bd4)',
    'linear-gradient(135deg, #dd2a7b, #8134af)',
    'linear-gradient(135deg, #515bd4, #4dabf7)',
  ]
</script>

<div class="grid" role="listbox" aria-orientation="horizontal">
  {#each items as item, i (item.index)}
    <button
      type="button"
      role="option"
      class="cell"
      aria-selected={item.index === selected}
      aria-label={String(i + 1)}
      style:background={TILES[i % TILES.length]}
      onclick={() => onselect(item.index)}
    >
      {#if thumbs[item.index]}
        <img src={thumbs[item.index]} alt="" draggable="false" />
      {/if}
      {#if item.isVideo}
        <span class="play" aria-hidden="true">
          <svg viewBox="0 0 24 24"><path d="M8.5 6.5v11l9-5.5z" /></svg>
        </span>
      {/if}
      <span class="num">{i + 1}</span>
    </button>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(78px, 1fr));
    gap: 8px;
  }
  .cell {
    position: relative;
    aspect-ratio: 1;
    padding: 0;
    border: 0;
    border-radius: 10px;
    overflow: hidden;
    outline: 2px solid transparent;
    outline-offset: 2px;
    opacity: 0.78;
    transition:
      opacity 0.15s var(--ease),
      outline-color 0.15s var(--ease),
      transform 0.15s var(--ease);
  }
  .cell:hover {
    opacity: 1;
    transform: translateY(-1px);
  }
  .cell[aria-selected='true'] {
    opacity: 1;
    outline-color: rgba(255, 255, 255, 0.85);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .play {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgba(10, 12, 18, 0.45);
    backdrop-filter: blur(6px);
  }
  .play svg {
    width: 15px;
    height: 15px;
    fill: #fff;
    margin-left: 2px;
  }
  .num {
    position: absolute;
    left: 6px;
    bottom: 6px;
    min-width: 18px;
    padding: 1px 5px;
    border-radius: 6px;
    background: rgba(10, 12, 18, 0.5);
    font-size: 10.5px;
    font-weight: 600;
    color: #fff;
  }
</style>
