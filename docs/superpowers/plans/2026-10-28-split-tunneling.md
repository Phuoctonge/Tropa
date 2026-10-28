# Application Split Tunneling Implementation Plan

**Goal:** Selected Firefox traffic uses WARP while unselected Chrome traffic uses the direct connection.

**Architecture:** Portable routing preferences generate exact process-path rules in the existing mihomo runtime. The connection coordinator owns a frozen policy and mode-appropriate verification. The existing game placeholder becomes an application routing panel.

**Tech Stack:** Rust, Tauri, React, TypeScript, pinned mihomo 1.19.33.

**Spec:** ../specs/2026-10-28-split-tunneling-design.md

## Constraints

No service installation, new dependencies or changes to user-owned running processes. Keep all-computer mode available and explicit. Do not treat legacy game selection as active split routing. Manual endpoint selection remains pinned. Network failures and cleanup failures remain distinct.

## Implementation

1. Add routing preferences and bounded validated EXE paths in `src-tauri/src/config/tropa.rs`, retaining backward compatibility. Resolve Firefox Portable explicitly and preserve saved data after failed validation. Reproduce ignored routing preferences with a regression test before implementation.
2. Generate exact case-insensitive process rules in `src-tauri/src/core/warp_probe.rs`; retain `MATCH,WARP` for all mode, use `MATCH,DIRECT` for selected mode and direct general DNS. Cover escaping, empty policy refusal and unrelated paths in tests.
3. Wire preferences, active routing snapshot and authenticated loopback verification in `core/warp_connection.rs`. Add `set_tropa_routing` to `cmd/tropa.rs` and register it in `lib.rs`; reject changes while a runtime or operation is active. Preserve existing fields when the legacy command saves a game.
4. Replace game placeholder UI in `components/tropa/tropa-view.tsx` with a focused `routing-picker.tsx`; extend `types/tropa.ts`, `services/tropa.ts`, `pages/tropa.tsx` and preview props. Verify clear full/selected labels and handling of resolved executable paths.
5. Run backend/frontend tests, type checking, focused lint/format and native builds. Request read-only review and correct findings. Test named selected/unselected executable copies through real Windows TUN when no user-owned connection is active; preserve route/DNS cleanup evidence. Package a clean uniquely named portable ZIP and state any native/UI limitations accurately.

No commits or public release are requested. Continue in the existing workspace to preserve its uncommitted Tropa work.

## Result

Implemented steps 1–5. Backend (396) and frontend (50) tests, changed-file ESLint, TypeScript, web and native builds passed. Read-only review has no blocking findings. The second real Windows TUN run confirmed selected/unselected process separation for IPv4 HTTPS/TCP and complete cleanup; the first run failed outbound verification. Browser interaction, UDP and IPv6 remain unverified. A clean seven-file portable ZIP was checked against the native-tested executable. Full evidence and limits: [validation](../../validation/split-tunneling.md).
