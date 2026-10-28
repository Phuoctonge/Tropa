# Tropa Portable Lifecycle Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Windows portable builds own their child core, reject service management, hide the main window to the tray on close, and stop the core through controlled tray Exit.

**Architecture:** Keep the upstream lifecycle and cleanup owners. Apply the portable policy before service waiting, core-stop side effects, and privileged service dispatch. Prevent repeated window-close requests from bypassing asynchronous cleanup.

**Tech Stack:** Rust, Tauri 2, existing mihomo sidecar and Windows Job Object.

**Spec:** ../specs/2026-10-27-game-warp-design.md, approved portable requirements.

## Global Constraints

- Windows portable is selected only by the existing `portable` Cargo feature.
- Preserve administrator/TUN checks and controlled-stop failure cancellation.
- No service installation, removal, repair, or adoption in portable mode.
- Keep default builds unchanged; add no dependencies.
- The approved derivative brief defines local scope; this is not an upstream contribution.
- No commits until a Git author identity is available.

### Task 1: Standalone startup and service protection

Files: `src-tauri/src/core/manager/lifecycle.rs`, `src-tauri/src/core/service.rs`, `src-tauri/src/cmd/service.rs`.

- [x] Add a portable startup regression covering ready, absent, broken, checking and pending-service states, with TUN enabled and disabled. Every case must select `StartupDecision::Sidecar`.
- [x] Run `cargo test --locked -p clash-verge --lib --features clippy,portable portable_startup`; observe the ready-service case fail against upstream selection.
- [x] Select portable sidecar before service waiting in `prepare_startup`, accept sidecar at detection, and skip service handoff. Keep normal startup tests enabled for normal builds.
- [x] Add `ensure_service_management_available() -> anyhow::Result<()>` at service dispatch and before command/lifecycle side effects. Reject Windows portable with a clear error.
- [x] Verify rejection before configuration initialization with portable command tests; no real privileged action may be invoked in tests.

### Task 2: Main-window close and controlled tray Exit

Files: `src-tauri/src/lib.rs`, `src-tauri/src/core/notify/handle.rs`.

The initial close-to-exit design was implemented and verified, then superseded by the user's explicit hide-to-tray choice on 2026-10-27.

- [x] Update the portable close regression to expect repeated hiding without setting the exit flag. Observe it fail against close-to-exit behavior (`.artifacts/portable-tray-close-red.log`).
- [x] Remove close-triggered quit and its unused atomic exit reservation. Keep close prevention during an existing portable shutdown.
- [x] Preserve the existing tray Exit handler calling `feat::quit()`; shutdown still belongs to the upstream cleanup owner.
- [x] Re-run the portable app-library suite, native build and focused source review for the revised behavior.

### Task 3: Verification and report

Files: `docs/validation/portable.md`, `docs/validation/build.md`.

- [x] Run focused regression tests, then app-library suites with `--features clippy,portable` and `--features clippy`.
- [x] Check rustfmt for touched files and `git diff --check`.
- [x] Build with `npx.cmd --yes pnpm@12.10.0 exec tauri build --debug --no-bundle --features portable -- --locked`.
- [x] Request independent source review, address material findings, and record evidence and untested runtime behavior.

The updater, identity, distributable archive, elevation UX, real network recovery and game/WARP validation remain separate foundation work. This change alone is not a release.

## Execution evidence

Startup/close RED: `.artifacts/portable-lifecycle-red.log` (service selected; window hidden). Service-policy RED: `.artifacts/portable-service-red.log`. Portable GREEN: `.artifacts/portable-lifecycle-tests.log` (360 passed). Normal suite: `.artifacts/standard-lifecycle-tests.log` (357 passed). Independent read-only lifecycle review reported no material findings. Runtime testing remains outside these results.

Native portable build completed with exit 0 (1m 56s); artifact and checksum are recorded in docs/validation/build.md. Formatting and diff checks passed. No app runtime or portable release is claimed.

Hide-to-tray revision verified: 360 portable app-library tests passed, native portable build exit 0 (1m 44s), focused read-only review without material findings. Latest artifact hash and logs are in docs/validation/build.md.
