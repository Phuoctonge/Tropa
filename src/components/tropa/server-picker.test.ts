import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { expect, test } from 'vitest'

import { ServerPicker } from './server-picker'

test.each(['icmp-v4-sequential-v1:1.1.1.1', 'icmp-unknown-version'])(
  'preserves exact details without claiming runtime verification for %s',
  (method) => {
    const html = renderToStaticMarkup(
      createElement(ServerPicker, {
        unavailable: false,
        busy: false,
        catalog: {
          selection: { mode: 'automatic' },
          servers: [
            {
              id: 'precise',
              endpoint: '162.159.198.2:443',
              transport: 'MASQUE HTTP/2',
              parameters: { kind: 'masqueH2', sni: 'www.google.com' },
              validation: 'untested',
              checkedAt: null,
              medianMs: null,
              jitterMs: null,
              failed: null,
              samples: null,
              exitCountry: 'DE',
              nodeCountry: 'FI',
              endpointPingMs: 1,
              tunnelPingMs: 4,
              tunnelLossPercent: 0,
              searchMeasurement: {
                engineBuildId: `source:${'a'.repeat(64)}`,
                stages: {
                  handshake: true,
                  http: true,
                  https: false,
                  udp: null,
                },
                endpointPingMs: 0.25,
                tunnelPingMs: 3.625,
                lossPercent: 0,
                jitterMs: 0.125,
                samples: 5,
                method,
                exitCountry: 'DE',
              },
            },
          ],
        },
      }),
    )
    const collapsedRow = html.split('<tbody>')[1].split('</tr>')[0]
    expect(collapsedRow).toContain('0.3 мс')
    expect(collapsedRow).toContain('3.6 мс')
    expect(collapsedRow).not.toContain('SNI:')
    expect(collapsedRow).not.toContain('3.625 мс')
    expect(html).toMatch(/<tr hidden="" id="server-precise"/)
    expect(html).toContain('0.25 мс')
    expect(html).toContain('3.625 мс')
    expect(html).toContain(
      method === 'icmp-unknown-version'
        ? `Другой метод: ${method}`
        : 'ICMP → 1.1.1.1',
    )
    expect(collapsedRow).toContain(
      method === 'icmp-unknown-version' ? '>? ·' : '>ICMP ·',
    )
    expect(html).toContain('SNI: www.google.com')
    expect(html).toContain('HTTPS: нет')
    expect(html).toContain('UDP: неизвестно')
    expect(html).toContain('Не проверен Tropa')
    expect(html).not.toContain('Подтверждён Tropa')
  },
)

test('parameter tuning is separate from regular search and disabled for a WG-only policy', () => {
  const render = (allowedTransports: ('wireGuard' | 'awg')[]) =>
    renderToStaticMarkup(
      createElement(ServerPicker, {
        unavailable: false,
        busy: false,
        onRefresh: () => undefined,
        catalog: {
          selection: { mode: 'automatic' },
          servers: [],
          policy: {
            allowedTransports,
            includeNodes: [],
            excludeNodes: [],
            includeNodeCountries: [],
            excludeNodeCountries: [],
          },
        },
      }),
    )
  const available = render(['awg'])
  expect(available).toContain('Найти серверы')
  expect(available).toContain('Подобрать параметры')
  expect(available).not.toMatch(
    /<button[^>]*disabled=""[^>]*>Подобрать параметры/,
  )
  expect(render(['wireGuard'])).toMatch(
    /<button[^>]*disabled=""[^>]*>Подобрать параметры/,
  )
})

test('a recent runtime check does not refresh old scanner measurements', () => {
  const html = renderToStaticMarkup(
    createElement(ServerPicker, {
      unavailable: false,
      busy: false,
      catalog: {
        selection: { mode: 'automatic' },
        servers: [
          {
            id: 'old-scanner',
            endpoint: '188.114.96.10:500',
            transport: 'AWG',
            validation: 'verified',
            checkedAt: Math.floor(Date.now() / 1000) - 5,
            scannerCheckedAt: Math.floor(Date.now() / 1000) - 3600,
            medianMs: 30,
            jitterMs: 1,
            samples: 5,
            failed: 0,
            exitCountry: null,
            endpointPingMs: 10,
            tunnelPingMs: 20,
            tunnelLossPercent: 0,
          },
        ],
      },
    }),
  )
  expect(html).toContain('Проверка Tropa: недавно')
  expect(html).toContain('Замеры поиска: устарели')
  expect(html).toContain('Смена сети требует повторной проверки')
})

