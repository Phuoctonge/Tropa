# WARP Transport Validation Plan

> Use superpowers:executing-plans inline in the existing checkout. Scope is approved by the user's request to continue and test registration/connection on the current router. Preserve all current changes; no commits without an author.

**Goal:** Prove real WireGuard WARP HTTPS and UDP before wiring a game TUN.

**Architecture:** Feature-gated headless developer binary calls the existing registration lifecycle and a small probe orchestrator. Config owns construction of secret WireGuard settings. Existing shared execution reservation and Windows Job Object protect single-core ownership. No second production core manager is introduced.

**Tech stack:** Existing Rust/Tokio/reqwest/serde and pinned mihomo, no new dependency.

**Spec:** [Transport design](../specs/2026-10-27-warp-transport-design.md).

**Execution result:** Developer probe implemented and live attempts recorded in [validation](../../validation/warp-transport.md). Registration/activation and core startup succeeded; HTTPS and UDP transport did not. The goal of proving usable WARP transport remains unmet. Do not enable the production connection switch on this evidence.

## Task 1: Validated secret config and packet checks

Files: config/warp_account.rs; new core/warp_probe.rs; core/mod.rs.

- [ ] Add regression for a synthetic account's WireGuard settings: stored private/public keys and addresses populate the config; token/ID never occur; TUN false and loopback-only listener. Add DNS/SOCKS packet acceptance/rejection assertions. Run targeted tests before implementation.
- [ ] Implement `WarpAccount::wireguard_proxy(server: Ipv4Addr, port:u16)->serde_json::Value` only for probe/test, without exposing it as IPC. Use `probe_config(account,port)` to construct rule-based MATCH WARP with no provider/geodata downloads, no controller, DNS off and TUN off.
- [ ] Implement bounded Cloudflare trace parsing (`warp=on|plus`) and a SOCKS5 UDP DNS query/response validator with matching transaction ID, QR, RCODE and answer count. A mere process/listener start is insufficient evidence.

## Task 2: Feature-gated orchestration

Files: Cargo.toml, lib.rs, core/warp_probe.rs, bin/tropa-warp-probe.rs, manager/state.rs and manager/mod.rs (visibility-only reuse).

- [ ] Add feature tropa-probe and required-feature binary. Export only a probe entry returning a public report without credentials.
- [ ] Resolve the explicitly supplied root/core path. Hold reserve_sidecar throughout the child lifetime. Call existing ensure_registration; load the account from the same store. Write config atomically under secrets.
- [ ] Validate with `mihomo -t -d root -f secret_config`, discard raw output on failure. Spawn hidden with kill_on_drop and existing kill-on-job-close; log output goes to null. Wait boundedly for loopback listener, perform HTTPS trace and SOCKS UDP DNS, always kill/wait the owned child and remove runtime config. Report stage and cleanup failures without raw account/config errors.
- [ ] Compile and run local tests. Confirm developer binary does not invoke Tauri app startup, service install, updater, TUN, routes or system proxy.

## Task 3: Authorized live evidence

- [ ] Record core hash/version, current running cores, network adapters/default routes/system proxy before run. Use .artifacts/warp-live/data, never real records in repo.
- [ ] Run the probe against the current router, with bounded timeouts. Do not echo secrets or send them to any third-party relay. Record API, core validation/start, HTTPS WARP and UDP outcomes separately.
- [ ] Verify repeat invocation reuses registration and stops its own core; compare network state after cleanup. If direct registration/tunnel fails, diagnose the actual failing stage without adding restriction bypass or pretending success.
- [ ] Full Rust app-library tests, frontend regression/type checks as needed; read-only review of secret handling/cleanup. Write docs/validation/warp-transport.md with exact limits and next coordinator work.
