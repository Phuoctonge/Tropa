import {
  AltRouteRounded,
  ArrowBackRounded,
  ChevronRightRounded,
  PublicOutlined,
  SettingsOutlined,
} from '@mui/icons-material'
import { useState, type ReactNode } from 'react'

import type {
  TropaTheme,
  WarpAccountStatus,
  MasqueAccountStatus,
  TropaServers,
  TropaServerSelection,
  TropaConnection,
  TropaRouting,
  TropaSelectionPolicy,
  TropaTransport,
} from '@/types/tropa'
import { serverLabel } from '@/utils/tropa-server'

import { DiagnosticsSettings } from './diagnostics-settings'
import { RegistrationSettings } from './registration-settings'
import { RoutingPicker } from './routing-picker'
import { ServerPicker } from './server-picker'

import './tropa.css'

interface TropaViewProps {
  game: string | null
  theme: TropaTheme
  busy?: boolean
  preferencesUnavailable?: boolean
  routing?: TropaRouting
  onRoutingChange?: (routing: TropaRouting) => void
  onAddApplication?: (path?: string) => void
  onRemoveApplication?: (path: string) => void
  accountStatus?: WarpAccountStatus
  accountError?: string
  masqueStatus?: MasqueAccountStatus
  masqueError?: string
  onResetMasqueAccount?: () => Promise<boolean>
  onResetAccount: () => Promise<boolean>
  error?: string
  windowControls?: ReactNode
  onTheme: (theme: TropaTheme) => void
  onOpenAdvanced?: () => void
  onCopyDiagnostics?: () => Promise<boolean>
  connection?: TropaConnection
  connectionUnavailable?: boolean
  connectionBusy?: boolean
  onToggleConnection?: () => void
  onRefreshServers?: (transport?: TropaTransport, advanced?: boolean) => void
  onCancelSearch?: () => void
  onServerPanelChange?: (open: boolean) => void
  servers?: TropaServers
  serversUnavailable?: boolean
  onPolicyChange?: (policy: TropaSelectionPolicy) => void
  onSelectServer?: (selection: TropaServerSelection) => void
}

type Screen = 'home' | 'games' | 'servers' | 'settings' | 'setup'

const titles: Record<Exclude<Screen, 'home'>, string> = {
  games: 'Раздельное туннелирование',
  servers: 'Выбор сервера',
  settings: 'Настройки',
  setup: 'Подключение WARP',
}

const DEFAULT_ROUTING: TropaRouting = { mode: 'all', applications: [] }