test('recent timestamps cannot hide changed or unknown network context', () => {
  const html = renderToStaticMarkup(
    createElement(ServerPicker, {
      unavailable: false,
      busy: false,
      catalog: {
        selection: { mode: 'manual', profileId: 'changed-network' },
        servers: [
          {
            id: 'changed-network',
            endpoint: '188.114.96.10:500',
            transport: 'AWG',
            validation: 'verified',
            checkedAt: Math.floor(Date.now() / 1000) - 5,
            scannerCheckedAt: Math.floor(Date.now() / 1000) - 5,
            networkContext: 'changed',
            scannerContext: 'unknown',
            medianMs: 30,
            jitterMs: 1,
            samples: 5,
            failed: 0,
            exitCountry: null,
            endpointPingMs: 10,
            tunnelPingMs: 20,
            tunnelLossPercent: 0,
          },
        ],
      },
    }),
  )
  expect(html).toContain('Проверка Tropa: устарела')
  expect(html).toContain('Замеры поиска: контекст неизвестен')
  expect(html).not.toContain('Проверка Tropa: недавно')
  expect(html).toContain('aria-pressed="true"')
})

test.each([false, true])(
  'separates unchecked addresses from rejected ones (time limit: %s)',
  (timeLimitReached) => {
    const html = renderToStaticMarkup(
      createElement(ServerPicker, {
        unavailable: false,
        busy: false,
        searching: false,
        progress: {
          preset: 'standard',
          discovered: 70,
          checked: 48,
          verified: 46,
          timeLimitReached,
        },
      }),
    )
    expect(html).toContain('Результат последнего поиска')
    expect(html).toMatch(/Проверено:.*48/)
    expect(html).toMatch(/Подтверждено:.*46/)
    expect(html).toMatch(/Не прошли проверку:.*2/)
    expect(html).toMatch(/Не проверено:.*22/)
    expect(html.includes('Достигнут лимит времени')).toBe(timeLimitReached)
  },
)

test('connection restrictions are separate from table filters and retain a conflicting pin', () => {
  const html = renderToStaticMarkup(
    createElement(ServerPicker, {
      unavailable: false,
      busy: false,
      catalog: {
        selection: { mode: 'manual', profileId: 'old' },
        servers: [],
        manualSelectionAllowed: false,
      },
    }),
  )
  expect(html).toContain('Ограничения подключения')
  expect(html).toContain('Только страны узлов')
  expect(html).toContain('Фильтр таблицы')
  expect(html).toContain('Выбор сохранён')
  expect(html).toContain('Страна узла не задаёт страну выходного IP')
})

test('shows separate transport outcomes without treating untested candidates as failures', () => {
  const html = renderToStaticMarkup(
    createElement(ServerPicker, {
      unavailable: false,
      busy: false,
      searching: false,
      progress: {
        preset: 'wg',
        discovered: 6,
        checked: 3,
        verified: 2,
        transports: [
          {
            transport: 'wireGuard',
            state: 'completed',
            discovered: 3,
            checked: 2,
            verified: 1,
            rejected: 1,
            untested: 1,
            elapsedMs: 2400,
            timings: {
              preparationMs: 0,
              scanMs: 1200,
              validationMs: 1000,
              cleanupMs: 200,
            },
            failure: null,
          },
          {
            transport: 'awg',
            state: 'timedOut',
            discovered: 3,
            checked: 1,
            verified: 1,
            rejected: 0,
            untested: 2,
            elapsedMs: 120000,
            failure: null,
          },
        ],
      },
    }),
  )
  expect(html).toContain('Результаты поиска по протоколам')
  expect(html).toContain('Сканирование: 1.2 с')
  expect(html).toContain('Очистка: 0.2 с')
  expect(html).toMatch(/WireGuard.*Завершён/s)
  expect(html).toMatch(/AWG.*Лимит времени/s)
  expect(html).toMatch(/Не проверено:.*2/s)
})

