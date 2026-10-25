<script lang="ts">
  import TitleBar from './lib/components/TitleBar.svelte'
  import { t } from './lib/i18n'
</script>

<TitleBar />
<main>
  <h1>{$t('hero.title')}</h1>
  <p>{$t('hero.sources')}</p>
</main>

<style>
  main {
    padding: 48px 24px;
    text-align: center;
  }
  h1 {
    margin: 0 0 8px;
    font-size: 28px;
    font-weight: 600;
  }
  p {
    margin: 0;
    color: var(--muted);
  }
</style>
