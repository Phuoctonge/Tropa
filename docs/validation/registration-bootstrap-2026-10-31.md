# Automatic registration bootstrap — 2026-10-31

Tropa previously registered only through direct HTTPS. WarpScout v0.16.1's
registration fallback was absent even though its scanning transports had been
adapted. A first installation could therefore fail before discovering servers.

The owned worker now supports registration-only framed pipe requests. When the
direct API availability check fails, Rust starts the pinned worker, which tries
supported AWG masking profiles and then WG with WarpScout's public initial
identity. Discovery uses serial sessions, primary ports and sampled IPv4 pools,
with a 12-second budget per profile. It confirms HTTPS API access before sending
any enrollment. API DNS is resolved inside the tunnel and TLS verification stays
enabled. No hosted relay, system proxy, OS routes or system TUN are used.

Rust retains personal key generation, POST/pending/PATCH persistence and handling
of rejected/revoked identities. Confirmed POST results are saved before activation.
An ambiguous POST response is not replayed automatically. WG and MASQUE preparation
share the temporary worker; search timing starts afterward. Manual connections
prepare only their selected transport's family. Cancelled exchanges drop their
worker rather than allowing the next request to reuse a partial pipe frame.

## Observed verification

- All Go packages passed `go test ./...`, including local TLS enrollment forwarding,
  API-rejection/no-replay behavior and cancellation of bootstrap endpoint discovery.
- All 481 ordinary Rust library tests passed; three native tests remain opt-in.
  The new cancellation regression verifies cancelled search progress and absence
  of a registration file when cancelled before enrollment.
- The opt-in Rust test
  `blocked_direct_api_registers_and_persists_through_owned_worker` passed in 4.86s
  on the final verification run.
  Its direct HTTP client's DNS resolver deliberately rejects every lookup. Starting
  with an empty temporary data directory, real WG and MASQUE devices were enrolled
  through the source-bound worker. WG pending cleanup, reuse without replacement,
  unchanged WG data during MASQUE enrollment and worker OS exit were confirmed.
  Temporary personal files were removed; no credentials are included here.
- `cargo check --locked -p clash-verge --lib --features portable --offline` passed
  with existing dead-code warnings. Repository-wide Clippy is not clean; its
  run reported existing lint/style findings together with findings in changed
  code. Targeted corrections followed, but a clean final Clippy run has not
  been established.
- `git diff --check` and source-bound worker verification passed.

The tested worker snapshot is
`dc071aba022d1059a70b7c6195982769b19794ed04b0b47c582818ba5511bee8`;
its executable SHA-256 is
`bc0422c4d4a6ce92ebdd5f00268eb2e6f24d05b48dfec6cd8387bf327bca6542`.
Build records are under `.artifacts/engine-build/registration-bootstrap-20261031-final`.

These tests used the current network. Router zapret state was not changed or
independently established. Forcing the Rust direct path to fail verifies fallback
selection, but does not establish availability on every provider or simultaneous
bootstrap for 300 clients.

The portable application was subsequently rebuilt with the corrected Tropa tray,
application and frontend icons. The verified archive is
`.artifacts/portable/Tropa-registration-bootstrap-icons-20261031.zip`, with build
evidence under `.artifacts/validation/registration-bootstrap-icons-20261031`.
Binary inspection confirmed all three Windows tray modes contain the new artwork
and none contains the previous tray ICO bytes. The rebuilt frontend favicon
matches the Tropa application icon. The application itself was not launched as
part of this packaging verification.
