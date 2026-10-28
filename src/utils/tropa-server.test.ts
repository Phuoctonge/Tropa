import { expect, test } from 'vitest'

import type { TropaServer } from '@/types/tropa'

import { filterServers, measurementAge, sortServers } from './tropa-server'

test('measurement age does not treat missing or future timestamps as recent results', () => {
  const now = 1_800_000_000_000
  expect(measurementAge(null, now)).toBe('unknown')
  expect(measurementAge(undefined, now)).toBe('unknown')
  expect(measurementAge(0, now)).toBe('unknown')
  expect(measurementAge(Number.NaN, now)).toBe('unknown')
  expect(measurementAge(now / 1000 + 1, now)).toBe('unknown')
  expect(measurementAge(now / 1000 - 30, now)).toBe('recent')
  expect(measurementAge(now / 1000 - 3600, now)).toBe('stale')
  expect(measurementAge(now / 1000 - 30, now, 'changed')).toBe('stale')
  expect(measurementAge(now / 1000 - 30, now, 'unknown')).toBe('unknown')
  expect(measurementAge(now / 1000 - 30, now, 'same')).toBe('recent')
})

test('server quality favours fewer losses over latency and does not mutate the catalog', () => {
  const make = (
    id: string,
    failed: number,
    samples: number,
    medianMs: number,
  ): TropaServer => ({
    id,
    failed,
    samples,
    medianMs,
    jitterMs: 2,
    endpoint: id,
    transport: 'AWG',
    exitCountry: null,
    checkedAt: 1,
  })
  const rows = [
    make('fast-lossy', 1, 5, 2),
    make('healthy', 0, 5, 30),
    make('less-loss', 1, 10, 5),
  ]
  expect(sortServers(rows, 'quality').map((row) => row.id)).toEqual([
    'healthy',
    'less-loss',
    'fast-lossy',
  ])
  expect(rows[0].id).toBe('fast-lossy')
  expect(sortServers(rows, 'latency')[0].id).toBe('fast-lossy')
})

test('table filtering does not mutate candidates or their selection eligibility', () => {
  const servers = [
    {
      id: 'a',
      endpoint: '188.114.96.1:500',
      transport: 'AWG',
      node: 'HEL',
      exitCountry: 'RU',
      selectable: false,
    },
    {
      id: 'b',
      endpoint: '188.114.96.2:500',
      transport: 'WireGuard',
      node: 'FRA',
      exitCountry: 'DE',
      selectable: true,
    },
  ] as TropaServer[]
  expect(filterServers(servers, 'hel').map((server) => server.id)).toEqual([
    'a',
  ])
  expect(
    filterServers(servers, '', 'WireGuard').map((server) => server.id),
  ).toEqual(['b'])
  expect(servers).toHaveLength(2)
  expect(servers[0].selectable).toBe(false)
})

test('quality keeps scanner-only rows below confirmed runtime results', () => {
  const base = {
    endpoint: '188.114.96.1:500',
    transport: 'AWG',
    exitCountry: null,
    jitterMs: 1,
    checkedAt: 1,
  }
  const rows = [
    {
      ...base,
      id: 'unchecked',
      validation: 'untested',
      medianMs: null,
      samples: null,
      failed: null,
      tunnelPingMs: 1,
      tunnelLossPercent: 0,
    },
    {
      ...base,
      id: 'verified',
      validation: 'verified',
      medianMs: 30,
      samples: 5,
      failed: 0,
    },
  ] as TropaServer[]
  expect(sortServers(rows, 'quality').map((row) => row.id)).toEqual([
    'verified',
    'unchecked',
  ])
})

test('quality ranks verified profiles by runtime measurements even when scanner results disagree', () => {
  const base = {
    endpoint: '188.114.96.1:500',
    transport: 'AWG',
    exitCountry: null,
    validation: 'verified',
    checkedAt: 1,
    jitterMs: 2,
    samples: 5,
  }
  const rows = [
    {
      ...base,
      id: 'scanner-favourite',
      failed: 1,
      medianMs: 5,
      tunnelPingMs: 1,
      tunnelLossPercent: 0,
    },
    {
      ...base,
      id: 'runtime-favourite',
      failed: 0,
      medianMs: 40,
      tunnelPingMs: 200,
      tunnelLossPercent: 80,
    },
  ] as TropaServer[]
  expect(sortServers(rows, 'quality').map((row) => row.id)).toEqual([
    'runtime-favourite',
    'scanner-favourite',
  ])
  expect(sortServers(rows, 'latency')[0].id).toBe('scanner-favourite')
})
