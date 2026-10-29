import { useRef, useState } from 'react'

import { WindowControls } from '@/components/layout/window-controller'
import { TropaView } from '@/components/tropa/tropa-view'
import { useTropaPanelWindow } from '@/hooks/use-tropa-panel-window'
import { useVerge } from '@/hooks/use-verge'
import { useWindowDecorations } from '@/hooks/use-window'
import { useLoadingOverlay } from '@/pages/_layout/hooks/use-loading-overlay'
import { mutate } from '@/services/mutate'
import {
  revalidateQueries,
  setCacheData,
  useQuery,
} from '@/services/query-client'
import {
  chooseTropaFolder,
  chooseTropaGame,
  getTropaPreferences,
  getTropaServers,
  selectTropaServer,
  getWarpAccountStatus,
  resetWarpRegistration,
  getMasqueAccountStatus,
  resetMasqueRegistration,
  setTropaRouting,
  setTropaCloseAction,
  exitApp,
  setTropaSelectionPolicy,
  getTropaConnection,
  connectTropa,
  disconnectTropa,
  refreshTropaServers,
  copyTropaDiagnostics,
} from '@/services/tropa'
import type {
  TropaTheme,
  WarpAccountStatus,
  MasqueAccountStatus,
  TropaRouting,
  TropaCloseAction,
} from '@/types/tropa'

