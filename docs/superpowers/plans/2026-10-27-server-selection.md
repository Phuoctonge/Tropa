# Server Selection Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Automatic WARP connection with an independent manual server choice and no required game selection.

**Architecture:** Config owns a versioned catalog and selection. Discovery produces complete profiles; mihomo validates observations. Commands expose only public server summaries; the existing CoreManager owns production lifecycle.

**Tech Stack:** Rust, Tokio, serde, existing WarpScout 0.16.1 and mihomo 1.19.33; React/TypeScript/Tauri.

**Spec:** `docs/superpowers/specs/2026-10-27-server-selection-design.md`

## Global Constraints

- No permanent service, no installation; credentials remain in portable data/secrets.
- Connection is independent of game choice; no healthy session switches automatically.
- No new dependencies, no raw credential/log output, no fake connection or ping.
- Keep existing uncommitted work; develop on codex/foundation-validation.

### Task 1: Full profile catalog and policy

Files: create `src-tauri/src/config/warp_profiles.rs`, register in `config/mod.rs`.

Interfaces: `WarpProfile`, `WarpTransport`, `WarpObservation`, `WarpCatalog`, `ServerSelection`; `WarpCatalog::{load,save,validate,candidates,select,record}`.

- [x] Add failing tests: complete AWG round-trip; manual profile stays selected despite faster alternatives; invalid selection preserves file; loss outranks RTT and incomplete validation is rejected.
- [x] Run `cargo test -p clash-verge --lib config::warp_profiles` with local Rust environment and verify expected missing-interface failure.
- [x] Implement bounded serde model, validated transport-specific endpoints/parameters, secret-free DTO data, atomic file persistence and deterministic quality ordering.
- [x] Re-run focused tests, then app library tests.

### Task 2: Profile-aware discovery and same-core validation

Files: `core/warp_discovery.rs`, `core/warp_probe.rs`, `bin/tropa-warp-probe.rs`, `config/warp_account.rs`.

Interfaces: profile-aware `run_warp_transport_probe_profile`; original endpoint probe remains compatible. `TransportProbeReport` includes a validated observation and transport. Discovery stores only profiles passing same-core HTTPS and UDP.

- [x] Add failing configuration test that AWG parameters survive scanner and core configuration exactly.
- [x] Add AWG standard and generated QUIC CPS init-parameter support to pinned scanner adapter; keep subprocess cancellation/cleanup. Copy the exact exported expression, including its built-in random fields.
- [x] Measure repeated UDP round-trips and observed exit country; save complete validated profile and reuse cached profiles before new discovery.
- [x] Verify with pinned binaries and zapret enabled; confirm no adapter/route/proxy mutation, no child/core left and temporary credentials removed. See `docs/validation/server-selection.md`: first search 18.21 s, cached validation 2.74 s, refresh 37.98 s, manual validation 2.79 s.

### Task 3: Catalog commands and manual server screen

Files: `cmd/tropa.rs`, `lib.rs`, `services/tropa.ts`, `types/tropa.ts`, `pages/tropa.tsx`, `components/tropa/tropa-view.tsx`, `components/tropa/tropa.css`.

- [x] Expose read catalog and selection commands under config-write lock, using public profile summaries only.
- [x] Display automatic/manual selection, dated comparable measurements and empty/error states. Remove game-first wording. Keep actual connection state truthful until coordinator is ready.
- [x] Run frontend typecheck/lint/tests and app library tests.

### Task 4: Production coordinator and remaining transports

Files: existing `core/manager/` lifecycle, `core/warp_registration.rs`, portable `utils/resolve/` startup, Tropa command/services/UI.

- [ ] Implement separate MASQUE EC registration and validate HTTP/3 and HTTP/2 profile adapters before exposing them as available.
- [x] Isolate upstream startup updater and idle core/network effects for portable Tropa before GUI launch. Native idle launch: responsive Tropa window, zero test cores; routes, adapters and system proxy unchanged.
- [x] Add cancellable single-owner coordinator using CoreManager, explicit progress, TUN/UAC, manual pinning and bounded cached fallback in automatic mode.
- [ ] Verify active-candidate failure preserves connection; persist only successful manual changes; retain active manual profile during refresh.
- [ ] Enable switch/refresh only when wired to this coordinator. Verify exit/tray cleanup and native Windows connection/disconnection, plus unchanged direct traffic policy.

## Execution notes

The user approved the design and requested implementation in this session. Execute inline without a further planning handoff question. Tasks remain unchecked until their behavior is implemented and verified; do not report the full feature as complete after a developer-only probe.

Tasks 1–3 verified on 2026-10-27. Task 4 now has a production coordinator, connection/refresh/disconnect IPC, live UI progress/cancellation, owned runtime startup and graceful TUN stop. The user subsequently requested mandatory administrator elevation at startup; the portable EXE now embeds `requireAdministrator`, superseding connection-time elevation as the packaged behavior. The ordinary connection uses all computer traffic; game-specific optimization remains separate. Manual selection is applied on the next connection; live replacement is deliberately not exposed. MASQUE EC identity/adapters remain outstanding.

Real elevated TUN connection, cached reconnect and route/DNS/adapter/proxy cleanup passed native smoke checks. An unavailable manually pinned profile was refused without fallback; automatic search from the same minimal resources validated another AWG profile. The final seven-file administrator test ZIP contains no user data. Its embedded manifest was verified; the no-RunAs launch returned UAC cancellation, so an accepted startup-UAC launch of this exact package, native tray interaction and clean-Windows operation remain unverified. See `docs/validation/production-connection.md`.

385 backend tests and 45 frontend tests passed after coordinator integration. A read-only independent review found unconfirmed discovery-process cleanup ownership; this was fixed with OS process supervision retaining the execution reservation, then re-reviewed without remaining material findings. Native Windows idle startup was checked against a saved route/adapter/proxy baseline. UI automation was unavailable because the local helper failed to start; this does not count as a successful click/tray/UAC test.
