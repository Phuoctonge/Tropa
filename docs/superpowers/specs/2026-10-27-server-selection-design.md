# Tropa: automatic connection and manual server selection

Approved in chat on 2026-10-27. Connection is independent of game selection. This supersedes the game-first connection flow in earlier interface and project documents.

## User behavior

The main view has a connection switch and a server row. Automatic mode discovers and validates a connection without asking for transport parameters. Later connections revalidate cached profiles before scanning. Manual mode remembers a selected profile and never silently substitutes another endpoint. An unavailable manual profile offers retry or automatic mode.

The ordinary switch routes computer traffic through the owned WARP TUN, including IPv6 routing and DNS interception. Game routing is a separate future feature. Opening the application is idle: it must not start a core, install a service, enable system proxy, register upstream URL schemes or run the upstream updater. Updated by the user's explicit request: Windows portable EXE embeds a requireAdministrator manifest and requests elevation at launch. Cancelling that Windows prompt prevents launch. The previous connection-time relaunch remains a fallback for development builds without that manifest.

The server screen retains previous results during refresh, supports cancellation, and shows validated profiles with measurement age, test-target latency, loss and observed exit country where available. Transport/IP/obfuscation details are secondary. Country is an observation, not a geographic routing guarantee. Failed or unmeasured candidates cannot appear as validated. Measurements are not game RTT.

Selecting a row does not disrupt a running connection. Explicit connection first validates the candidate; a failed check preserves the current connection. A successful transition must use the existing CoreManager lifecycle and preserve the actual resulting state on partial failure. Healthy sessions are never switched for a marginal latency improvement. Game optimization is a separate future feature.

The first production iteration applies a saved server choice on the next connection. It does not replace a live tunnel or refresh the catalog through that tunnel. Users disconnect explicitly before refreshing or connecting a different profile. A failed connection disables TUN and waits for the owned core. Search children retain their shared execution reservation until Windows confirms exit, even after a stop timeout. An unconfirmed child blocks a new connection and application exit.

## Boundaries and sequencing

1. Profile catalog and selection policy: versioned portable persistence, validated transport-specific parameters, manual selection and quality ordering independent of network effects.
2. Discovery and validation: stock pinned WarpScout finds candidates; pinned mihomo validates HTTPS WARP and real UDP. Cache complete concrete AWG I1/SNI parameters, not just endpoint. Introduce AWG first using existing automatic WG registration; MASQUE requires its own EC identity registration before being offered in production.
3. Public catalog commands and server UI: secrets never enter webview. Cached observations are visibly dated; no fake connected state.
4. Connection coordinator: integrate profile choice, progress/cancellation, registration and CoreManager ownership. Portable startup must remain idle and upstream updater/network effects must be isolated before native GUI testing. Add owned TUN/UAC lifecycle, recovery and safe candidate switching before enabling the connection switch.

The stages belong to one feature, but transport validation must be proved before enabling user-facing connection. A developer probe is not a production TUN connection.

## Profile contract

Each profile has a stable local ID, endpoint and tagged transport parameters: WireGuard; AWG with concrete junk/init parameters; MASQUE HTTP/3 or HTTP/2 with SNI. Credentials are separate in data/secrets. Profile identity includes exact transport parameters: a newly generated I1 expression is a new profile. Persist the generated CPS expression exactly; its built-in random fields remain part of AWG packet construction, not a reason to rerun the profile generator. Existing endpoint-only data remains available to the legacy developer probe; it is not silently promoted to a verified full profile.

A validated observation records successful HTTPS WARP and UDP checks, sample count, failed sample count, median successful UDP DNS round-trip, jitter and timestamp, plus optional observed country. Quality comparison uses comparable observation methods: loss first, then median latency, then jitter, then stable ID. Missing measurements are never treated as zero loss/latency. First connection seeks a good working candidate in a bounded search, not a global optimum. Initial search favors AWG standard/QUIC based on local evidence, not a claim about all networks. WG and both MASQUE variants remain supported search families as their adapters are completed.

The catalog is bounded, rejects unknown versions and malformed parameters without overwriting existing data, and persists atomically before publishing selection. A selected manual profile cannot be evicted by refresh. Refresh merges successful observations; no result leaves the previous catalog intact. Resetting credentials invalidates verification and is prohibited during an active or recovering connection. Catalog observations are bound to a SHA256 fingerprint of the local registration ID; mismatches clear historical verified entries and selection before exposure or reuse. Catalog writes and credential mutation share an OS file lock across GUI and developer processes. Conflicting writes fail without changing the selection; reading previously saved results remains possible.

## Verification

Exercise manual pinning, rejection of incomplete measurements, ranking by loss before latency, complete parameter round-trip, validation failure preserving saved selection, and cancellation preserving previously saved results. Verify adapter output with the same mihomo used for connection. Live tests use zapret enabled and official WARP disconnected, with no system proxy/TUN changes for developer probes; confirm child cleanup and temporary secret deletion. No game-latency or geographic guarantee follows from these tests.
