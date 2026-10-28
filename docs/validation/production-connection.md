# Production connection integration — 2026-10-27

For the subsequent same-window server table, expanded search and latest portable test ZIP, see [expanded server panel validation](expanded-server-panel.md).

The ordinary connection is independent of game choice and routes computer traffic through WARP TUN. Server choices apply on the next connection. Refresh is available while disconnected; a healthy connection is not replaced automatically.

## Implemented

- A single operation owns discovery, cancellation, runtime startup and final system HTTPS WARP verification. Only successful verification publishes Connected.
- CoreManager owns the stable mihomo process, its Job Object and a private temporary runtime. TUN starts disabled, is enabled after IPC readiness, and is disabled before process termination. Runtime deletion follows confirmed exit. This path does not clear or set Windows system proxy.
- Discovery subprocess monitors keep execution reservations until Windows confirms exit, including failure and cancellation. Pending cleanup blocks exit and a new connection.
- The Windows portable executable embeds `requireAdministrator`, so Windows requests elevation at startup. The earlier connection-time relaunch remains a development fallback. Startup elevation does not start a connection or install a service.
- Portable startup skips upstream core/service detection, startup scripts, background updates, backups, URL registration and autostart registration. Tray menu offers Open and Exit.
- `scripts/prepare-tropa.mjs` verifies pinned scanner/core SHA256 values before copying resources; the separate Tropa Tauri config does not create installers or updater artifacts.

## Evidence

Backend: 385 tests passed, including cancellation before token publication, operation ownership, TUN-disable-before-stop, stop-failure preservation, and OS-confirmed child ownership. Frontend: 45 tests passed; TypeScript, ESLint and web build passed.

Native debug application built with `tauri.tropa.conf.json` and portable enabled. The process exposed a responsive window titled Tropa. Saved network comparison reported unchanged active routes, adapters and system proxy, with zero mihomo/scanner children. Evidence is local in `.artifacts/native-connection/{before,idle}.json`.

Subsequent elevated native smoke runs connected through the real TUN and disconnected successfully. The first run registered a device; the cached reconnect reused the same registration and profile and completed in 9.61 seconds including startup and disconnect. Saved comparisons confirmed unchanged routes, adapters, DNS and system proxy after cleanup, with no remaining owned processes or temporary runtime configurations. Evidence: `.artifacts/native-connection/tun-2026-10-27/`.

A seven-file portable folder also started successfully from a path containing spaces without an installer. A later manual validation of the original profile failed even when elevated; a separate probe reproduced that failure. No TUN was enabled and no alternate server was substituted. Automatic discovery from the same minimal resource set then validated another AWG profile with HTTPS WARP and 5/5 UDP DNS replies. This establishes that the old profile failure was not explained solely by missing administrator rights; its precise network cause remains unresolved.

After adding the startup administrator manifest, the executable was rebuilt and its embedded PE manifest independently extracted and verified as `requireAdministrator`. Launching this packaged executable without an explicit RunAs verb returned Windows error “operation canceled by the user”; that launch did not start the application. Successful accepted-UAC connection of this exact rebuilt package remains unverified. All 385 backend tests passed after this change.

The local Computer Use helper failed to initialize because its Windows sandbox refresh failed. No automated native click/tray/UAC test is claimed. The idle process was stopped by its recorded PID before rebuilding, after confirming there were no network children.

Portable WebView2 browser data is explicitly located at `data/webview`; the upstream window-state plugin is skipped because it otherwise saves into AppData. Core and scanner are copied into `resources` by the pinned preparation script; no startup copying, service installer or externalBin duplicate is required.

The user's extracted-package run exposed two development-build issues: a visible console and repeated `kode_bridge::pool` connection failures. The console came from the debug-only console subsystem. The WARP scanner/probe and production core called the upstream `reserve_sidecar`, which first probes service IPC even when no service is registered. Windows reservation now checks SCM registration first: absence goes directly to the same shared OS execution guard (including its process inspection); registered services retain the upstream handoff checks, and inspection errors remain failures. Portable debug builds now use the Windows GUI subsystem too. File logging and the administrator manifest remain enabled. All 385 backend tests and focused Rust formatting checks passed after this fix; successful GUI interaction of the rebuilt archive remains unverified.

## Remaining verification and scope

Real TUN connection and route/DNS cleanup have passed native smoke checks. Native tray Exit, click-driven manual selection and an accepted startup-UAC launch of the final package still need verification. A feature-gated native smoke runner (`portable,tropa-probe`, `--tropa-smoke`) connects, records safe status, disconnects and exits. The current test package includes that feature; ordinary launch does not invoke the runner.

Production search currently uses the verified WG/AWG adapters. MASQUE HTTP/3 and HTTP/2 need separate EC registration before exposure. The local test ZIP is `.artifacts/portable/Tropa-portable-admin-test.zip`; it contains only the executable, two resources, licenses, README and build hashes, with no registration or user data. It is a development test build, not a production release. A clean-Windows WebView2/driver check remains outstanding.

## Local build

The existing local Rust toolchain and Node dependencies are required for development. Obtain the original Windows amd64 WarpScout 0.16.1 from [its releases](https://github.com/vernette/warpscout/releases), place `warpscout.exe` in `.artifacts/warpscout-0.16.1/`, and put its MIT license in `.artifacts/warpscout-source/LICENSE`. The preparation script rejects binary hashes differing from the validated version. The pinned mihomo sidecar must already be present under `src-tauri/sidecar`.

Build through `pnpm exec tauri build --debug --no-bundle --features portable --config src-tauri/tauri.tropa.conf.json`. The Tropa overlay prepares resources and builds the frontend. The output is a development executable with its resource directory, not an installer. Keep `resources/verge-mihomo.exe`, `resources/warpscout.exe` and the license beside the app's resource directory when preparing a test folder. Credentials and WebView2 data are created inside `data/`.