export function TropaView({
  game,
  theme,
  busy = false,
  preferencesUnavailable = false,
  routing = DEFAULT_ROUTING,
  onRoutingChange,
  onAddApplication,
  onRemoveApplication,
  accountStatus,
  accountError,
  onResetAccount,
  masqueStatus,
  masqueError,
  onResetMasqueAccount,
  error,
  windowControls,
  onTheme,
  onOpenAdvanced,
  onCopyDiagnostics,
  servers,
  serversUnavailable = false,
  onSelectServer,
  onPolicyChange,
  connection,
  connectionUnavailable = false,
  connectionBusy = false,
  onToggleConnection,
  onRefreshServers,
  onCancelSearch,
  onServerPanelChange,
}: TropaViewProps) {
  const [screens, setScreens] = useState<Screen[]>(['home'])
  const [serverPanelOpen, setServerPanelOpen] = useState(false)
  const toggleServerPanel = (open: boolean) => {
    setServerPanelOpen(open)
    onServerPanelChange?.(open)
  }
  const screen = screens[screens.length - 1]
  const registered = accountStatus?.status === 'registered' && !accountError
  const accountLoading = !accountStatus && !accountError
  const connected = connection?.phase === 'connected'
  const routingLocked = connected || connection?.needsDisconnect === true
  const activeRouting = connection?.routing ?? routing
  const emptySelection =
    routing.mode === 'selected' && !routing.applications.length
  const connectionTitle = connectionUnavailable
    ? 'Состояние недоступно'
    : connection?.phase === 'searching' && connection.searchOnly
      ? 'Поиск серверов…'
      : {
          disconnected: 'Не подключено',
          searching: 'Ищем подключение…',
          starting: 'Запускаем туннель…',
          verifying: 'Проверяем соединение…',
          connected: {
            healthy: 'Подключено',
            degraded: 'Соединение нестабильно',
            unreachable: 'Нет передачи данных',
            unknown: 'Проверяем связь…',
          }[connection?.health ?? 'healthy'],
          disconnecting: 'Отключаем…',
          failed: 'Ошибка подключения',
        }[connection?.phase ?? 'disconnected']
  const accountLabel = accountError
    ? 'Ошибка чтения регистрации'
    : registered
      ? 'Устройство зарегистрировано'
      : accountLoading
        ? 'Чтение регистрации…'
        : 'Устройство ещё не зарегистрировано'

  const navigate = (next: Screen) => {
    setScreens((previous) => [...previous, next])
  }

  return (
    <main
      className={`tropa${serverPanelOpen ? ' tropa-expanded' : ''}`}
      data-theme={theme}
      lang="ru"
    >
      <div className="tropa-windowbar" data-tauri-drag-region="true">
        <span data-tauri-drag-region="true">Tropa</span>
        {windowControls}
      </div>
      <div className="tropa-workspace">
        <div className="tropa-shell">
          <header className="tropa-header">
            {screen === 'home' ? (
              <a
                className="tropa-brand"
                href="#"
                aria-label="Tropa — главный экран"
              >
                <svg viewBox="0 0 28 28" aria-hidden="true">
                  <path d="M4 23 13 5h5l-5 10h7l-4 8" />
                </svg>
                tropa<span className="tropa-brand-dot">.</span>
              </a>
            ) : (
              <button
                className="tropa-back"
                onClick={() => {
                  setScreens((previous) => previous.slice(0, -1))
                }}
                autoFocus
              >
                <ArrowBackRounded fontSize="small" /> Назад
              </button>
            )}
            {screen === 'home' ? (
              <button
                className="tropa-icon-button"
                aria-label="Настройки"
                onClick={() => navigate('settings')}
              >
                <SettingsOutlined />
              </button>
            ) : (
              <span className="tropa-header-label">tropa.</span>
            )}
          </header>

          {error && (
            <div className="tropa-error" role="alert">
              {error}
            </div>
          )}

          {screen === 'home' && (
            <>
              <section className="tropa-connection" aria-label="Подключение">
                <span className="tropa-eyebrow">ПОДКЛЮЧЕНИЕ WARP</span>
                <button
                  className="tropa-switch"
                  role="switch"
                  aria-checked={connected}
                  aria-label={
                    connected
                      ? 'Отключить WARP'
                      : connectionBusy
                        ? 'Отменить подключение'
                        : 'Подключить WARP'
                  }
                  aria-describedby="tropa-connection-hint"
                  disabled={
                    !onToggleConnection ||
                    connection?.phase === 'disconnecting' ||
                    (!connected &&
                      !connectionBusy &&
                      (emptySelection || preferencesUnavailable))
                  }
                  onClick={onToggleConnection}
                >
                  <span className="tropa-switch-thumb" />
                </button>
                <h1 aria-live="polite">{connectionTitle}</h1>
                <p id="tropa-connection-hint">
                  {connectionUnavailable
                    ? 'Не удалось прочитать состояние подключения.'
                    : connected
                    ? connection?.health === 'unreachable'
                      ? 'Проверка WARP не проходит. Можно отключиться и подключиться снова.'
                      : connection?.health === 'degraded'
                        ? 'Не удалось проверить связь. Повторяем проверку без смены сервера.'
                        : connection?.health === 'unknown'
                          ? 'Состояние связи ещё не подтверждено.'
                          : activeRouting.mode === 'selected'
                            ? 'Выбранные приложения через WARP, остальные напрямую'
                            : 'Трафик компьютера проходит через WARP'
                    : connectionBusy
                      ? connection?.searchOnly
                        ? 'Проверяем варианты; готовые результаты сохраняются'
                        : 'Можно отменить; первый поиск может занять несколько минут'
                      : emptySelection
                        ? 'Добавьте приложение или выберите весь компьютер'
                        : 'Выберите сервер или доверьте подбор Tropa'}
                </p>
                <button
                  className="tropa-text-button"
                  onClick={() => navigate('setup')}
                >
                  О подключении <ChevronRightRounded fontSize="small" />
                </button>
              </section>

              <section
                className="tropa-selections"
                aria-label="Выбор подключения"
              >
                <button className="tropa-row" onClick={() => navigate('games')}>
                  <span className="tropa-row-icon">
                    <AltRouteRounded />
                  </span>
                  <span className="tropa-row-copy">
                    <span>Раздельное туннелирование</span>
                    <strong>
                      {activeRouting.mode === 'selected'
                        ? 'Только выбранные приложения'
                        : 'Весь компьютер'}
                    </strong>
                  </span>
                  <ChevronRightRounded className="tropa-chevron" />
                </button>
                <button
                  className="tropa-row"
                  aria-expanded={serverPanelOpen}
                  aria-controls="tropa-server-panel"
                  onClick={() => toggleServerPanel(!serverPanelOpen)}
                >
                  <span className="tropa-row-icon">
                    <PublicOutlined />
                  </span>
                  <span className="tropa-row-copy">
                    <span>Сервер</span>
                    <strong>
                      {servers?.selection.mode === 'manual'
                        ? (() => {
                            const selectedId = servers.selection.profileId
                            const selected = servers.servers.find(
                              (server) => server.id === selectedId,
                            )
                            return selected
                              ? serverLabel(
                                  selected.exitCountry,
                                  selected.endpoint,
                                )
                              : 'Выбранный сервер'
                          })()
                        : servers
                          ? 'Автоматически'
                          : 'Загрузка…'}
                    </strong>
                  </span>
                  <ChevronRightRounded className="tropa-chevron" />
                </button>
              </section>
              <footer className="tropa-footer">
                <span className="tropa-status-dot" /> Предварительная версия
              </footer>
            </>
          )}

          {screen !== 'home' && (
            <section className="tropa-page" aria-label={titles[screen]}>
              <h1 tabIndex={-1}>{titles[screen]}</h1>
              {screen === 'games' && (
                <RoutingPicker
                  routing={routing}
                  disabled={busy || routingLocked || preferencesUnavailable}
                  connected={routingLocked}
                  legacyGame={game}
                  onChange={onRoutingChange}
                  onAdd={onAddApplication}
                  onRemove={onRemoveApplication}
                />
              )}
              {screen === 'settings' && (
                <>
                  <p className="tropa-intro">
                    Только необходимое. Остальное — под рукой.
                  </p>
                  <h2 className="tropa-section-label">Оформление</h2>
                  <div
                    className="tropa-themes"
                    role="group"
                    aria-label="Тема оформления"
                  >
                    {(
                      [
                        ['system', 'Системная'],
                        ['light', 'Светлая'],
                        ['dark', 'Тёмная'],
                      ] as const
                    ).map(([value, label]) => (
                      <button
                        key={value}
                        className={`tropa-theme-option${theme === value ? ' is-selected' : ''}`}
                        aria-pressed={theme === value}
                        onClick={() => onTheme(value)}
                        disabled={busy}
                      >
                        <span className={`tropa-theme-sample sample-${value}`}>
                          <span />
                          <span />
                        </span>
                        {label}
                      </button>
                    ))}
                  </div>
                  <h2 className="tropa-section-label">Подключение</h2>
                  <div className="tropa-selections">
                    <button
                      className="tropa-row"
                      onClick={() => navigate('setup')}
                    >
                      <span className="tropa-row-copy">
                        <strong>Профиль WARP</strong>
                        <span>{accountLabel}</span>
                      </span>
                      <ChevronRightRounded className="tropa-chevron" />
                    </button>
                  </div>
                  <h2 className="tropa-section-label">Приложение</h2>
                  <div className="tropa-card tropa-explanation">
                    <strong>Крестик скрывает Tropa в трей</strong>
                    <p>
                      Подключение продолжает работать. Для завершения выберите
                      «Выход» в меню трея.
                    </p>
                  </div>
                  {onOpenAdvanced && (
                    <button
                      className="tropa-text-button tropa-advanced"
                      onClick={onOpenAdvanced}
                    >
                      Технические инструменты{' '}
                      <ChevronRightRounded fontSize="small" />
                    </button>
                  )}
                  <DiagnosticsSettings onCopy={onCopyDiagnostics} />
                </>
              )}
              {screen === 'setup' && (
                <RegistrationSettings
                  wireGuardStatus={accountStatus}
                  wireGuardError={accountError}
                  masqueStatus={masqueStatus}
                  masqueError={masqueError}
                  busy={busy}
                  locked={
                    connected ||
                    connection?.needsDisconnect === true ||
                    connectionBusy
                  }
                  onResetWireGuard={onResetAccount}
                  onResetMasque={onResetMasqueAccount}
                />
              )}
            </section>
          )}
        </div>
        {serverPanelOpen && (
          <aside
            className="tropa-server-panel"
            id="tropa-server-panel"
            aria-label="Серверы WARP"
          >
            <div className="tropa-panel-heading">
              <div>
                <span className="tropa-eyebrow">ПОДКЛЮЧЕНИЕ</span>
                <h1>Серверы</h1>
              </div>
              <button
                className="tropa-icon-button"
                aria-label="Закрыть список серверов"
                onClick={() => toggleServerPanel(false)}
              >
                ×
              </button>
            </div>
            <ServerPicker
              catalog={servers}
              onPolicyChange={onPolicyChange}
              unavailable={serversUnavailable}
              busy={busy}
              onSelect={onSelectServer}
              onRefresh={onRefreshServers}
              connected={connected}
              searching={
                connection?.phase === 'searching' &&
                connection.searchOnly === true
              }
              progress={connection?.progress}
              onCancel={onCancelSearch}
            />
          </aside>
        )}
      </div>
    </main>
  )
}
