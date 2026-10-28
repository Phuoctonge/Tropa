import { beforeEach, expect, test, vi } from 'vitest'

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), writeText: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: mocks.writeText,
}))

import { copyTropaDiagnostics, refreshTropaServers } from './tropa'

beforeEach(() => vi.resetAllMocks())

test('advanced tuning is an explicit refresh request and never silently enabled', async () => {
  await refreshTropaServers('masqueH3', true)
  expect(mocks.invoke).toHaveBeenLastCalledWith('refresh_tropa_servers', {
    transport: 'masqueH3', advanced: true,
  })
  await refreshTropaServers()
  expect(mocks.invoke).toHaveBeenLastCalledWith('refresh_tropa_servers', {
    transport: null, advanced: false,
  })
})

test('copies only the diagnostic command response and preserves clipboard on collection failure', async () => {
  const report = { schemaVersion: 1, connection: { phase: 'failed' } }
  mocks.invoke.mockResolvedValueOnce(report)
  await copyTropaDiagnostics()
  expect(mocks.invoke).toHaveBeenCalledWith('get_tropa_diagnostics')
  expect(mocks.writeText).toHaveBeenCalledWith(JSON.stringify(report, null, 2))
  mocks.writeText.mockClear()
  mocks.invoke.mockRejectedValueOnce(new Error('collection failed'))
  await expect(copyTropaDiagnostics()).rejects.toThrow('collection failed')
  expect(mocks.writeText).not.toHaveBeenCalled()
})
