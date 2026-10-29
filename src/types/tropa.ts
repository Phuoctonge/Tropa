export type TropaTheme = 'system' | 'light' | 'dark'

export type TropaCloseAction = 'minimizeToTray' | 'quit'

export interface TropaPreferences {
  game: string | null
  routing: TropaRouting
  selectionPolicy?: TropaSelectionPolicy
  closeAction?: TropaCloseAction
}

export interface TropaRouting {
  mode: 'all' | 'selected'
  applications: string[]
}

export type WarpAccountStatus =
  | { status: 'missing' }
  | { status: 'registered'; hasIpv6: boolean }

export type MasqueAccountStatus = WarpAccountStatus | { status: 'pending' }

export type TropaServerSelection =
  | { mode: 'automatic' }
  | { mode: 'manual'; profileId: string }

export interface TropaSearchMeasurement {
  engineBuildId: string
  stages: Record<'handshake' | 'http' | 'https' | 'udp', boolean | null>
  endpointPingMs: number | null
  tunnelPingMs: number | null
  lossPercent: number | null
  jitterMs: number | null
  samples: number
  method: string | null
  exitCountry: string | null
}

export type TropaTransportParameters =
  | { kind: 'wireGuard' }
  | { kind: 'awg'; jc: number; jmin: number; jmax: number; i1: string }
  | { kind: 'masqueH3' | 'masqueH2'; sni: string }

export interface TropaServer {
  parameters?: TropaTransportParameters
  searchMeasurement?: TropaSearchMeasurement | null
  runtimeMethod?: string | null
  networkContext?: 'same' | 'changed' | 'unknown'
  scannerContext?: 'same' | 'changed' | 'unknown'
  id: string
  endpoint: string
  transport: string
  validation?: 'verified' | 'untested' | 'rejected'
  rejectedAt?: number | null
  checkedAt: number | null
  medianMs: number | null
  jitterMs: number | null
  failed: number | null
  samples: number | null
  exitCountry: string | null
  nodeCountry?: string | null
  selectable?: boolean
  node?: string | null
  nodeLocation?: string | null
  endpointPingMs?: number | null
  tunnelPingMs?: number | null
  tunnelLossPercent?: number | null
  scannerCheckedAt?: number | null
}

export interface TropaServers {
  selection: TropaServerSelection
  servers: TropaServer[]
  policy?: TropaSelectionPolicy
  manualSelectionAllowed?: boolean
}

export interface TropaConnection {
  health?: 'healthy' | 'degraded' | 'unreachable' | 'unknown'
  routing?: TropaRouting | null
  phase:
    | 'disconnected'
    | 'searching'
    | 'starting'
    | 'verifying'
    | 'connected'
    | 'disconnecting'
    | 'failed'
  activeProfileId: string | null
  needsDisconnect: boolean
  error: string | null
  failure?: TropaFailure | null
  searchOnly?: boolean
  progress?: {
    preset: string
    addressFamily?: 'ipv4' | 'ipv6' | null
    discovered: number
    checked: number
    verified: number
    timeLimitReached?: boolean
    transports?: TropaTransportSearchProgress[]
    unavailableTuning?: { name: string; reason: string }[]
  } | null
}

export interface TropaFailure {
  stage:
    | 'registration'
    | 'scanner'
    | 'validation'
    | 'runtime'
    | 'routing'
    | 'cleanup'
  reason:
    | 'unreachable'
    | 'noCandidates'
    | 'timeout'
    | 'invalidOutput'
    | 'unsupported'
    | 'permissionDenied'
    | 'processExited'
    | 'cancelled'
    | 'cleanupPending'
    | 'policyRestricted'
  retryable: boolean
  transport: 'wireGuard' | 'awg' | 'masqueH3' | 'masqueH2' | null
}

export type TropaTransport = 'wireGuard' | 'awg' | 'masqueH3' | 'masqueH2'

export interface TropaSelectionPolicy {
  allowedTransports: TropaTransport[]
  includeNodes: string[]
  excludeNodes: string[]
  includeNodeCountries: string[]
  excludeNodeCountries: string[]
}

export interface TropaTransportSearchProgress {
  transport: 'wireGuard' | 'awg' | 'masqueH3' | 'masqueH2'
  state:
    | 'queued'
    | 'running'
    | 'completed'
    | 'cancelled'
    | 'timedOut'
    | 'failed'
  discovered: number
  checked: number
  verified: number
  rejected: number
  untested: number
  elapsedMs: number
  timings?: {
    preparationMs: number
    scanMs: number
    validationMs: number
    cleanupMs: number
  }
  failure: TropaFailure | null
}
