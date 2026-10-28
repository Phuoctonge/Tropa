import type { TropaServer, TropaSelectionPolicy } from '@/types/tropa'

export const countryLabel = (country: string | null) =>
  country
    ? new Intl.DisplayNames(['ru'], { type: 'region' }).of(country)
    : undefined

export type ServerSort = 'quality' | 'latency' | 'country'

export function measurementAge(
  checkedAt: number | null | undefined,
  now = Date.now(),
  context?: TropaServer['networkContext'],
): 'recent' | 'stale' | 'unknown' {
  if (
    checkedAt == null ||
    !Number.isFinite(checkedAt) ||
    checkedAt <= 0 ||
    checkedAt * 1000 > now
  )
    return 'unknown'
  if (context === 'unknown') return 'unknown'
  if (context === 'changed') return 'stale'
  return now - checkedAt * 1000 < 15 * 60 * 1000 ? 'recent' : 'stale'
}

export function sortServers(servers: TropaServer[], sort: ServerSort) {
  const hasRuntime = (server: TropaServer) =>
    server.validation === 'verified' ||
    (server.samples != null && server.samples > 0)
  const loss = (server: TropaServer) =>
    hasRuntime(server)
      ? server.failed != null && server.samples != null && server.samples > 0
        ? (server.failed / server.samples) * 100
        : Infinity
      : (server.tunnelLossPercent ?? Infinity)
  const latency = (server: TropaServer) =>
    server.tunnelPingMs ?? server.medianMs ?? Infinity
  const qualityLatency = (server: TropaServer) =>
    hasRuntime(server)
      ? (server.medianMs ?? Infinity)
      : (server.tunnelPingMs ?? Infinity)
  const rank = (server: TropaServer) =>
    server.validation === 'rejected'
      ? 2
      : server.validation === 'untested'
        ? 1
        : 0
  const quality = (a: TropaServer, b: TropaServer) =>
    rank(a) - rank(b) ||
    loss(a) - loss(b) ||
    qualityLatency(a) - qualityLatency(b) ||
    (a.jitterMs ?? Infinity) - (b.jitterMs ?? Infinity) ||
    a.id.localeCompare(b.id)
  return [...servers].sort((a, b) => {
    if (sort === 'latency') return latency(a) - latency(b) || quality(a, b)
    if (sort === 'country')
      return (
        (countryLabel(a.exitCountry) ?? 'Я').localeCompare(
          countryLabel(b.exitCountry) ?? 'Я',
          'ru',
        ) || quality(a, b)
      )
    return quality(a, b)
  })
}

export function serverLabel(country: string | null, endpoint: string) {
  const name = country
    ? new Intl.DisplayNames(['ru'], { type: 'region' }).of(country)
    : undefined
  return name ? `${name} · ${endpoint}` : endpoint
}

export function filterServers(
  servers: TropaServer[],
  query: string,
  transport = '',
) {
  const text = query.trim().toLowerCase()
  return servers.filter(
    (server) =>
      (!transport || server.transport === transport) &&
      (!text ||
        [
          server.endpoint,
          server.node,
          server.nodeLocation,
          server.nodeCountry,
          server.exitCountry,
          countryLabel(server.exitCountry),
          countryLabel(server.nodeCountry ?? null),
        ]
          .filter(Boolean)
          .join(' ')
          .toLowerCase()
          .includes(text)),
  )
}

export const DEFAULT_SELECTION_POLICY: TropaSelectionPolicy = {
  allowedTransports: [],
  includeNodes: [],
  excludeNodes: [],
  includeNodeCountries: [],
  excludeNodeCountries: [],
}
