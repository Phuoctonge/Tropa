# WarpScout Discovery Implementation Plan

> Execute inline using superpowers:executing-plans in the existing checkout; preserve prior work. User approved WarpScout and delegated integration choices. No commits without an author.

**Goal:** Discover a WARP candidate using pinned WarpScout, independently validate it through mihomo and persist only a working endpoint.

**Architecture:** Config owns validated endpoint storage and minimal secret export. Core owns the bounded scanner child; the existing probe accepts a validated endpoint. No second production core manager or GUI startup.

**Tech Stack:** Rust/Tokio/serde, existing Windows Job Object/reservation, WarpScout v0.16.1 and mihomo v1.19.33, no dependency additions.

**Spec:** [Approved integration](../specs/2026-10-27-warpscout-discovery-design.md).

### Task 1: Validated candidate and secret export

Files: config/warp_endpoint.rs, config/warp_account.rs, config/mod.rs.

- [x] Add regression: accept `188.114.96.10:500`, reject private addresses, unknown ports, multiple lines and oversized stdout; `account.scout_account()` excludes id/token. Write test before implementation and observe failure.
- [x] Implement `WarpEndpoint::parse(&str)->Result<Self>`, `address()->SocketAddrV4`, `save(&Path)->Result<()>` using config's atomic writer; support only the documented IPv4 pools/ports. Save `{version:1,endpoint:"IP:port"}`. Implement minimal account export behind probe/test cfg.
- [x] Verify targeted regression and preservation on rejected endpoint.

### Task 2: Owned scanner and endpoint-aware probe

Files: core/warp_discovery.rs, core/warp_probe.rs, core/mod.rs, lib.rs, bin/tropa-warp-probe.rs.

- [x] Implement `run_warp_discovery_probe(root:PathBuf,core:PathBuf,scout:PathBuf)->Result<TransportProbeReport>`: validate paths and scanner hash, hold execution reservation, ensure/load existing registration, atomically export temporary secret, spawn pinned scan with documented arguments.
- [x] Bound stdout to 4097 bytes, discard stderr, protect child with Job Object; select exit / 180-second timeout / shared Ctrl+C state, always stop/wait then remove temporary file before interpreting output. The same cancellation state interrupts validator/HTTPS/UDP and prevents saving after cancellation. Fixed error messages only.
- [x] Make existing probe accept `WarpEndpoint` internally; existing two-argument CLI remains compatible, optional third scanner path enables discovery. Save endpoint only after successful probe. Expose safe report's actual endpoint.
- [x] Build and run full library tests; request read-only review of ownership and identity handling.

### Task 3: Real scan and report

- [x] Run pinned scanner through new orchestration with existing registration and current router; record actual outcome, registration unchanged, secret/runtime deletion and no remaining children.
- [x] Compare network state, record limits in docs/validation/warpscout-discovery.md and update roadmap. Do not claim game routing or successful transport unless measured.

Execution evidence: ../../validation/warpscout-discovery.md. Implementation and direct stock verification completed; usable WG transport was not demonstrated. GUI integration remains outside this slice.
