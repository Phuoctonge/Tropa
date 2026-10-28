# Portable feasibility: source findings

Date: 2026-10-27. Evidence is from v2.5.8 source inspection and a successful native baseline compilation. The application has not been run; runtime success is not claimed.

## Useful existing mechanisms

- `src-tauri/src/core/manager/state.rs:162` starts an owned sidecar process. Its Windows branch preserves TUN in the generated configuration when the application is elevated, and strips TUN when not elevated.
- `src-tauri/src/core/runstate/health.rs:67` considers elevation sufficient for TUN capability. Existing Rust tests cover absent-service/elevated states and passed in the local app-library suites.
- `src-tauri/src/core/manager/state.rs:600` creates a Windows Job Object using JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE and assigns the child. Its child-termination regression passed locally. Whole-application crash behavior and network recovery still need runtime testing.
- `src-tauri/src/feat/window.rs:133` runs controlled quit, awaits core stop and cancels exit if it fails. Reuse this path instead of killing the app directly.

## Changes necessary for Tropa

| Requirement | Current upstream behavior | Smallest relevant integration point |
|---|---|---|
| Data beside exe | `app_home_dir()` uses the Windows user data directory and upstream APP_ID | `src-tauri/src/utils/dirs.rs:28`; use the same resolver before and after Tauri initialization |
| No mandatory permanent service | Normal startup still selects service according to observed health and session state; elevated TUN capability alone does not select the startup mode | `src-tauri/src/core/service.rs:1889`, `src-tauri/src/core/manager/lifecycle.rs:820` |
| Explicit standalone mode | The `dev-sidecar` feature predetermines sidecar acceptance, but also enables development behavior; it is not a production portable flag | `src-tauri/src/core/service.rs:1889`, Cargo features |
| Closing the application stops core | The main window close handler prevents close and hides the window in the tray | `src-tauri/src/lib.rs:382`; distinguish actual quit from hiding/minimizing |
| Tropa identity and updates | Upstream identifiers, signing public key and updater endpoints remain configured | `src-tauri/tauri.conf.json`, platform configuration, `utils/dirs.rs` and updater initialization |
| Portable archive | CONTRIBUTING mentions `pnpm portable`, but package.json has no such script | Define packaging only after the actual binary/resource layout is verified |

## Startup detail that must not be missed

`prepare_startup()` derives `service_required` from the TUN preference. For an absent service it can call `require_install_for_session()`, which records an install request; service status then prioritizes that pending action. The elevated-sidecar branch in the spawn function is therefore insufficient on its own. Tropa needs an explicit production portable policy before startup selection, without reusing the development-channel identity or removing permission checks.

Reading this source does not show that launch automatically executes a service installer: recording a pending install action and performing a privileged install are different operations. Tropa must avoid both requiring that action and silently adopting another client's installed service.

## Implemented first slice: portable data root

The `portable` Cargo feature now selects `current_exe().parent()/data` on Windows in the shared `app_home_dir()` resolver. `preinit_app_data_dir()` delegates to that resolver, so early startup logs and singleton state use the same root as profiles and runtime data. Other build configurations keep the upstream data-directory behavior. Resolution errors are returned without a fallback to AppData.

The regression test was run before the behavior change and failed because it received AppData instead of the executable-adjacent directory. After the change it passed. The complete app-library suite with `--features clippy,portable` also passed: 358 tests, 0 failures. An independent read-only review found no material issues in the two-file source diff. Native build verification for the feature is recorded in build.md.

## Implemented second slice: standalone lifecycle

Windows builds with `portable` now select the owned sidecar before service waiting or install-request creation, even if an upstream service is already ready. Startup accepts sidecar explicitly and no handoff watcher runs. The existing elevated/non-elevated TUN checks and Windows Job Object remain in place.

Service install, reinstall, repair and uninstall commands reject portable mode before configuration/lifecycle changes or core stopping. The service manager and privileged dispatcher also enforce the policy. Approved-core staging rejects it before invoking the elevated installer. This prevents the portable build from changing an existing upstream service through these entry points.

The user subsequently selected hide-to-tray behavior: the main-window close path prevents destruction and hides the window without setting the exit flag or stopping the core. Tray menu Exit invokes the existing `feat::quit()`, which saves configuration, waits for core stop and cancels exit on a stop error. During an in-progress portable quit, another close request remains prevented so it cannot bypass cleanup. Normal builds retain their upstream close behavior.

In the initial close-to-exit implementation, regressions failed with service selection instead of sidecar and two window hides instead of no hides. A separate rejection check failed against unrestricted service dispatch. After implementation, 360 portable app-library tests passed; the normal configuration passed 357 tests. Portable-only tests replace two normal startup-policy assertions, so the suite totals differ. Service-command tests invoke the actual command entry points without app/config initialization and require the portable rejection diagnostic. They do not invoke a real installer.

## Remaining validation

The application has not been launched. Unit tests and compilation do not establish TUN creation/removal, DNS or route recovery, behavior with a real pre-existing service, elevation consent/refusal, or game/WARP connectivity. The requested connect-time elevation flow is not implemented yet; the current upstream sidecar path requires the app already to be elevated for TUN.

Upstream identity and updater configuration remain, and no portable archive is produced. The next foundation work must isolate updates and identity, verify the resource layout, then perform controlled runtime checks before treating this as a portable distribution. The game UI and WARP profile remain subsequent development work.

The follow-up hide-to-tray regression failed against the initial close-to-exit behavior (no window hide instead of two). The revised test checks repeated hiding without starting shutdown and prevents window destruction during an existing shutdown. The tray Exit handler remains the existing `feat::quit()` call in core/tray/mod.rs. Latest verification evidence is recorded in build.md.
