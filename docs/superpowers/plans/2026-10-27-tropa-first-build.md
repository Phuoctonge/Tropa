# Tropa First Build Implementation Plan

> Execution: use superpowers:executing-plans in this chat, following the user's request to begin development. The approved specification is the scope authority for this derivative project; this work is not a contribution to upstream.

**Goal:** Import the pinned upstream history into Tropa, reproduce the available baseline checks, and establish the actual requirements for a portable Windows build.

**Architecture:** Keep the upstream application intact while measuring its build and lifecycle. Preserve the Tropa specification and upstream attribution. Any behavioral change to portable startup follows inspection of the real implementation and its own focused test cycle.

**Tech Stack:** Clash Verge Rev v2.5.8, commit 4337bd85758d54d7b9044b2da3c9ac1176eae1f3; Rust 1.99.0; pnpm 12.10.0; Node 24.x; Windows x64, React and Tauri.

**Spec:** [Tropa design](../specs/2026-10-27-game-warp-design.md), approved by the user in this chat on 2026-10-27.

## Global Constraints

- Target Windows 11 x64; support is verified rather than inferred.
- Portable distribution: no installer or permanently installed service required. Elevation on connection is permitted.
- One mihomo instance during connection; stop the instance owned by Tropa after disconnect.
- Preserve upstream history, LICENSE and attribution. Do not publish or install networking components during the source/build task.
- No WARP registrations, credentials or raw connection logs in Git.
- Source code, code comments and commit subjects are English; existing Russian product documents remain Russian.
- The approved project brief supplies the problem and authorization for this local derivative. No upstream issue or PR is created as a prerequisite for local compilation.

## Task 1: Import the pinned source without losing local documents

**Files:** import upstream tree and Git metadata; preserve root README as `docs/tropa-overview.md`; keep upstream README at root with a Tropa introduction; preserve `docs/superpowers`, `docs/roadmap.md`, and the review.

**Interfaces:** existing `.tropa-upstream` checkout at the exact commit → current root becomes branch `codex/foundation-validation`, remote `upstream` points to the original, and no `origin` is configured.

- [x] Verify `git -C .tropa-upstream rev-parse HEAD` equals the pinned SHA; read root and scoped AGENTS.md.
- [x] Complete checkout on `codex/foundation-validation`; list collisions before copying. Abort on any collision other than README, docs directories, or the Tropa plan already accounted for here.
- [x] Move the current README to `docs/tropa-overview.md`, adjusting its relative links from `docs/...` to paths relative to `docs/`.
- [x] Copy source files recursively without deleting any current document. Move the staged `.git` directory to the project root only after the file copy succeeds. Rename the original remote to `upstream`.
- [x] Add a short Tropa heading above the original README, retaining its attribution and content. Add local tooling/staging ignores to `.git/info/exclude`; no runtime dependency or application behavior changes in this task.
- [x] Verify `git status --short`, `git log -1 --oneline`, and the complete local Markdown-link graph of Tropa documents. The imported source should differ only in intentional project documents.

## Task 2: Record and reproduce the baseline build

**Files:** create `docs/validation/build.md`, `docs/validation/environment.md`, `docs/validation/components.md`.

**Interfaces:** pinned source and upstream scripts → factual commands, actual tool versions, exit statuses, artifacts and explicit blockers.

