import { CheckRounded, PublicOutlined } from '@mui/icons-material'
import { Fragment, useEffect, useState } from 'react'

import type {
  TropaConnection,
  TropaServers,
  TropaServerSelection,
  TropaSelectionPolicy,
  TropaTransport,
  TropaServer,
} from '@/types/tropa'
import {
  countryLabel,
  measurementAge,
  filterServers,
  DEFAULT_SELECTION_POLICY,
  sortServers,
  searchMetrics,
  measurementMethodLabel,
  serverParameters,
  type ServerSort,
} from '@/utils/tropa-server'

import { ServerPolicy } from './server-policy'

export function ServerPicker({
  catalog,
  unavailable,
  busy,
  onSelect,
  onRefresh,
  connected,
  activeProfileId,
  searching,
  progress,
  onCancel,
  onPolicyChange,
}: {
  onPolicyChange?: (policy: TropaSelectionPolicy) => void
  catalog?: TropaServers
  unavailable: boolean
  busy: boolean
  onSelect?: (selection: TropaServerSelection) => void
  onRefresh?: (transport?: TropaTransport, advanced?: boolean) => void
  connected?: boolean
  activeProfileId?: string | null
  searching?: boolean
  progress?: TropaConnection['progress']
  onCancel?: () => void
}) {
  const [sort, setSort] = useState<ServerSort>('quality')
  const [expanded, setExpanded] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [transport, setTransport] = useState('')
  const [searchTransport, setSearchTransport] = useState<TropaTransport | ''>(
    '',
  )
  const policy = catalog?.policy ?? DEFAULT_SELECTION_POLICY
  const blockedSearch =
    searchTransport !== '' &&
    policy.allowedTransports.length > 0 &&
    !policy.allowedTransports.includes(searchTransport)
  const [now, setNow] = useState(Date.now)
  const rows = sortServers(
    filterServers(catalog?.servers ?? [], query, transport),
    sort,
    now,
  )
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 60_000)
    return () => window.clearInterval(timer)
  }, [])
  const ageLabel = (
    at: number | null | undefined,
    plural = false,
    context?: TropaServer['networkContext'],
  ) =>
    ({
      recent: 'недавно',
      stale: plural ? 'устарели' : 'устарела',
      unknown:
        context === 'unknown' ? 'контекст неизвестен' : 'возраст неизвестен',
    })[measurementAge(at, now, context)]
  return (
    <>
      {catalog?.manualSelectionAllowed === false && (
        <p role="status" className="tropa-note">
          Выбранный сервер не соответствует ограничениям или его узел ещё не
          подтверждён. Выбор сохранён. Повторите поиск, выберите другой сервер
          или измените ограничения.
        </p>
      )}
      <div className="tropa-server-toolbar">
        <button
          className="tropa-search-button"
          disabled={searching ? !onCancel : busy || !onRefresh || blockedSearch}
          onClick={
            searching
              ? onCancel
              : () => onRefresh?.(searchTransport || undefined)
          }
        >
          {searching ? 'Остановить поиск' : 'Найти серверы'}
        </button>
        <button
          className="tropa-search-button"
          disabled={
            busy ||
            searching ||
            !onRefresh ||
            blockedSearch ||
            searchTransport === 'wireGuard' ||
            (policy.allowedTransports.length > 0 &&
              policy.allowedTransports.every((kind) => kind === 'wireGuard'))
          }
          title="Подбор параметров AWG и SNI MASQUE: до 10 минут. Ручной выбор сервера сохраняется."
          onClick={() => onRefresh?.(searchTransport || undefined, true)}
        >
          Подобрать параметры
        </button>
        <label className="tropa-sort">
          Режим поиска
          <select
            aria-label="Режим поиска"
            value={searchTransport}
            disabled={busy || searching || !onRefresh}
            onChange={(event) =>
              setSearchTransport(event.target.value as TropaTransport | '')
            }
          >
            <option value="">Все разрешённые</option>
            {(
              [
                ['wireGuard', 'WireGuard'],
                ['awg', 'AWG'],
                ['masqueH3', 'MASQUE HTTP/3'],
                ['masqueH2', 'MASQUE HTTP/2'],
              ] as const
            ).map(([kind, label]) => (
              <option
                key={kind}
                value={kind}
                disabled={
                  policy.allowedTransports.length > 0 &&
                  !policy.allowedTransports.includes(kind)
                }
              >
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="tropa-sort">
          Сортировка
          <select
            value={sort}
            onChange={(event) => setSort(event.target.value as ServerSort)}
          >
            <option value="quality">По качеству</option>
            <option value="latency">По задержке</option>
            <option value="country">По стране выхода</option>
          </select>
        </label>
        {blockedSearch && (
          <p className="tropa-note" role="status">
            Этот режим исключён в ограничениях подключения. Выберите другой
            режим поиска или измените ограничения.
          </p>
        )}
        <label className="tropa-sort">
          Фильтр таблицы
          <input
            value={query}
            placeholder="Адрес, узел или страна"
            maxLength={120}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <label className="tropa-sort">
          Протокол в таблице
          <select
            value={transport}
            onChange={(event) => setTransport(event.target.value)}
          >
            <option value="">Все</option>
            {[
              ...new Set(
                (catalog?.servers ?? []).map((server) => server.transport),
              ),
            ].map((value) => (
              <option key={value}>{value}</option>
            ))}
          </select>
        </label>
      </div>
      {searching && (
        <div className="tropa-search-progress" role="status">
          <span className="tropa-search-pulse" />
          Ищем и проверяем подключения
          {progress?.preset
            ? ` · ${
                {
                  cached: 'сохранённые профили',
                  wg: 'WireGuard',
                  masque: 'MASQUE HTTP/3',
                  'masque-h2': 'MASQUE HTTP/2',
                }[progress.preset] ??
                (progress.preset.startsWith('masque-h2-')
                  ? 'MASQUE HTTP/2 · подбор SNI'
                  : progress.preset.startsWith('masque-')
                    ? 'MASQUE HTTP/3 · подбор SNI'
                    : progress.preset === 'random'
                      ? 'AWG · случайные параметры'
                      : progress.preset === 'no-init'
                        ? 'AWG · без I1'
                        : 'AWG')
              }${progress.addressFamily ? ` · ${progress.addressFamily === 'ipv6' ? 'IPv6' : 'IPv4'}` : ''}`
            : ''}
          <span>
            Найдено: {progress?.discovered ?? 0} · Проверено:{' '}
            {progress?.checked ?? 0} · Подтверждено: {progress?.verified ?? 0}.
            Можно остановить — готовые результаты сохранятся.
          </span>
        </div>
      )}
      <div className="tropa-server-utilities">
        {!searching && progress && (
          <details className="tropa-search-result">
            <summary title="Результат последнего поиска">
              Поиск · {progress.discovered}
              {progress.timeLimitReached ? ' · лимит' : ''}
            </summary>
            <p>
              Найдено: {progress.discovered} · Проверено: {progress.checked} ·
              Подтверждено: {progress.verified} · Не прошли проверку:{' '}
              {Math.max(0, progress.checked - progress.verified)} · Не
              проверено: {Math.max(0, progress.discovered - progress.checked)}.
            </p>
            {progress.timeLimitReached && (
              <p>
                Достигнут лимит времени. Часть адресов могла остаться
                непроверенной. Подтверждённые подключения сохранены.
              </p>
            )}
          </details>
        )}
        {!!progress?.transports?.length && (
          <details
            className="tropa-protocol-progress tropa-protocol-details"
            aria-label="Результаты поиска по протоколам"
          >
            <summary className="tropa-protocol-summary">
              <strong>Результаты поиска по протоколам</strong>
              <span className="tropa-protocol-chips">
                {progress.transports.map((entry) => (
                  <span key={entry.transport} className="tropa-protocol-chip">
                    {
                      {
                        wireGuard: 'WG',
                        awg: 'AWG',
                        masqueH3: 'H3',
                        masqueH2: 'H2',
                      }[entry.transport]
                    }
                    : {entry.verified} из {entry.discovered}
                  </span>
                ))}
              </span>
            </summary>
            <div className="tropa-protocol-body">
              {progress.transports.map((entry) => (
                <div key={entry.transport} className="tropa-protocol-result">
                  <strong>
                    {
                      {
                        wireGuard: 'WireGuard',
                        awg: 'AWG',
                        masqueH3: 'MASQUE H3',
                        masqueH2: 'MASQUE H2',
                      }[entry.transport]
                    }
                  </strong>
                  <span>
                    {
                      {
                        queued: searching ? 'В очереди' : 'Не запускался',
                        running: 'Проверяется',
                        completed: 'Завершён',
                        cancelled: 'Остановлен',
                        timedOut: 'Лимит времени',
                        failed: 'Ошибка',
                      }[entry.state]
                    }{' '}
                    ·{' '}
                    {(
                      Math.max(
                        entry.elapsedMs,
                        entry.timings
                          ? entry.timings.preparationMs +
                              entry.timings.scanMs +
                              entry.timings.validationMs +
                              entry.timings.cleanupMs
                          : 0,
                      ) / 1000
                    ).toFixed(1)}{' '}
                    с
                  </span>
                  <span>
                    Найдено: {entry.discovered} · Проверено: {entry.checked} ·
                    Подтверждено: {entry.verified} · Не прошли проверку:{' '}
                    {entry.rejected} · Не проверено: {entry.untested}.
                  </span>
                  {entry.timings && (
                    <span>
                      Сканирование: {(entry.timings.scanMs / 1000).toFixed(1)} с
                      · Проверка:{' '}
                      {(entry.timings.validationMs / 1000).toFixed(1)} с ·
                      Очистка: {(entry.timings.cleanupMs / 1000).toFixed(1)} с ·
                      Подготовка:{' '}
                      {(entry.timings.preparationMs / 1000).toFixed(1)} с
                    </span>
                  )}
                  {entry.failure && entry.state === 'completed' && (
                    <span>
                      Часть предыдущих проверок этого протокола завершилась без
                      результата.
                    </span>
                  )}
                </div>
              ))}
            </div>
          </details>
        )}

        <button
          className="tropa-row tropa-auto-choice"
          aria-pressed={catalog?.selection.mode === 'automatic'}
          disabled={busy || unavailable || !onSelect}
          onClick={() => onSelect?.({ mode: 'automatic' })}
        >
          <span className="tropa-row-copy">
            <strong>Автоматически</strong>
          </span>
          {catalog?.selection.mode === 'automatic' && (
            <CheckRounded className="tropa-accent" />
          )}
        </button>
        <ServerPolicy
          key={JSON.stringify(policy)}
          policy={policy}
          disabled={busy || unavailable || !catalog}
          onSave={onPolicyChange}
        />
        <details className="tropa-measurement-help">
          <summary>Как читать результаты</summary>
          <p className="tropa-panel-hint">
            В столбцах задержки и потерь показаны замеры поиска; метод указан в
            строке. Проверка mihomo использует отдельную выборку UDP DNS; старые
            записи без метода остаются неизвестными. Сортировка сначала
            учитывает статус и актуальность, затем сравнивает значения внутри
            одного известного метода: UDP DNS, затем ICMP. Это не пинг игры. «—»
            означает, что значение неизвестно. Узел Cloudflare и страна
            выходного IP могут различаться. Выбор строки не подтверждает
            работоспособность: перед включением туннеля выбранный профиль
            проверяется Tropa. Пометка «недавно» означает только возраст до 15
            минут. Смена сети требует повторной проверки, даже если замеры
            недавние. Изменение локальных адаптеров или новый запуск Tropa
            делают прежние метки контекста устаревшими. Неизвестный контекст не
            подтверждает актуальность результата.
          </p>
        </details>
      </div>
      {connected && (
        <p className="tropa-panel-hint">
          Другой сервер — новое подключение. Для поиска сначала отключитесь.
        </p>
      )}
      {unavailable ? (
        <p role="status" className="tropa-note">
          Не удалось загрузить список серверов. Сохранённый выбор не изменён.
        </p>
      ) : rows.length ? (
        <>
          <div className="tropa-table-caption">
            Найденные подключения <span>{rows.length}</span>
          </div>
          <div className="tropa-table-scroll">
            <table className="tropa-server-table">
              <caption className="tropa-sr-only">
                Серверы WARP с результатами проверки через туннель
              </caption>
              <thead>
                <tr>
                  <th scope="col">Узел / адрес</th>
                  <th scope="col">Протокол</th>
                  <th scope="col">Пинг адреса</th>
                  <th scope="col">Пинг туннеля</th>
                  <th scope="col">Потери</th>
                  <th scope="col">Страна выхода</th>
                  <th scope="col">
                    <span className="tropa-sr-only">Выбор</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {rows.map((server) => {
                  const selected =
                    catalog?.selection.mode === 'manual' &&
                    catalog.selection.profileId === server.id
                  const active = connected && activeProfileId === server.id
                  const open = expanded === server.id
                  const measurement = searchMetrics(server)
                  const isClickable =
                    !busy && !!onSelect && server.selectable !== false
                  return (
                    <Fragment key={server.id}>
                      <tr
                        className={[
                          selected ? 'is-selected' : '',
                          active ? 'is-active' : '',
                          isClickable ? 'is-clickable' : '',
                        ]
                          .filter(Boolean)
                          .join(' ')}
                        onClick={(e) => {
                          const target = e.target as HTMLElement
                          if (
                            target.closest('.tropa-endpoint') ||
                            target.closest('.tropa-select-server')
                          ) {
                            return
                          }
                          if (isClickable && !selected) {
                            onSelect?.({
                              mode: 'manual',
                              profileId: server.id,
                            })
                          }
                        }}
                      >
                        <td className="tropa-server-identity">
                          <button
                            className="tropa-endpoint"
                            aria-expanded={open}
                            aria-controls={`server-${server.id}`}
                            aria-label={`Подробности ${server.endpoint} ${server.transport}`}
                            onClick={() => setExpanded(open ? null : server.id)}
                          >
                            <strong>
                              <span aria-hidden="true">
                                {open ? '▾' : '▸'}{' '}
                              </span>
                              {server.node ?? 'Узел неизвестен'}
                              {server.nodeCountry
                                ? ` · ${server.nodeCountry}`
                                : ''}
                            </strong>
                            <span className="tropa-cell-secondary">
                              {server.endpoint}
                            </span>
                          </button>
                          {(active || selected) && (
                            <span className="tropa-server-state">
                              {active ? 'Подключён' : 'Сохранённый выбор'}
                              {active && selected ? ' · выбран' : ''}
                            </span>
                          )}
                        </td>
                        <td>
                          <span className="tropa-protocol tropa-proto-badge">
                            {server.transport.replace('MASQUE HTTP/', 'H')}
                          </span>
                          <span
                            className="tropa-cell-secondary"
                            title={
                              server.checkedAt != null
                                ? `Проверка Tropa: ${ageLabel(server.checkedAt, false, server.networkContext)}`
                                : undefined
                            }
                          >
                            {server.validation === 'untested'
                              ? 'Не проверен Tropa'
                              : server.validation === 'rejected'
                                ? 'Отклонён Tropa'
                                : measurementAge(
                                      server.checkedAt,
                                      now,
                                      server.networkContext,
                                    ) === 'recent'
                                  ? 'Подтверждён Tropa'
                                  : 'Нужна проверка'}
                          </span>
                        </td>
                        <td
                          className={[
                            'tropa-measure',
                            measurement.endpointPingMs != null
                              ? measurement.endpointPingMs < 60
                                ? 'tropa-ping-fast'
                                : measurement.endpointPingMs < 120
                                  ? 'tropa-ping-medium'
                                  : 'tropa-ping-slow'
                              : '',
                          ]
                            .filter(Boolean)
                            .join(' ')}
                        >
                          {measurement.endpointPingMs != null
                            ? `${Number(measurement.endpointPingMs.toFixed(1))} мс`
                            : '—'}
                        </td>
                        <td
                          className={[
                            'tropa-measure',
                            measurement.tunnelPingMs != null
                              ? measurement.tunnelPingMs < 60
                                ? 'tropa-ping-fast'
                                : measurement.tunnelPingMs < 120
                                  ? 'tropa-ping-medium'
                                  : 'tropa-ping-slow'
                              : '',
                          ]
                            .filter(Boolean)
                            .join(' ')}
                        >
                          {measurement.tunnelPingMs != null
                            ? `${Number(measurement.tunnelPingMs.toFixed(1))} мс`
                            : '—'}
                          <span
                            className="tropa-cell-secondary"
                            title={`${measurementMethodLabel(measurement.method)} · Замеры поиска: ${ageLabel(server.scannerCheckedAt, true, server.scannerContext)}`}
                          >
                            {measurement.method ===
                            'icmp-v4-sequential-v1:1.1.1.1'
                              ? 'ICMP'
                              : measurement.method ===
                                  'udp-dns-sequential-v1:1.1.1.1:53'
                                ? 'UDP DNS'
                                : '?'}
                            {' · '}
                            {measurementAge(
                              server.scannerCheckedAt,
                              now,
                              server.scannerContext,
                            ) === 'unknown'
                              ? '?'
                              : ageLabel(
                                  server.scannerCheckedAt,
                                  true,
                                  server.scannerContext,
                                )}
                          </span>
                        </td>
                        <td className="tropa-measure">
                          {measurement.lossPercent != null
                            ? `${Number(measurement.lossPercent.toFixed(1))}%`
                            : '—'}
                        </td>
                        <td>
                          {countryLabel(server.exitCountry) ?? 'Не определена'}
                        </td>
                        <td>
                          <button
                            className="tropa-select-server"
                            aria-label={`${selected ? 'Выбран' : 'Выбрать'} ${server.endpoint} ${server.transport}`}
                            aria-pressed={selected}
                            disabled={
                              busy ||
                              !onSelect ||
                              selected ||
                              server.selectable === false
                            }
                            onClick={() =>
                              onSelect?.({
                                mode: 'manual',
                                profileId: server.id,
                              })
                            }
                          >
                            {server.selectable === false ? (
                              'Не разрешён'
                            ) : selected ? (
                              <CheckRounded fontSize="small" />
                            ) : server.validation === 'untested' ||
                              server.validation === 'rejected' ? (
                              <span title="Выбрать для проверки">Выбрать</span>
                            ) : (
                              'Выбрать'
                            )}
                          </button>
                        </td>
                      </tr>
                      <tr
                        hidden={!open}
                        id={`server-${server.id}`}
                        className="tropa-server-details"
                      >
                        <td colSpan={7}>
                          <strong>Параметры и этапы</strong>
                          <span
                            className="tropa-cell-secondary"
                            style={{ overflowWrap: 'anywhere' }}
                          >
                            {serverParameters(server)}
                          </span>
                          {server.searchMeasurement && (
                            <span className="tropa-cell-secondary">
                              Предварительный поиск:{' '}
                              {Object.entries(server.searchMeasurement.stages)
                                .map(
                                  ([stage, result]) =>
                                    `${stage === 'handshake' ? 'Handshake' : stage.toUpperCase()}: ${result == null ? 'неизвестно' : result ? 'да' : 'нет'}`,
                                )
                                .join(' · ')}
                              <br />
                              Выборка: {measurement.samples} · Разброс:{' '}
                              {measurement.jitterMs ?? '—'} мс
                            </span>
                          )}
                          {server.checkedAt != null && (
                            <span className="tropa-cell-secondary">
                              Проверка Tropa:{' '}
                              {ageLabel(
                                server.checkedAt,
                                false,
                                server.networkContext,
                              )}
                            </span>
                          )}
                          {server.checkedAt != null && (
                            <span className="tropa-cell-secondary">
                              {measurementMethodLabel(server.runtimeMethod)}:{' '}
                              {server.medianMs ?? '—'} мс
                            </span>
                          )}
                          <span className="tropa-cell-secondary">
                            {measurementMethodLabel(measurement.method)}
                          </span>
                          {(measurement.endpointPingMs != null ||
                            measurement.tunnelPingMs != null ||
                            measurement.lossPercent != null) && (
                            <span className="tropa-cell-secondary">
                              Замеры поиска:{' '}
                              {ageLabel(
                                server.scannerCheckedAt,
                                true,
                                server.scannerContext,
                              )}
                            </span>
                          )}
                          <p>
                            Пинг адреса: {measurement.endpointPingMs ?? '—'} мс
                            · Пинг туннеля: {measurement.tunnelPingMs ?? '—'} мс
                            · Потери: {measurement.lossPercent ?? '—'}%
                          </p>
                          <p>
                            Узел:{' '}
                            {[
                              server.nodeLocation,
                              countryLabel(server.nodeCountry ?? null),
                            ]
                              .filter(Boolean)
                              .join(' · ') || 'Расположение неизвестно'}
                          </p>
                          Последняя успешная проверка Tropa:{' '}
                          {server.checkedAt != null
                            ? new Date(server.checkedAt * 1000).toLocaleString(
                                'ru-RU',
                              )
                            : 'Не выполнена'}
                          {server.rejectedAt != null && (
                            <>
                              <br />
                              Отклонён Tropa:{' '}
                              {new Date(
                                server.rejectedAt * 1000,
                              ).toLocaleString('ru-RU')}
                              . Повторите поиск для новой проверки.
                            </>
                          )}
                          {server.scannerCheckedAt && (
                            <>
                              <br />
                              Измерения поиска:{' '}
                              {new Date(
                                server.scannerCheckedAt * 1000,
                              ).toLocaleString('ru-RU')}
                            </>
                          )}
                          <br />
                          {server.validation !== 'untested' &&
                          server.validation !== 'rejected' &&
                          server.medianMs != null &&
                          server.jitterMs != null &&
                          server.failed != null &&
                          server.samples != null &&
                          server.samples > 0 ? (
                            <>
                              {measurementMethodLabel(server.runtimeMethod)}:{' '}
                              {server.medianMs} мс · Потери выборки:{' '}
                              {Math.round(
                                (server.failed / server.samples) * 100,
                              )}
                              % · Разброс задержки: {server.jitterMs} мс ·
                              Ответов: {server.samples - server.failed}/
                              {server.samples} · HTTPS через WARP подтверждён.
                            </>
                          ) : (
                            'Нет подтверждённого результата последней проверки Tropa. Результаты предварительного поиска сами по себе не подтверждают подключение через ядро Tropa.'
                          )}
                          <br />
                          Узел указан по наблюдению во время поиска. Страна
                          выходного IP может измениться.
                        </td>
                      </tr>
                    </Fragment>
                  )
                })}
              </tbody>
            </table>
          </div>
        </>
      ) : (
        <div className="tropa-empty">
          <PublicOutlined />
          <h2>
            {catalog?.servers.length
              ? 'Нет строк по фильтру'
              : 'Найдём подходящие серверы'}
          </h2>
          <p>
            Нажмите «Найти серверы». Здесь появятся найденные адреса и
            результаты их проверки через туннель.
          </p>
        </div>
      )}
    </>
  )
}
