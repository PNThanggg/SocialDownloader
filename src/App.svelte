<script lang="ts">
  import { onMount } from 'svelte'
  import { fly } from 'svelte/transition'
  import { accounts, connectAccount, isConnected, refreshAccounts } from './lib/accounts'
  import * as api from './lib/api'
  import ErrorBanner from './lib/components/ErrorBanner.svelte'
  import LinkInput from './lib/components/LinkInput.svelte'
  import ResultCard from './lib/components/ResultCard.svelte'
  import SettingsPanel from './lib/components/SettingsPanel.svelte'
  import SetupScreen from './lib/components/SetupScreen.svelte'
  import TitleBar from './lib/components/TitleBar.svelte'
  import ToolPrompt from './lib/components/ToolPrompt.svelte'
  import { t, type ErrorAction } from './lib/i18n'
  import { askForTool, CORE_TOOLS, installWithProgress, isInstalled, toolRequests, toolStatuses } from './lib/tools'
  import type { LumiError, MediaInfo, Tool } from './lib/types'

  let url = $state('')
  let busy = $state(false)
  let info = $state<MediaInfo | null>(null)
  let error = $state<LumiError | null>(null)
  /** The link behind `info`/`error`; retries reuse it even if the input changed. */
  let attempted = $state('')
  let lastResolved = $state('')

  /** `checking` shows the main screen while the tools are looked up; `setup` installs the missing core tools. */
  let phase = $state<'checking' | 'setup' | 'ready'>('checking')
  let missing = $state<Tool[]>([])
  let settingsOpen = $state(false)
  /** "Later" on the deno question holds for the rest of the session. */
  let denoDeclined = false

  const active = $derived(busy || info !== null || error !== null)
  const connected = $derived.by(() => {
    const e = error?.error
    return e?.code === 'login_required' && isConnected($accounts, e.site)
  })
  const denoMissing = $derived(isInstalled($toolStatuses, 'deno') === false)

  /** Startup: the setup screen runs only when yt-dlp or gallery-dl is missing. */
  async function checkTools() {
    try {
      // Only whether the files exist (instant); Settings loads the versions when opened.
      const list = await api.toolsInstalled()
      toolStatuses.set(list)
      missing = CORE_TOOLS.filter((tool) => isInstalled(list, tool) === false)
    } catch {
      // Backend unavailable: skip setup; resolve reports a missing tool with a Reinstall button.
    }
    if (missing.length > 0) {
      // Nothing read before the tools existed is worth keeping.
      error = null
      info = null
      phase = 'setup'
    } else {
      phase = 'ready'
    }
  }

  async function resolveLink(link = url.trim()) {
    // Until the tool check is done a link could fail on a tool setup is about to install.
    if (!link || busy || phase !== 'ready') return
    attempted = link
    busy = true
    error = null
    info = null
    try {
      // Without deno YouTube lists fewer formats; offer it once, and resolve either way.
      if (!denoDeclined && denoMissing && api.detectSiteLocal(link) === 'youtube') {
        if (!(await askForTool('deno'))) denoDeclined = true
      }
      info = await api.resolve(link)
      lastResolved = link
    } catch (e) {
      error = api.normalize(e)
      void refreshAccounts()
    } finally {
      busy = false
    }
  }

  /** "Get all qualities" on a limited result: install deno, then read the same link again. */
  async function installDeno() {
    if (await askForTool('deno', true)) await resolveLink(lastResolved)
  }

  /** Runs the side effect behind an error button; true when the failed step should run again. */
  async function fix(action: ErrorAction, err: LumiError): Promise<boolean> {
    const e = err.error
    switch (action) {
      case 'connect':
      case 'reconnect': {
        if (e.code !== 'login_required') return false
        const status = await connectAccount(e.site)
        await refreshAccounts()
        return status.connected
      }
      case 'update': {
        const report = await api.updateTools()
        toolStatuses.set(report.statuses)
        // A tool that could not be updated: show why instead of retrying with the old one.
        if (report.errors.length > 0) throw report.errors[0].error
        return true
      }
      case 'reinstall':
        if (e.code !== 'tool_missing') return false
        await installWithProgress(e.tool)
        return true
      case 'retry':
        return true
      case 'details':
        return false
    }
  }

  async function onResolveError(action: ErrorAction) {
    if (!error) return
    try {
      if (await fix(action, error)) await resolveLink(attempted)
    } catch (e) {
      error = api.normalize(e)
    }
  }

  async function start() {
    if (import.meta.env.DEV) {
      // Dev-only fixtures: `?demo=tiktok` or VITE_LUMI_DEMO=tiktok:vi (see lib/dev/demo.ts).
      const name = new URLSearchParams(location.search).get('demo') ?? import.meta.env.VITE_LUMI_DEMO
      if (name) {
        const { demo } = await import('./lib/dev/demo')
        const d = demo(name)
        url = d.url
        info = d.info ?? null
        error = d.error ?? null
        busy = d.busy ?? false
        if (d.info) lastResolved = d.url
        settingsOpen = d.settings ?? false
        void refreshAccounts()
        await checkTools()
        if (d.resolve) void resolveLink(d.url)
        if (d.after) setTimeout(d.after, 700)
        return
      }
    }
    void refreshAccounts()
    await checkTools()
  }

  onMount(() => {
    void start()
  })