export default function TropaPage() {
  const { verge, patchVerge } = useVerge()
  const { decorated } = useWindowDecorations()
  const [busy, setBusy] = useState(false)
  const busyRef = useRef(false)
  const [error, setError] = useState('')
  const resizeServerPanel = useTropaPanelWindow(setError)
  const [connectionRequests, setConnectionRequests] = useState(0)
  const { data: connection, error: connectionReadError } = useQuery({
    queryKey: ['tropa-connection'],
    queryFn: getTropaConnection,
    refetchInterval: 700,
    refetchIntervalInBackground: true,
    retry: false,
  })
  const connectionBusy =
    connectionRequests > 0 ||
    ['searching', 'starting', 'verifying', 'disconnecting'].includes(
      connection?.phase ?? '',
    )
  const runConnection = async (operation: typeof connectTropa) => {
    setError('')
    setConnectionRequests((count) => count + 1)
    try {
      const saved = await operation()
      await setCacheData(['tropa-connection'], saved)
    } catch (failure) {
      const detail =
        failure && typeof failure === 'object' && 'detail' in failure
          ? String(failure.detail)
          : ''
      setError(
        detail ||
          'Не удалось выполнить операцию подключения. Проверьте Интернет и запустите Tropa от имени администратора.',
      )
    } finally {
      setConnectionRequests((count) => count - 1)
      await revalidateQueries([
        ['tropa-connection'],
        ['tropa-servers'],
        ['tropa-warp-account'],
        ['tropa-masque-account'],
      ])
    }
  }
  const {
    data: preferences,
    error: readError,
    isPending,
  } = useQuery({
    queryKey: ['tropa-preferences'],
    queryFn: getTropaPreferences,
    retry: false,
  })
  useLoadingOverlay(true)

  const { data: accountStatus, error: accountReadError } = useQuery({
    queryKey: ['tropa-warp-account'],
    queryFn: getWarpAccountStatus,
    retry: false,
  })

  const { data: masqueStatus, error: masqueReadError } = useQuery({
    queryKey: ['tropa-masque-account'],
    queryFn: getMasqueAccountStatus,
    retry: false,
  })

  const {
    data: servers,
    error: serversError,
    isPending: serversPending,
  } = useQuery({
    queryKey: ['tropa-servers'],
    queryFn: getTropaServers,
    refetchInterval: connection?.phase === 'searching' ? 1000 : 15_000,
    refetchIntervalInBackground: true,
    retry: false,
  })

  const change = async (
    operation: () => Promise<unknown>,
    message = 'Не удалось сохранить настройки. Проверьте доступ к папке Tropa и выбранному файлу.',
  ) => {
    if (busyRef.current) return false
    busyRef.current = true
    setBusy(true)
    setError('')
    try {
      const result = await operation()
      return result !== false
    } catch {
      setError(message)
      return false
    } finally {
      setBusy(false)
      busyRef.current = false
    }
  }

  const saveRouting = (routing: TropaRouting) =>
    mutate(() => setTropaRouting(routing), {
      id: 'tropa-preferences',
      errorNotice: false,
      onFulfilled: (saved) => {
        void setCacheData(['tropa-preferences'], saved)
      },
    })

  const saveCloseAction = (action: TropaCloseAction) =>
    mutate(() => setTropaCloseAction(action), {
      id: 'tropa-preferences',
      errorNotice: false,
      onFulfilled: (saved) => {
        void setCacheData(['tropa-preferences'], saved)
      },
    })

  const chooseApplication = (previousPath?: string) =>
    change(async () => {
      if (!preferences) return
      const path = previousPath ?? (await chooseTropaGame())
      if (typeof path === 'string') {
        await saveRouting({
          mode: 'selected',
          applications: [...preferences.routing.applications, path],
        })
      }
    }, 'Не удалось добавить приложение. Выберите его действующий EXE; для Firefox Portable — внутренний App/Firefox64/firefox.exe.')

  const chooseFolder = () =>
    change(async () => {
      if (!preferences) return
      const path = await chooseTropaFolder()
      if (typeof path === 'string') {
        await saveRouting({
          mode: 'selected',
          applications: [...preferences.routing.applications, path],
        })
      }
    }, 'Не удалось добавить папку. Выберите существующую папку с игрой или приложением.')

  const changeAccount = (operation: () => Promise<WarpAccountStatus>) =>
    mutate(operation, {
      id: 'tropa-warp-account',
      errorNotice: false,
      onFulfilled: (saved) => {
        void setCacheData(['tropa-warp-account'], saved)
      },
      revalidate: [['tropa-warp-account'], ['tropa-servers']],
    })

  const changeMasqueAccount = () =>
    mutate<MasqueAccountStatus>(resetMasqueRegistration, {
      id: 'tropa-masque-account',
      errorNotice: false,
      onFulfilled: (saved) => {
        void setCacheData(['tropa-masque-account'], saved)
      },
      revalidate: [['tropa-masque-account'], ['tropa-servers']],
    })

  const theme = verge?.theme_mode
  const validTheme: TropaTheme =
    theme === 'light' || theme === 'dark' ? theme : 'system'

  return (
    <TropaView
      game={preferences?.game ?? null}
      routing={preferences?.routing}
      onRoutingChange={(routing) => {
        void change(() => saveRouting(routing))
      }}
      onAddApplication={(path) => {
        void chooseApplication(path)
      }}
      onAddFolder={() => {
        void chooseFolder()
      }}
      onRemoveApplication={(path) => {
        if (preferences)
          void change(() =>
            saveRouting({
              ...preferences.routing,
              applications: preferences.routing.applications.filter(
                (saved) => saved !== path,
              ),
            }),
          )
      }}
      theme={validTheme}
      closeAction={preferences?.closeAction}
      onCloseActionChange={(action) => {
        void change(() => saveCloseAction(action))
      }}
      onQuit={() => {
        void exitApp()
      }}
      connection={connection}
      connectionUnavailable={Boolean(connectionReadError)}
      onServerPanelChange={resizeServerPanel}
      onCancelSearch={() => {
        void runConnection(disconnectTropa)
      }}
      connectionBusy={connectionBusy}
      onToggleConnection={
        connectionReadError
          ? undefined
          : () => {
              void runConnection(
                connection?.needsDisconnect || connectionBusy
                  ? disconnectTropa
                  : connectTropa,
              )
            }
      }
      onRefreshServers={
        connectionBusy || connection?.phase === 'connected'
          ? undefined
          : (transport, advanced) => {
              void runConnection(() => refreshTropaServers(transport, advanced))
            }
      }
      servers={servers}
      serversUnavailable={Boolean(serversError)}
      onPolicyChange={(policy) => {
        void change(
          () =>
            mutate(() => setTropaSelectionPolicy(policy), {
              id: 'tropa-preferences',
              errorNotice: false,
              onFulfilled: (saved) => {
                void setCacheData(['tropa-preferences'], saved)
              },
              revalidate: [['tropa-servers']],
            }),
          'Не удалось сохранить ограничения. Проверьте коды узлов/стран и доступ к папке Tropa. Прежние ограничения сохранены.',
        )
      }}
      onSelectServer={
        serversPending
          ? undefined
          : (selection) => {
              void change(async () => {
                const saved = await mutate(() => selectTropaServer(selection), {
                  id: 'tropa-servers',
                  errorNotice: false,
                  onFulfilled: (saved) => {
                    void setCacheData(['tropa-servers'], saved)
                  },
                })
                if (connection?.phase === 'connected') {
                  void runConnection(async () => {
                    await disconnectTropa()
                    return await connectTropa()
                  })
                }
                return saved
              }, 'Не удалось переключить сервер. Прежний выбор сохранён.')
            }
      }
      busy={busy || connectionBusy}
      preferencesUnavailable={isPending || Boolean(readError)}
      accountStatus={accountStatus}
      masqueStatus={masqueStatus}
      masqueError={
        masqueReadError
          ? 'Не удалось прочитать регистрацию MASQUE. Сохранённые данные не изменены.'
          : undefined
      }
      onResetMasqueAccount={() =>
        change(
          changeMasqueAccount,
          'Не удалось сбросить регистрацию MASQUE. Проверьте Интернет и доступ к папке Tropa. Прежняя регистрация сохранена.',
        )
      }
      accountError={
        accountReadError
          ? 'Не удалось прочитать регистрацию. Сбросьте её, чтобы зарегистрировать устройство заново.'
          : undefined
      }
      onResetAccount={() =>
        change(
          () => changeAccount(resetWarpRegistration),
          'Не удалось сбросить регистрацию. Проверьте Интернет и доступ к папке Tropa. Прежняя регистрация сохранена.',
        )
      }
      error={
        error ||
        connection?.error ||
        (readError
          ? 'Не удалось прочитать настройки Tropa. Сохранённый выбор не изменён.'
          : '')
      }
      windowControls={decorated === false ? <WindowControls /> : undefined}
      onCopyDiagnostics={() =>
        change(
          () =>
            mutate(copyTropaDiagnostics, {
              id: 'tropa-diagnostics-clipboard',
              errorNotice: false,
            }),
          'Не удалось собрать или скопировать диагностику. Проверьте доступ к файлам Tropa и буферу обмена.',
        )
      }
      onTheme={(value) => {
        void change(() => patchVerge({ theme_mode: value }))
      }}
    />
  )
}
