import { invoke } from '@tauri-apps/api/core'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { open } from '@tauri-apps/plugin-dialog'

import type {
  TropaPreferences,
  WarpAccountStatus,
  MasqueAccountStatus,
  TropaServers,
  TropaServerSelection,
  TropaConnection,
  TropaRouting,
  TropaSelectionPolicy,
  TropaTransport,
} from '@/types/tropa'

export const getTropaPreferences = () =>
  invoke<TropaPreferences>('get_tropa_preferences')

export const setTropaGame = (game: string | null) =>
  invoke<TropaPreferences>('set_tropa_game', { game })

export const setTropaRouting = (routing: TropaRouting) =>
  invoke<TropaPreferences>('set_tropa_routing', { routing })

export const chooseTropaGame = () =>
  open({
    title: 'Выберите приложение для WARP',
    multiple: false,
    directory: false,
    filters: [{ name: 'Приложение Windows', extensions: ['exe'] }],
  })

export const getWarpAccountStatus = () =>
  invoke<WarpAccountStatus>('get_warp_account_status')

export const ensureWarpRegistration = () =>
  invoke<WarpAccountStatus>('ensure_warp_registration')

export const resetWarpRegistration = () =>
  invoke<WarpAccountStatus>('reset_warp_registration')

export const getMasqueAccountStatus = () =>
  invoke<MasqueAccountStatus>('get_masque_account_status')

export const resetMasqueRegistration = () =>
  invoke<MasqueAccountStatus>('reset_masque_registration')

export const getTropaServers = () => invoke<TropaServers>('get_tropa_servers')

export const selectTropaServer = (selection: TropaServerSelection) =>
  invoke<TropaServers>('select_tropa_server', { selection })

export const getTropaConnection = () =>
  invoke<TropaConnection>('get_tropa_connection')
export async function copyTropaDiagnostics() {
  const report = await invoke<unknown>('get_tropa_diagnostics')
  await writeText(JSON.stringify(report, null, 2))
}
export const connectTropa = () => invoke<TropaConnection>('connect_tropa')
export const disconnectTropa = () => invoke<TropaConnection>('disconnect_tropa')
export const refreshTropaServers = (transport?: TropaTransport, advanced = false) =>
  invoke<TropaConnection>('refresh_tropa_servers', {
    transport: transport ?? null,
    advanced,
  })

export const setTropaSelectionPolicy = (policy: TropaSelectionPolicy) =>
  invoke<TropaPreferences>('set_tropa_selection_policy', { policy })