- [x] Read `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `.tool-versions`, `rust-toolchain.toml`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and the prebuild script before running any lifecycle script.
- [x] Check `node --version`, `pnpm --version`, `cargo --version`, `rustc --version`, and availability of MSVC/Windows SDK. Do not call an unavailable command successful. If Rust or MSVC is missing, continue independent frontend checks and document the native blocker.
- [x] Use the package manager pinned in package.json through `npx.cmd --yes pnpm@12.10.0 --version`, then `npx.cmd --yes pnpm@12.10.0 install --frozen-lockfile`. Do not rewrite the lockfile to match the preinstalled pnpm 11.
- [x] Run `npx.cmd --yes pnpm@12.10.0 typecheck`, `npx.cmd --yes pnpm@12.10.0 test`, and `npx.cmd --yes pnpm@12.10.0 web:build` if installation succeeds. Record output and any pre-existing failures before editing product behavior.
- [x] If the native toolchain is available, record the exact resource-download behavior before `prebuild`; it downloads service binaries but must not execute their installers. Do not use NSIS-only preparation merely to build the frontend.
- [x] Build the native binary without bundling an installer using the actual Tauri CLI options after resource preparation. If a toolchain prerequisite is absent, record that fact and the exact next dependency; do not claim that a web build is a Windows application.

## Task 3: Trace portable startup and shutdown

**Files:** create `docs/validation/portable.md`; update the shell decision only when evidence is sufficient.

**Interfaces:** source startup, directory selection, core ownership and termination code → file/line evidence, required changes, runtime checks still pending.

- [x] Trace portable detection, data-directory selection, service installation calls, elevated standalone core startup, window close versus application exit, normal shutdown and abnormal child termination.
- [x] Distinguish a source-supported branch from verified runtime behavior. Identify whether any service installation happens automatically and whether an elevated process can run TUN without that installation.
- [x] Confirm whether `pnpm portable` exists in scripts; stale CONTRIBUTING commands are not execution recipes.
- [x] Record all findings and the smallest next implementation slice. If permanent service registration is mandatory in the selected version, mark portable blocked and investigate a limited startup adaptation before considering another shell.
- [x] Run `git diff --check`, inspect the diff and document actual baseline status. Commit the approved specification and source-import changes if Git author identity is already configured; never invent identity.

## Scope boundary

This plan produces the first source/build baseline and evidence for portable development. It does not implement the v0.1 game UI, activate a tunnel, claim anti-cheat compatibility or complete the full foundation-validation matrix.

## Task 4: First portable slice — shared data root

The native baseline completed successfully before this task began. This limited portable experiment implements the storage requirement in specification section 13; it is not a distributable portable client.

**Files:** modify `src-tauri/Cargo.toml` and `src-tauri/src/utils/dirs.rs`; update the portable report.

**Interfaces:** feature `portable` selects `exe_directory/data` on Windows through the existing `app_home_dir() -> anyhow::Result<PathBuf>` and `preinit_app_data_dir() -> anyhow::Result<PathBuf>`. Default builds retain their current storage behavior.

- [x] Add `portable = []` to Cargo features and a Windows portable-only Rust regression test in dirs.rs. The test derives the expected directory from `std::env::current_exe()` and asserts that both startup and runtime resolvers use its adjacent `data` directory. It would catch a regression back to AppData or diverging startup/runtime roots.
- [x] Run `cargo test --locked -p clash-verge --lib --features clippy,portable portable_data_tests -- --nocapture`. Confirm an assertion failure showing the actual AppData path before changing `app_home_dir()`.
- [x] In the portable Windows branch of `app_home_dir()`, resolve current_exe, require its parent and append `data` using PathBuf. Return errors rather than falling back to user data. Keep the non-portable branch unchanged; preinit continues delegating to the same resolver.
- [x] Run the same targeted test again and require a pass. Use the existing clippy feature's test manifest; do not introduce test scaffolding.
- [x] Format only the touched Rust file with `rustfmt --edition 2024 src-tauri/src/utils/dirs.rs`, inspect the diff and run `git diff --check`. Build the application with `--features portable --no-bundle --debug` to verify the production configuration as well as the test configuration.
- [x] Record that standalone startup, close semantics, identity/updater isolation and ZIP packaging are still separate requirements. Do not run or distribute the storage-only binary as a completed portable client.


## Execution result

Baseline native build succeeded; the first portable storage slice passed its red/green regression, all 358 app-library tests, native feature compilation and independent read-only review. No application runtime or full portable acceptance is claimed. Git author identity is absent, so changes are intentionally left uncommitted on codex/foundation-validation; no merge, push or public fork was performed.
