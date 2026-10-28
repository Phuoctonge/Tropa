# Automatic WARP Registration Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans in the existing approved checkout; no implementation delegation. Read-only review follows requesting-code-review.

**Goal:** Replace manual import with automatic registration and explicit reset, preserving valid credentials through failures.

**Architecture:** Config owns validated records and atomic main/pending storage. A dedicated core module owns the HTTP registrar and ensure/reset orchestration; commands hold config-write serialization. UI exposes status/reset only until the real connection coordinator consumes ensure.

**Tech Stack:** Rust, reqwest, getrandom, x25519-dalek 2.0.1, existing async-trait/Tokio; React/TypeScript and shared mutation funnel.

**Spec:** [Automatic registration](../specs/2026-10-27-automatic-warp-design.md).

User approved this design and inline implementation in the ongoing chat. No new approval gate is needed for routine implementation choices. Evidence: [validation report](../../validation/automatic-warp.md). Real first-connection integration, live Cloudflare and native smoke remain outside this registration slice.

**Execution status:** Implementation, local HTTP/filesystem/browser regressions, full Rust/frontend checks, read-only review and final native compilation completed. 366 Rust tests and 45 frontend tests pass. First-connection coordinator/tunnel integration remains pending as specified above.

## Constraints

Portable folder, plaintext local secrets, no licenses/import UI, no credentials in DTO/logs/backups. Direct Cloudflare API only; no relay or silent retry. Do not launch the native app or create real devices for tests. Keep existing working changes and branch codex/foundation-validation; no commits without configured author. Only the crypto dependency graph may change. Network tunnel and updater isolation remain separate tasks.

## Task 1: Account lifecycle and recoverable persistence

Files: config/warp_account.rs, core/warp_registration.rs, core/mod.rs under src-tauri/src.

- [x] Write a workflow regression using a temporary directory and a fake `WarpRegistrar` with create/enable counters. `ensure_registration(store, registrar)` called twice must create once; failing `reset_registration` must preserve the previous bytes; retry after enable failure must reuse pending; corrupted main must fail ensure and be explicitly recoverable with reset. Run before implementation and observe missing-symbol failure.
- [x] Config adds `load`, `save`, `load_pending`, `save_pending`, `clear_pending`, all using the existing bounded reader, validated envelope and atomic writer. Status becomes `registered` with `hasIpv6`, independent of import provenance. Reuse existing normalized-size and filesystem regressions. Remove import/remove IPC, keep parsing helpers internal where useful.
- [x] Define `#[async_trait] trait WarpRegistrar { async fn create(&self)->Result<WarpAccount>; async fn enable(&self, account:&WarpAccount)->Result<()>; }`. Ensure returns existing first. Shared create flow resumes pending; reset compares pending ID with current ID and removes already committed leftovers before creating a fresh candidate. Save pending before enable, save main only after enable, then best-effort clear pending. Errors stay fixed and sanitized.
- [x] Run focused lifecycle/config tests; verify old bytes remain authoritative and no secret is serializable through status.

## Task 2: Real direct API adapter

Files: core/warp_registration.rs, src-tauri/Cargo.toml, Cargo.lock.

- [x] Add a local HTTP regression for POST public key and PATCH activation. Synthetic response includes id/token/config.peers[0].public_key/config.interface.addresses.v4/v6. Check public key matches generated stored private key, token only in PATCH Authorization, fixed errors for non-2xx/malformed/oversized response. No external HTTP in tests. Run before adapter implementation.
- [x] Add x25519-dalek =2.0.1; fill a 32-byte secret using getrandom::fill, derive public via `x25519(secret, X25519_BASEPOINT_BYTES)`. Use base64 STANDARD. Secret model has no Debug/frontend Serialize.
- [x] Implement HttpWarpRegistrar with private endpoint; production URL fixed in spec. Client: no_proxy, redirect none, connect_timeout 5s, timeout 15s, explicit headers. Stream body through `Response::chunk` up to 64 KiB, reject status without echoing response body. Parse private API DTO and validate through config constructor. Construct the activation URL with path_segments_mut.push(id), never string interpolation into the host/query.
- [x] Re-run local HTTP/workflow tests, updating lock only for the direct crypto dependency.

## Task 3: Commands and simple settings

Files: cmd/warp_account.rs, lib.rs, services/tropa.ts, types/tropa.ts, pages/tropa.tsx, components/tropa/tropa-view.tsx, tropa-preview.tsx.

- [x] Replace import/remove commands with `ensure_warp_registration` and `reset_warp_registration`, config-write serialization, skip_all tracing, fixed error codes. get status remains. No registration call on page mount.
- [x] Add browser regression for no import/license inputs, pending registration explanation, reset confirmation/cancel, success, failure retaining status, corrupt-file recovery, busy blocking and Back navigation. Run against old UI and observe failure.
- [x] Use `WarpAccountStatus = {status:'missing'} | {status:'registered';hasIpv6:boolean}`. Services own invoke; page reset uses shared mutate identity tropa-warp-account and fulfilled status/cache only. Missing setup explains first-connection registration, registered/error setup permits confirmed reset. Remove JSON picker/import/replace/remove UI and preview operations. Keep main switch disabled until tunnel coordinator exists.
- [x] Verify frontend types/lint/tests and browser themes/compact navigation. No fake connected state.

## Task 4: Verification and evidence

- [x] Run full Rust portable lib tests and frontend typecheck/lint/test, formatting checks and git diff --check. Build portable debug without launching. Record actual results/hash/size.
- [x] Request read-only review of API request secrecy, pending recovery, saved-record reuse and UI reset semantics. Fix material findings and repeat affected checks.
- [x] Update roadmap and actual validation report. Mark old import design as superseded; record no live API/native smoke/tunnel test. Do not claim functional first connection before coordinator integration.
