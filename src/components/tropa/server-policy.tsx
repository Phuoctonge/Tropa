import { useState } from 'react'

import type { TropaSelectionPolicy } from '@/types/tropa'
import { DEFAULT_SELECTION_POLICY } from '@/utils/tropa-server'

export function ServerPolicy({
  policy,
  disabled,
  onSave,
}: {
  policy: TropaSelectionPolicy
  disabled: boolean
  onSave?: (policy: TropaSelectionPolicy) => void
}) {
  const [nodes, setNodes] = useState(policy.includeNodes.join(', '))
  const [excludeNodes, setExcludeNodes] = useState(
    policy.excludeNodes.join(', '),
  )
  const [countries, setCountries] = useState(
    policy.includeNodeCountries.join(', '),
  )
  const [excludeCountries, setExcludeCountries] = useState(
    policy.excludeNodeCountries.join(', '),
  )
  const [transports, setTransports] = useState(
    policy.allowedTransports.length
      ? policy.allowedTransports
      : ['wireGuard', 'awg', 'masqueH3', 'masqueH2'],
  )
  const [error, setError] = useState('')
  const save = () => {
    setError('')
    const codes = (text: string) => [
      ...new Set(
        text
          .toUpperCase()
          .split(/[\s,;]+/)
          .filter(Boolean),
      ),
    ]
    const include = codes(nodes)
    const exclude = codes(excludeNodes)
    const includeCountries = codes(countries)
    const excludedCountries = codes(excludeCountries)
    for (const [list, width] of [
      [include, 3],
      [exclude, 3],
      [includeCountries, 2],
      [excludedCountries, 2],
    ] as const) {
      if (
        list.length > 64 ||
        list.some((code) => !new RegExp(`^[A-Z]{${width}}$`).test(code))
      ) {
        setError('Укажите до 64 кодов: узлы — три буквы, страны — две.')
        return
      }
    }
    if (
      !transports.length ||
      [
        [include, exclude],
        [includeCountries, excludedCountries],
      ].some(
        ([positive, negative]) =>
          positive.length > 0 &&
          positive.every((code) => negative.includes(code)),
      )
    ) {
      setError(
        'Ограничения исключают все разрешённые варианты. Измените список.',
      )
      return
    }
    onSave?.({
      allowedTransports:
        transports as TropaSelectionPolicy['allowedTransports'],
      includeNodes: include,
      excludeNodes: exclude,
      includeNodeCountries: includeCountries,
      excludeNodeCountries: excludedCountries,
    })
  }
  return (
    <details className="tropa-server-policy">
      <summary>Ограничения подключения</summary>
      <p className="tropa-panel-hint">
        Применяются к поиску и следующему подключению. Пустой список узлов или
        стран разрешает все; исключения имеют приоритет. Страна узла не задаёт
        страну выходного IP.
      </p>
      <fieldset disabled={disabled || !onSave}>
        <legend>Режимы подключения</legend>
        {(
          [
            ['wireGuard', 'WireGuard'],
            ['awg', 'AWG'],
            ['masqueH3', 'MASQUE HTTP/3'],
            ['masqueH2', 'MASQUE HTTP/2'],
          ] as const
        ).map(([kind, label]) => (
          <label key={kind} className="tropa-policy-transport">
            <input
              type="checkbox"
              checked={transports.includes(kind)}
              onChange={(event) =>
                setTransports(
                  event.target.checked
                    ? [...transports, kind]
                    : transports.filter((value) => value !== kind),
                )
              }
            />
            {label}
          </label>
        ))}
        <div className="tropa-policy-grid">
          <label>
            Только узлы
            <input
              value={nodes}
              maxLength={2048}
              placeholder="HEL, ARN"
              onChange={(event) => setNodes(event.target.value)}
            />
          </label>
          <label>
            Исключить узлы
            <input
              value={excludeNodes}
              maxLength={2048}
              placeholder="DME"
              onChange={(event) => setExcludeNodes(event.target.value)}
            />
          </label>
          <label>
            Только страны узлов
            <input
              value={countries}
              maxLength={2048}
              placeholder="DE, NL"
              onChange={(event) => setCountries(event.target.value)}
            />
          </label>
          <label>
            Исключить страны узлов
            <input
              value={excludeCountries}
              maxLength={2048}
              placeholder="RU"
              onChange={(event) => setExcludeCountries(event.target.value)}
            />
          </label>
        </div>
        {error && <p role="alert">{error}</p>}
        <button className="tropa-select-server" onClick={save}>
          Сохранить ограничения
        </button>
        <button
          className="tropa-select-server"
          onClick={() => onSave?.(DEFAULT_SELECTION_POLICY)}
        >
          Разрешить все
        </button>
      </fieldset>
    </details>
  )
}
