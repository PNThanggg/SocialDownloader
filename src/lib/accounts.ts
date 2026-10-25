import { get, writable } from 'svelte/store'
import * as api from './api'
import type { AccountStatus, LumiError, Site } from './types'

/** Connected state per site; shared by Settings and the error banners. */
export const accounts = writable<AccountStatus[]>([])

/** The Connect / Sign out in flight per site; outlives the Settings panel (a login window can stay open). */
export interface AccountAction {
  kind: 'connect' | 'signout'
  promise: Promise<AccountStatus>
}
export const accountActions = writable<Partial<Record<Site, AccountAction>>>({})

/** The last Connect / Sign out failure per site; cleared when the next action starts. */
export const accountErrors = writable<Partial<Record<Site, LumiError>>>({})

export const isConnected = (list: AccountStatus[], site: Site) => list.some((a) => a.site === site && a.connected)

function setAccount(status: AccountStatus) {
  accounts.update((list) =>
    list.some((a) => a.site === status.site)
      ? list.map((a) => (a.site === status.site ? status : a))
      : [...list, status],
  )
}

/** Reloads the list; outside Tauri every site simply stays not connected. */
export async function refreshAccounts(): Promise<void> {
  try {
    accounts.set(await api.accountsStatus())
  } catch {
    // Backend unavailable: keep what we have.
  }
}

/** Runs one account action for `site`, recording it as pending and its failure (rethrown) per site. */
function track(site: Site, kind: AccountAction['kind'], run: () => Promise<AccountStatus>): Promise<AccountStatus> {
  accountErrors.update(({ [site]: _, ...rest }) => rest)
  const promise = run().then(
    (status) => {
      setAccount(status)
      return status
    },
    (e: unknown) => {
      accountErrors.update((m) => ({ ...m, [site]: api.normalize(e) }))
      throw api.normalize(e)
    },
  )
  accountActions.update((m) => ({ ...m, [site]: { kind, promise } }))
  const clear = () =>
    accountActions.update((m) => {
      if (m[site]?.promise !== promise) return m
      const { [site]: _, ...rest } = m
      return rest
    })
  promise.then(clear, clear)
  return promise
}

/**
 * Opens the login window for `site`; resolves when it closes. A Connect already
 * waiting on its window is joined, so a second click never opens a second login.
 */
export function connectAccount(site: Site): Promise<AccountStatus> {
  const pending = get(accountActions)[site]
  if (pending?.kind === 'connect') return pending.promise
  return track(site, 'connect', () => api.connectAccount(site))
}

export function signOut(site: Site): Promise<AccountStatus> {
  const pending = get(accountActions)[site]
  if (pending?.kind === 'signout') return pending.promise
  return track(site, 'signout', async () => {
    await api.disconnectAccount(site)
    return { site, connected: false }
  })
}
