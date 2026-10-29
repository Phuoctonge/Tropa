import { useState } from 'react'
import { createRoot } from 'react-dom/client'

import { TropaView } from '@/components/tropa/tropa-view'
import type { TropaTheme, WarpAccountStatus, TropaServers } from '@/types/tropa'

export function Preview() {
  const options = new URLSearchParams(window.location.search)
  const [theme, setTheme] = useState<TropaTheme>(
    options.get('theme') === 'light' ? 'light' : 'dark',
  )
  const [error, setError] = useState('')
  const [servers, setServers] = useState<TropaServers>({
    selection: { mode: 'automatic' },
    servers:
      options.get('servers') === 'demo'
        ? Array.from({ length: 18 }, (_, index) => ({
            id: `preview-${index}`,
            endpoint: `192.0.2.${index + 1}:2408`,
            transport: ['AWG', 'WireGuard', 'MASQUE HTTP/3', 'MASQUE HTTP/2'][
              index % 4
            ],
            parameters:
              index % 4 === 0
                ? { kind: 'awg', jc: 4, jmin: 40, jmax: 70, i1: '<b 0x01>' }
                : index % 4 === 1
                  ? { kind: 'wireGuard' }
                  : {
                      kind: index % 4 === 2 ? 'masqueH3' : 'masqueH2',
                      sni: 'www.cloudflare.com',
                    },
            validation:
              index === 4
                ? 'rejected'
                : index % 3 === 0
                  ? 'untested'
                  : 'verified',
            checkedAt:
              index % 3 === 0 ? null : Math.floor(Date.now() / 1000) - 60,
            scannerCheckedAt:
              Math.floor(Date.now() / 1000) - (index % 5 === 0 ? 3600 : 60),
            networkContext: index % 5 === 0 ? 'changed' : 'same',
            scannerContext: 'same',
            runtimeMethod: 'udp-dns-sequential-v1:1.1.1.1:53',
            nodeCountry: index < 9 ? 'RU' : 'FI',
            searchMeasurement: {
              engineBuildId: 'preview',
              stages: {
                handshake: true,
                http: true,
                https: index !== 4,
                udp: null,
              },
              endpointPingMs: 3.625 + index,
              tunnelPingMs: 24.24016 + index,
              lossPercent: index === 4 ? 40 : 0,
              jitterMs: 1.421,
              samples: 5,
              method: 'icmp-v4-sequential-v1:1.1.1.1',
              exitCountry: index % 2 ? 'DE' : null,
            },
            medianMs: 7 + index,
            jitterMs: 2,
            failed: 0,
            samples: 5,
            exitCountry: 'RU',
            node: index < 9 ? 'DME' : 'HEL',
            nodeLocation: index < 9 ? 'Moscow, RU' : 'Helsinki, FI',
            endpointPingMs: 3 + index,
            tunnelPingMs: 5 + index,
            tunnelLossPercent: 0,
          }))
        : [],
  })
  const [accountStatus, setAccountStatus] = useState<WarpAccountStatus>(
    options.get('account') === 'registered'
      ? { status: 'registered', hasIpv6: true }
      : { status: 'missing' },
  )
  const [accountError, setAccountError] = useState(
    options.get('account') === 'error'
      ? 'Не удалось прочитать регистрацию. Сбросьте её, чтобы зарегистрировать устройство заново.'
      : '',
  )
  const [busy, setBusy] = useState(false)
  const accountAction = async () => {
    setBusy(true)
    setError('')
    await new Promise((resolve) => setTimeout(resolve, 350))
    setBusy(false)
    if (options.get('operation') === 'fail') {
      setError(
        'Не удалось сбросить регистрацию. Прежняя регистрация сохранена.',
      )
      return false
    }
    setAccountStatus({ status: 'registered', hasIpv6: true })
    setAccountError('')
    return true
  }
  return (
    <TropaView
      game={null}
      routing={{
        mode: 'selected',
        applications: [String.raw`C:\Apps\Firefox\firefox.exe`],
      }}
      connection={
        options.get('connected') === 'true'
          ? {
              phase: 'connected',
              activeProfileId: 'preview-3',
              needsDisconnect: true,
              error: null,
            }
          : undefined
      }
      theme={theme}
      servers={servers}
      onSelectServer={(selection) =>
        setServers((previous) => ({ ...previous, selection }))
      }
      error={error}
      busy={busy}
      accountStatus={accountStatus}
      accountError={accountError}
      onResetAccount={accountAction}
      onTheme={setTheme}
      onAddApplication={() =>
        setError('Выбор файла доступен в приложении Tropa.')
      }
      onOpenAdvanced={() =>
        setError('Технические инструменты доступны в приложении Tropa.')
      }
    />
  )
}

const root = document.getElementById('root')
if (root) createRoot(root).render(<Preview />)
