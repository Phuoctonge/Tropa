# Expanded Server Panel Implementation Plan

> Execute inline using superpowers:executing-plans; the user approved implementation.

**Goal:** Expand the existing window to show independently selectable, verified WARP alternatives in a table.

**Architecture:** Keep the home controls mounted alongside a server panel. The page owns native window resizing; ServerPicker owns presentation and sorting. Discovery reads the pinned WarpScout plain report, retains working endpoint candidates, validates them with mihomo, and saves each success atomically. Fast connection still returns after one working profile.

**Tech stack:** Existing React/TypeScript, Tauri window API, Rust/Tokio and pinned WarpScout.

- [x] Add regression coverage for report parsing, excluding torn-down results and preserving AWG parameters; run failing tests first.
- [x] Read bounded scanner report with `-P`, retain all working addresses up to 128 profiles within a ten-minute extended-search deadline, and retain partial results on cancellation. Keep fast connection's existing deadline, manual selection and runtime ownership unchanged. Add optional validated node metadata and distinct endpoint/tunnel ping and loss with backward-compatible defaults.
- [x] Replace cards with sortable table, details, distinct exit-country/node labels, explicit search/cancel controls, and progress. Poll atomically saved results without holding the config lock for the full scan.
- [x] Add same-window expansion/restoration through the existing window provider; constrain resizing to the monitor and preserve maximized/fullscreen state.
- [x] Run Rust tests, frontend tests/typecheck/lint/build, inspect a browser rendering if available, and build a clean portable test ZIP. Verify GUI subsystem, administrator manifest and archive allowlist/hashes.

No game-specific behavior, live tunnel switching, MASQUE implementation or permanent service is added.

Evidence and verification limits: `docs/validation/expanded-server-panel.md`. Live refresh confirmed 46 profiles; backend 388 tests and frontend 46 tests passed. Native panel interaction remains unverified because the UI automation runtime failed to initialize.