</script>

<TitleBar onsettings={phase === 'setup' ? undefined : () => (settingsOpen = !settingsOpen)} {settingsOpen} />
{#if phase === 'setup'}
  <SetupScreen {missing} onready={() => (phase = 'ready')} />
{:else}
  <main class:active>
    <section class="hero">
      <div class="mark" aria-hidden="true">
        <svg viewBox="0 0 32 32">
          <defs>
            <linearGradient id="lumi-g" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0" stop-color="#8fbcff" />
              <stop offset="1" stop-color="#f27aa8" />
            </linearGradient>
          </defs>
          <path
            fill="url(#lumi-g)"
            d="M16 3.5c.9 5.6 3.1 8.9 7.6 10.4l1.9.6-1.9.6C19.1 16.6 16.9 19.9 16 25.5c-.9-5.6-3.1-8.9-7.6-10.4l-1.9-.6 1.9-.6C12.9 12.4 15.1 9.1 16 3.5z"
          />
          <circle cx="24.5" cy="24" r="2.2" fill="url(#lumi-g)" opacity=".85" />
        </svg>
      </div>
      <h1>{$t('hero.title')}</h1>
      <p class="sources">{$t('hero.sources')}</p>
      <div class="input">
        <LinkInput
          bind:value={url}
          {busy}
          disabled={phase !== 'ready'}
          {lastResolved}
          onsubmit={() => resolveLink()}
        />
      </div>
    </section>

    {#if active}
      <section class="results" in:fly={{ y: 10, duration: 220 }}>
        {#if error}
          <ErrorBanner {error} {connected} onaction={onResolveError} />
        {:else if busy}
          <ResultCard loading />
        {:else if info}
          {#key info}
            <ResultCard {info} accounts={$accounts} onfix={fix} oninstalldeno={denoMissing ? installDeno : undefined} />
          {/key}
        {/if}
      </section>
    {/if}
  </main>
{/if}

{#if settingsOpen}
  <SettingsPanel onclose={() => (settingsOpen = false)} />
{/if}

<!-- One tool question at a time (ffmpeg from a row, deno before a YouTube link). -->
{#each $toolRequests.slice(0, 1) as req (req)}
  <ToolPrompt tool={req.tool} autostart={req.autostart} oninstalled={req.settle} onlater={() => req.settle(null)} />
{/each}

<style>
  main {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 20px 20px;
    scrollbar-gutter: stable both-edges;
  }
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding-top: calc(24vh - 30px);
    transition: padding-top 0.35s var(--ease);
  }
  main.active .hero {
    padding-top: 14px;
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
    transition:
      opacity 0.25s var(--ease),
      transform 0.35s var(--ease),
      height 0.35s var(--ease),
      margin 0.35s var(--ease);
  }
  .mark svg {
    width: 30px;
    height: 30px;
  }
  main.active .mark {
    opacity: 0;
    height: 0;
    margin-bottom: 0;
    transform: scale(0.6);
    border-width: 0;
  }
  h1 {
    margin: 0 0 6px;
    font-size: 26px;
    font-weight: 650;
    letter-spacing: -0.01em;
    transition: font-size 0.35s var(--ease);
  }
  main.active h1 {
    font-size: 20px;
  }
  .sources {
    margin: 0;
    font-size: 12.5px;
    color: var(--muted);
    letter-spacing: 0.02em;
  }
  .input {
    width: 100%;
    max-width: 460px;
    margin-top: 22px;
  }
  main.active .input {
    margin-top: 16px;
  }
  .results {
    max-width: 460px;
    margin: 16px auto 0;
  }
</style>
