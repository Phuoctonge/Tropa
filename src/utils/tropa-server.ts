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

export function searchMetrics(server: TropaServer) {
  return (
    server.searchMeasurement ?? {
      endpointPingMs: server.endpointPingMs ?? null,
      tunnelPingMs: server.tunnelPingMs ?? null,
      lossPercent: server.tunnelLossPercent ?? null,
      method: null,
      samples: 0,
      jitterMs: null,
    }
  )
}

export function measurementMethodLabel(method: string | null | undefined) {
  if (method === 'icmp-v4-sequential-v1:1.1.1.1') return 'ICMP → 1.1.1.1'
  if (method === 'udp-dns-sequential-v1:1.1.1.1:53')
    return 'UDP DNS → 1.1.1.1:53'
  return method ? `Другой метод: ${method}` : 'Метод неизвестен'
}

export function serverParameters(server: TropaServer) {
  const parameters = server.parameters
  if (!parameters) return 'Параметры неизвестны'
  if (parameters.kind === 'wireGuard') return 'Стандартный WireGuard'
  if (parameters.kind === 'awg')
    return `Jc=${parameters.jc} · Jmin=${parameters.jmin} · Jmax=${parameters.jmax} · I1=${parameters.i1 || 'нет'}`
  return `SNI: ${parameters.sni}`
}

export function sortServers(
  servers: TropaServer[],
  sort: ServerSort,
  now = Date.now(),
) {
  const metric = (server: TropaServer) => {
    const runtime =
      server.runtimeMethod === 'udp-dns-sequential-v1:1.1.1.1:53' &&
      server.samples != null &&
      server.samples >= 3
    const search = server.searchMeasurement
    const icmp =
      search?.method === 'icmp-v4-sequential-v1:1.1.1.1' && search.samples >= 3
    return {
      age: measurementAge(
        runtime || !search ? server.checkedAt : server.scannerCheckedAt,
        now,
        runtime || !search ? server.networkContext : server.scannerContext,
      ),
      group: runtime ? 0 : icmp ? 1 : search?.method ? 2 : 3,
      loss: runtime
        ? server.failed != null
          ? (server.failed / server.samples!) * 100
          : Infinity
        : icmp
          ? (search.lossPercent ?? Infinity)
          : Infinity,
      latency: runtime
        ? (server.medianMs ?? Infinity)
        : icmp
          ? (search.tunnelPingMs ?? Infinity)
          : Infinity,
      jitter: runtime
        ? (server.jitterMs ?? Infinity)
        : icmp
          ? (search.jitterMs ?? Infinity)
          : Infinity,
      samples: runtime ? server.samples! : icmp ? search.samples : 0,
    }
  }
  const rank = (server: TropaServer) =>
    server.validation === 'rejected'
      ? 2
      : server.validation === 'untested'
        ? 1
        : 0
  const quality = (a: TropaServer, b: TropaServer) => {
    const left = metric(a)
    const right = metric(b)
    const ageRank = { recent: 0, unknown: 1, stale: 2 }
    return (
      rank(a) - rank(b) ||
      ageRank[left.age] - ageRank[right.age] ||
      left.group - right.group ||
      (sort === 'latency'
        ? left.latency - right.latency
        : left.loss - right.loss) ||
      left.loss - right.loss ||
      left.latency - right.latency ||
      left.jitter - right.jitter ||
      right.samples - left.samples ||
      a.id.localeCompare(b.id)
    )
  }
  return [...servers].sort((a, b) => {
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
