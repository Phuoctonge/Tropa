import { AddRounded, FolderOpenRounded } from '@mui/icons-material'

import type { TropaRouting } from '@/types/tropa'

export function RoutingPicker({
  routing,
  disabled,
  connected,
  legacyGame,
  onChange,
  onAdd,
  onAddFolder,
  onRemove,
}: {
  routing: TropaRouting
  disabled: boolean
  connected: boolean
  legacyGame?: string | null
  onChange?: (routing: TropaRouting) => void
  onAdd?: (path?: string) => void
  onAddFolder?: () => void
  onRemove?: (path: string) => void
}) {
  return (
    <>
      <p className="tropa-intro">
        Выберите, какие приложения подключать через WARP. Оптимизация серверов
        для игры — отдельная функция.
      </p>
      <fieldset
        className="tropa-routing-modes"
        disabled={disabled || !onChange}
      >
        <legend className="tropa-sr-only">Режим маршрутизации</legend>
        <label className="tropa-card">
          <input
            type="radio"
            name="tropa-routing"
            checked={routing.mode === 'all'}
            onChange={() => onChange?.({ ...routing, mode: 'all' })}
          />
          <span>Весь компьютер через WARP</span>
        </label>
        <label className="tropa-card">
          <input
            type="radio"
            name="tropa-routing"
            checked={routing.mode === 'selected'}
            onChange={() => onChange?.({ ...routing, mode: 'selected' })}
          />
          <span>Только выбранные приложения</span>
        </label>
      </fieldset>
      {routing.mode === 'selected' && (
        <p className="tropa-note">
          Остальные приложения подключаются напрямую. При пустом списке
          подключение недоступно.
        </p>
      )}
      <div className="tropa-routing-applications">
        {routing.applications.map((path) => {
          const isFolder = !path.toLowerCase().endsWith('.exe')
          return (
            <div className="tropa-card" key={path}>
              <strong>{path.split(/[\\/]/).pop()}</strong>
              <span className="tropa-cell-secondary">
                {isFolder ? 'Папка (все процессы внутри)' : 'Приложение (.exe)'}
              </span>
              <p className="tropa-path">{path}</p>
              <button
                className="tropa-text-button"
                disabled={disabled || !onRemove}
                onClick={() => onRemove?.(path)}
              >
                {isFolder ? 'Убрать папку' : 'Убрать приложение'}
              </button>
            </div>
          )
        })}
      </div>
      <div className="tropa-routing-actions">
        <button
          className="tropa-primary"
          disabled={disabled || !onAdd}
          onClick={() => onAdd?.()}
        >
          <AddRounded fontSize="small" /> Добавить приложение (.exe)
        </button>
        {onAddFolder && (
          <button
            className="tropa-secondary"
            disabled={disabled || !onAddFolder}
            onClick={() => onAddFolder()}
          >
            <FolderOpenRounded fontSize="small" /> Добавить папку с игрой
          </button>
        )}
      </div>
      {legacyGame && !routing.applications.includes(legacyGame) && (
        <button
          className="tropa-text-button"
          disabled={disabled || !onAdd}
          onClick={() => onAdd?.(legacyGame)}
        >
          Добавить ранее выбранный EXE
        </button>
      )}
      <p className="tropa-note">
        Добавление приложения или папки включает выборочный режим. Выбирайте EXE
        или папку с игрой: через туннель идут процессы, EXE которых находятся в
        этой папке и её подпапках. Процессы и службы из других папок нужно
        выбирать отдельно. Для Firefox Portable определяется внутренний
        firefox.exe.
      </p>
      <p className="tropa-note">
        {connected
          ? 'Для изменения списка или режима сначала отключитесь.'
          : 'Настройки применяются при подключении. Уже открытые соединения приложения может потребоваться перезапустить.'}
      </p>
    </>
  )
}
