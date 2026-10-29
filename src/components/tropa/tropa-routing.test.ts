import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { expect, test } from 'vitest'

import { RoutingPicker } from './routing-picker'
import { TropaView } from './tropa-view'

test('selected application routing is explicit and is not labelled game optimization', () => {
  const html = renderToStaticMarkup(
    createElement(TropaView, {
      game: null,
      theme: 'light',
      routing: { mode: 'selected', applications: ['C:\\Firefox\\firefox.exe'] },
      onTheme: () => {},
      onResetAccount: async () => false,
    }),
  )
  expect(html).toContain('Раздельное туннелирование')
  expect(html).toContain('Выбранные приложения')
  expect(html).not.toContain('Оптимизация для игры')
})

test('connected status uses the active routing snapshot', () => {
  const html = renderToStaticMarkup(
    createElement(TropaView, {
      game: null,
      theme: 'light',
      routing: { mode: 'all', applications: [] },
      connection: {
        phase: 'connected',
        activeProfileId: 'profile',
        needsDisconnect: true,
        error: null,
        routing: {
          mode: 'selected',
          applications: ['C:\\Firefox\\firefox.exe'],
        },
      },
      onTheme: () => {},
      onResetAccount: async () => false,
    }),
  )
  expect(html).toContain('Выбранные приложения через WARP, остальные напрямую')
  expect(html).not.toContain('Трафик компьютера проходит через WARP')
})

test.each([
  ['unreachable', 'Нет передачи данных'],
  ['degraded', 'Соединение нестабильно'],
  ['unknown', 'Проверяем связь…'],
] as const)(
  'shows %s health while keeping disconnect available',
  (health, title) => {
    const html = renderToStaticMarkup(
      createElement(TropaView, {
        game: null,
        theme: 'light',
        routing: { mode: 'all', applications: [] },
        connection: {
          phase: 'connected',
          health,
          activeProfileId: 'same-profile',
          needsDisconnect: true,
          error: null,
        },
        onToggleConnection: () => {},
        onTheme: () => {},
        onResetAccount: async () => false,
      }),
    )
    expect(html).toContain(title)
    expect(html).toContain('aria-label="Отключить WARP"')
    expect(html).not.toContain('Трафик компьютера проходит через WARP')
  },
)

test('failed status reads do not present cached healthy state as confirmed connectivity', () => {
  const html = renderToStaticMarkup(
    createElement(TropaView, {
      game: null,
      theme: 'light',
      connectionUnavailable: true,
      connection: {
        phase: 'connected',
        health: 'healthy',
        activeProfileId: 'profile',
        needsDisconnect: true,
        error: null,
      },
      onTheme: () => {},
      onResetAccount: async () => false,
    }),
  )
  expect(html).toContain('Состояние недоступно')
  expect(html).toContain('Не удалось прочитать состояние подключения.')
  expect(html).not.toContain('<h1 aria-live="polite">Подключено</h1>')
  expect(html).not.toContain('Трафик компьютера проходит через WARP')
})

test('folder routing distinguishes directories and displays folder action buttons', () => {
  const html = renderToStaticMarkup(
    createElement(RoutingPicker, {
      routing: {
        mode: 'selected',
        applications: ['D:\\Games\\Dota 2', 'C:\\Apps\\test.exe'],
      },
      disabled: false,
      connected: false,
      onAddFolder: () => {},
    }),
  )
  expect(html).toContain('Dota 2')
  expect(html).toContain('Папка (все процессы внутри)')
  expect(html).toContain('Убрать папку')
  expect(html).toContain('test.exe')
  expect(html).toContain('Приложение (.exe)')
  expect(html).toContain('Убрать приложение')
  expect(html).toContain('Добавить папку с игрой')
})
