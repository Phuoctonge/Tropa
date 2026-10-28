import { useState } from 'react'
import { createRoot } from 'react-dom/client'

import { TropaView } from '@/components/tropa/tropa-view'
import type { TropaTheme, WarpAccountStatus, TropaServers } from '@/types/tropa'

export function Preview() {
  const [theme, setTheme] = useState<TropaTheme>('system')
  const [error, setError] = useState('')
  const options = new URLSearchParams(window.location.search)
  const [servers, setServers] = useState<TropaServers>({
    selection: { mode: 'automatic' },
    servers:
      options.get('servers') === 'demo'
        ? Array.from({ length: 18 }, (_, index) => ({
            id: `preview-${index}`,
            endpoint: `192.0.2.${index + 1}:2408`,
            transport: index % 3 ? 'AWG' : 'WireGuard',
            checkedAt: 1793120400,
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