test('a scanner-only address can be chosen for validation without a verified badge', () => {
  const html = renderToStaticMarkup(
    createElement(ServerPicker, {
      unavailable: false,
      busy: false,
      onSelect: () => {},
      catalog: {
        selection: { mode: 'automatic' },
        servers: [
          {
            id: 'scanned',
            endpoint: '[2606:4700:d0::1]:2408',
            transport: 'WireGuard',
            validation: 'untested',
            selectable: true,
            checkedAt: null,
            medianMs: null,
            jitterMs: null,
            samples: null,
            failed: null,
            exitCountry: null,
            endpointPingMs: 20,
            tunnelPingMs: 25,
            tunnelLossPercent: 0,
          },
        ],
      },
    }),
  )
  expect(html).toContain('[2606:4700:d0::1]:2408')
  expect(html).toContain('Не проверен Tropa')
  expect(html).toContain('Выбрать для проверки')
  expect(html).not.toContain('Подтверждён Tropa')
  expect(html).not.toContain('NaN')
  expect(html).not.toContain('1970')
})

test.each([
  ['masque', 'MASQUE HTTP/3'],
  ['masque-h2', 'MASQUE HTTP/2'],
])(
  'shows the active %s scan and both MASQUE policy choices',
  (preset, label) => {
    const html = renderToStaticMarkup(
      createElement(ServerPicker, {
        unavailable: false,
        busy: true,
        searching: true,
        progress: { preset, discovered: 0, checked: 0, verified: 0 },
      }),
    )
    expect(html).toContain(`Ищем и проверяем подключения · ${label}`)
    const policy = html.slice(0, html.indexOf('tropa-search-progress'))
    expect(policy).toContain('MASQUE HTTP/3')
    expect(policy).toContain('MASQUE HTTP/2')
  },
)

test('protocol search is separate from visibility filters and cannot enable excluded modes', () => {
  const html = renderToStaticMarkup(
    createElement(ServerPicker, {
      unavailable: false,
      busy: false,
      onRefresh: () => {},
      catalog: {
        selection: { mode: 'manual', profileId: 'keep-pin' },
        servers: [],
        policy: {
          allowedTransports: ['wireGuard'],
          includeNodes: [],
          excludeNodes: [],
          includeNodeCountries: [],
          excludeNodeCountries: [],
        },
      },
    }),
  )
  expect(html).toContain('Режим поиска')
  expect(html).toContain('Протокол в таблице')
  expect(html).toContain('Все разрешённые')
  const search = html.slice(html.indexOf('aria-label="Режим поиска"'))
  expect(search).toMatch(/option value="wireGuard"[^>]*>WireGuard/)
  expect(search).toMatch(/option value="awg" disabled=""/)
  expect(search).toMatch(/option value="masqueH3" disabled=""/)
  expect(search).toMatch(/option value="masqueH2" disabled=""/)
})

test('IPv6 scan progress keeps the transport label and displays the address family', () => {
  const html = renderToStaticMarkup(
    createElement(ServerPicker, {
      unavailable: false,
      busy: true,
      searching: true,
      progress: {
        preset: 'masque-h2',
        addressFamily: 'ipv6',
        discovered: 0,
        checked: 0,
        verified: 0,
      },
    }),
  )
  expect(html).toContain('Ищем и проверяем подключения · MASQUE HTTP/2 · IPv6')
})

test.each(['automatic', 'manual'] as const)(
  'marks the active profile separately from a %s selection',
  (mode) => {
    const server = {
      id: 'active',
      endpoint: '162.159.198.2:443',
      transport: 'MASQUE HTTP/2',
      validation: 'verified' as const,
      checkedAt: null,
      medianMs: null,
      jitterMs: null,
      failed: null,
      samples: null,
      exitCountry: null,
    }
    const html = renderToStaticMarkup(
      createElement(ServerPicker, {
        unavailable: false,
        busy: false,
        connected: true,
        activeProfileId: 'active',
        catalog: {
          selection:
            mode === 'automatic' ? { mode } : { mode, profileId: 'saved' },
          servers: [
            server,
            { ...server, id: 'saved', endpoint: '162.159.198.3:443' },
          ],
        },
      }),
    )
    const rows = html.match(/<tr class="[^"]*"[\s\S]*?<\/tr>/g) ?? []
    expect(rows.find((row) => row.includes('162.159.198.2:443'))).toContain(
      'Подключён',
    )
    expect(
      rows
        .find((row) => row.includes('162.159.198.3:443'))
        ?.includes('Сохранённый выбор'),
    ).toBe(mode === 'manual')
    expect(rows.find((row) => row.includes('162.159.198.3:443'))).not.toContain(
      'Подключён',
    )
  },
)
