# Tropa Minimal Interface Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement task-by-task in this session.

**Goal:** Deliver the approved preliminary main, game, server and settings screens in the existing application.

**Architecture:** A pure React view owns screen navigation. A page adapter uses backend services for the selected game and existing configuration for theme; mutations use the shared intent funnel. Backend config owns atomic persistence. Keep technical upstream routes available separately.

**Tech Stack:** React 19, TypeScript, existing Tauri services, Rust, existing YAML atomic writer, CSS.

**Spec:** ../specs/2026-10-27-tropa-interface-design.md

## Global constraints

- One selected `.exe`, no execution or network changes from file selection.
- No simulated successful connection, fabricated servers or pings.
- System/light/dark themes; settings and selections replace the current screen.
- No dependencies or lockfile changes. Existing approved derivative scope applies.
- Preserve existing local source changes; no commits without author identity.

### Task 1: Selected-game persistence

Files: create `src-tauri/src/config/tropa.rs`, `src-tauri/src/cmd/tropa.rs`; register in existing config/cmd modules and handler list.

- [x] Add a real temporary-directory regression: save a valid local game executable path, load it, attempt an invalid `.txt` path, and verify the previous selection survives. An absent preferences file returns no selection; malformed data returns an error.
- [x] Run `cargo test --locked -p clash-verge --lib --features clippy,portable config::tropa`; observe a missing behavior failure, then implement validation and atomic persistence using `help::save_yaml`.
- [x] Commands `get_tropa_preferences` and `set_tropa_game(game: Option<PathBuf>)` serialize through `Config::lock_config_write`. Save to `app_home_dir()/tropa.yaml`. Never execute the path.

### Task 2: Minimal screens and application entry

Files: create `src/components/tropa/tropa-view.tsx`, its CSS, `src/pages/tropa.tsx`, `src/services/tropa.ts`, `src/types/tropa.ts`; update `src/pages/_routers.tsx`.

- [x] Render the main screen with one unavailable connection switch and visible setup explanation, game/server rows and settings access.
- [x] Implement Back navigation, native `.exe` picker via services, removal of selection, Automatic server with empty candidate state, and persisted theme through existing Verge config.
- [x] Use Tropa at `/`; keep upstream Home at `/advanced` and the other technical routes. Keep native window controls and initial-overlay dismissal. Show errors inline and disable duplicate mutations.
- [x] Add a separate development-only preview entry using the same view for browser verification without launching the networking application.

### Task 3: Verification

- [x] Run frontend typecheck, lint, tests and web build. Run the backend app-library suite and native portable build.
- [x] Inspect main/game/server/settings screens and light/dark compact layouts in a browser, including navigation and keyboard behavior. Fix material visual or interaction defects.
- [x] Request read-only review of persistence, routing and UI truthfulness. Record limitations and exact build evidence in `docs/validation/interface.md`.

Execution completed: 45 frontend and 362 portable backend tests passed; browser navigation/theme/compact checks passed. Independent review findings were resolved. Final native build passed; exact artifact hash and limits are recorded in docs/validation/interface.md. No native application launch or network validation was performed.
