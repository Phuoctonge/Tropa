# Production connection integration

## Completed ordinary H2 hour session and isolated IPv6 comparison

The user-operated ordinary package-l selected-routing session passed 122 successful paired IPv4 HTTPS samples over 3,629.90 seconds. The selected executable remained `warp=on`, the byte-identical direct control remained `warp=off`, preferences stayed unchanged and the owned core kept PID 82688. After the user disconnected, HTTP/HTTPS for selected/direct/system executables were all direct, exit-IP hashes returned to the earlier baseline, and routes/adapters/DNS/proxy exactly matched the pre-hour browser-after snapshot. Registrations were preserved, temporary secrets removed and core/worker absent; the GUI stayed open. Evidence: `.artifacts/native-connection/ordinary-package-l-hour-2026-10-28-a/{samples.jsonl,result,after,acceptance}.json`. This establishes an hour of repeated new HTTPS requests, not one long-lived socket, arbitrary UDP or IPv6.

After that cleanup, a separate diagnostic control/override pair tested the upstream IPv6 availability gate. The control had only a link-local TUN IPv6 address and selected/direct explicit-AAAA requests failed immediately. With process-local `SKIP_SYSTEM_IPV6_CHECK=true`, the isolated TUN acquired `fdfe:dcba:9876::1/126`; selected IPv6 HTTP and certificate-verified HTTPS both returned `warp=on` and IPv6 exits. The direct control timed out on this IPv4-only uplink. Both diagnostic runs restored their network baseline with no remaining core/worker, and the core was still alive after requests. Evidence: `.artifacts/native-connection/ipv6-gate-lifecycle-2026-10-28-a/`. This is a proven diagnostic workaround, not a production fix or a complete IPv6 split-routing matrix. Normal hostname IPv6 DNS, a capable direct IPv6 uplink and outer IPv6 transport remain separate checks. The user explicitly deferred IPv6 production implementation to finish UI work first.

## Ordinary package l — IPv6 blocked before TUN capture on this host

While the user's H2 session remained connected, new IPv6 HTTP/HTTPS trace requests from selected, unselected and System32 byte-identical curl executables all returned curl 6 (name resolution). Their IPv4 HTTPS controls simultaneously passed selected `warp=on`, direct/system `warp=off`. A separate IPv4 HTTPS DNS-JSON request obtained a valid public AAAA answer. Repeating each IPv6 request with explicit `--resolve` and normal TLS certificate verification returned curl 7 immediately for all executables. No IPv6 HTTP/HTTPS transmission is claimed.

The Tropa adapter's IPv6 binding was enabled, but it had no IPv6 address and no unicast IPv6 routes (only `ff00::/8`). The host had five IPv6 addresses, all loopback/link-local; there were no global or ULA IPv6 addresses. Non-secret inspection of the actual runtime confirmed top-level/DNS IPv6 enabled, a MASQUE IPv6 identity present, and the mixed TUN stack. No system or runtime settings were changed.

The pinned upstream parser explains a concrete gate: `config/config.go::parseIPV6` clears `Tun.Inet6Address` and the IPv6 fake-IP range when `verifyIP6()` is false. `config/utils.go::verifyIP6` examines interfaces before TUN creation and returns false if none has a global-unicast IPv6 address (including ULA under Go's classification). The observed host meets that condition. The upstream-supported per-process `SKIP_SYSTEM_IPV6_CHECK=true` bypasses that availability heuristic; its applicability to IPv4-outer WARP with tunneled IPv6 needs a controlled separate native test, not an assumption. The running hour session was not restarted or changed to test it.

Evidence: `.artifacts/native-connection/ordinary-package-l-ipv6-2026-10-28-a/{connected,connected-explicit-resolution,adapter-state}.json`; reproducible observers under `.artifacts/validation/observe-selected-ipv6*-2026-10-28.py`. These findings establish the failed host/TUN scenario and the parser gate, not a fix, direct IPv6 availability, working tunneled IPv6 or an IPv6 outer endpoint.

## Repeated clean-core H2 lifecycle — 21 diagnostic cycles

The existing feature-gated application hook (`portable,tropa-probe`, `--tropa-smoke --tropa-split-smoke`) completed one initial cycle and a separate batch of 20 consecutive cycles. The diagnostic app was built successfully from the current application with the same pinned clean core and owned worker as ordinary package l, then staged in its own portable data folder. It is a separate debug executable, not the ordinary release binary and not twenty manual GUI cycles. Every cycle freshly validated the exact saved H2 profile before production TUN connection.

Each cycle confirmed selected IPv4 HTTPS `off → on → off`, byte-identical unselected curl `off → off → off`, selected exit-IP hash change and return, healthy Connected then Disconnected state, successful process exit, unchanged registrations, removal of temporary secret runtime and absence of core/worker. Routes, adapters, DNS and system proxy exactly matched the batch baseline after every cycle. The hook also required HTTP 407 from the unauthenticated local verification listener while connected. The 20-cycle batch took 6.57–7.01 seconds per complete app/connect/check/disconnect/exit cycle; the initial run took 8.20 seconds. These are complete lifecycle durations, not connection latency.

Evidence: `.artifacts/native-connection/clean-core-lifecycle-2026-10-28-b/{preparation,acceptance}.json` and `first/`, `repeated/` per-cycle smoke/network reports. Reproducers: `.artifacts/validation/{prepare,run,accept}-lifecycle-2026-10-28.{py,ps1}`. The independent Python acceptance rechecked all 21 raw reports, network comparisons, exit hashes and unchanged binary hashes. Final process inspection found only the two pre-existing user apps, with no core or worker. The abandoned `-a` staging directory never launched an app.

This closes repeated production-path H2 selected-IPv4-HTTPS cycles on this host. A 60-minute session, other transports, arbitrary UDP/IPv6, sleep/resume, network changes, faults and ordinary GUI repetition remain separate open scenarios. The fixture contains private local identities and must not be redistributed.

## Ordinary package l — Firefox selected / Chrome direct

The user added Firefox through the normal routing picker in the existing separate package-l fixture while disconnected. Persisted preferences contain the actual Firefox Portable executable `App/Firefox64/firefox.exe`; Chrome is absent. This exercises the GUI picker/persistence path, although whether the user chose the launcher or the internal EXE was not independently observed. Firefox 159.0 and Chrome 156.0.8078.48 binary hashes and running process IDs were recorded without reading browser history.

The user opened separate nonce-bearing HTTPS trace URLs before, during and after connection:

| Browser | Before | Connected | After |
| --- | --- | --- | --- |
| Selected Firefox | `warp=off`, HTTP/2 | `warp=on`, HTTP/3 | `warp=off`, HTTP/2 |
| Unselected Chrome | `warp=off`, HTTP/2 | `warp=off`, HTTP/2 | `warp=off`, HTTP/2 |

Browser trace values come from the user's supplied page text, not automated DOM capture. The reported exit changed only for Firefox and returned afterward; raw browser exit IPs were not copied to the evidence files. An independent Windows curl HTTPS control remained direct at all three phases. Network snapshots confirmed active core/TUN only during connection and exact restoration of routes/adapters/DNS/proxy afterward; registrations and temporary secret runtime cleanup also passed.

Evidence: `.artifacts/native-connection/ordinary-package-l-browser-2026-10-28-a/{baseline,connected,after,user-browser-reports,acceptance}.json`, network snapshots; observer/acceptance scripts under `.artifacts/validation/`. The HTTP/3 observation establishes one selected-browser QUIC page request through the existing H2 WARP transport. It does not establish MASQUE H3, arbitrary UDP, a long-lived socket matrix, IPv6 or known/enabled DoH. Browser privacy/DNS settings were not changed.

## Ordinary package l — exact-path split routing passed

The same ordinary release bytes were extracted to a separate own fixture, with copies of its own registrations and the verified H2 profile pinned manually. Offline fixture preferences selected one copy of Windows `curl.exe`; the other directory and System32 copies had the same basename and identical SHA-256. The GUI EXE picker was not exercised by this fixture setup. The user launched this app and used its normal Connect/Disconnect controls.

For new IPv4 TCP connections, HTTP and HTTPS both reported `off → on → off` for the selected executable, and `off → off → off` for the unselected and System32 executables. Exit-IP hash comparison confirmed that only the selected executable changed its exit during connection and returned afterward. The TUN adapter was observed Up while connected. Following explicit Disconnect, all network fields (routes, adapters, DNS and system proxy) exactly matched baseline, registrations and the manual profile were unchanged, core/worker were absent, temporary secret runtime files were removed and the app remained open.

Evidence: `.artifacts/native-connection/ordinary-package-l-split-2026-10-28-a/{preparation,baseline,connected,after,acceptance}.json`, network snapshots and stderr files. Reproducers: `.artifacts/validation/{prepare,observe,accept}-ordinary-package-l-split-2026-10-28.*` (observer filename is `observe-package-l-split-2026-10-28.ps1`). The original portable data was not modified by staging this separate fixture. Private registration files remain ignored local data and are not part of the ZIP.

This fixture proves exact executable-path separation for these new IPv4 HTTP/HTTPS TCP connections in H2. The separate browser acceptance above covers its own Firefox/Chrome scenario. DoH, arbitrary UDP, IPv6, long-lived/pre-existing sockets, folder/child/service routing, other transports and game/anti-cheat compatibility remain outside these results.

## Ordinary package l — successful user-operated system TUN cycle

The user launched the exact ordinary package l EXE (SHA-256 `65e56d678fd29b72c208c1668531c584b13162418e6c788af14d777fb4ea39c6`) and confirmed UAC manually. The fresh portable data root had no registration/catalog before the first connection. With full-computer routing and automatic selection, the app registered its identities, saved 15 scanned candidates and one runtime-verified H2 profile (`162.159.198.2:443`, default consumer SNI). This is distinct from the earlier cancelled diagnostic launch.

Actual Windows HTTP/HTTPS requests with explicit HTTP proxy bypass reported `warp=off` before, `warp=on` while connected, `warp=on` after the user closed the window to tray, and `warp=off` after the user selected tray Exit. A live `Tropa` / `Meta Tunnel` adapter was observed. Core PID 73452 belonged to the new app PID 75468. After Exit both were gone, the earlier user app PID 67668 remained, routes/adapters/DNS/system-proxy settings exactly matched the baseline, the two new registrations were unchanged since connection, and temporary secret runtime files were removed.

Evidence: `.artifacts/native-connection/ordinary-package-l-2026-10-28-a/{baseline,connected,tray,after,acceptance}.json` plus each network snapshot. Reproducers: `.artifacts/validation/{observe-package-l-network-2026-10-28.ps1,accept-ordinary-package-l-2026-10-28.py}`. UI Connected/tray/Exit observations came from the user; the UI helper did not operate the window. No raw exit IPs or registration keys were printed; local comparison uses IP hashes.

This closes one current ordinary full-computer H2 connect/tray/exit cycle. Selected-app routing was subsequently checked by the separate fixtures above. Arbitrary UDP, IPv6, other transports and clean Windows require their own evidence; lifecycle diagnostics also have a separate scope.

Ordinary GUI observation remains unavailable in the latest continuation: importing the documented `@oai/sky` helper failed with `trusted Node process exited unexpectedly; kernel reset, rerun your request`, then failed identically after an explicit kernel reset/reinitialization. No app input occurred. Existing user Tropa PID 67668 was left untouched. This helper failure supplies no evidence about Tropa GUI correctness and does not close native TUN/tray acceptance.

## Current clean-core scope — 28 October 2026

Ordinary package l now includes clean source-built mihomo SHA-256 `8ccf696897f4d56aa8f9b13893b6b3c6d24a8a505eb257d46eeac53e6701ed2e`. Its app/source/worker package verification and worker path/move check passed; these are not network acceptance. Earlier coordinator runs below used the former pinned core and keep their original scope.

The post-package Rust `core::warp_engine` suite ran with the exact source-bound worker pin and runtime fixture, `--include-ignored --test-threads=1`: **27 passed, 0 failed, 0 ignored**, including the native Go cancellation/OS-exit and fault-injection cases. The production persistence-error path retained its IO error and released ownership. Log: `.artifacts/validation/clean-core-pinned-native-engine-2026-10-28.txt`. These synthetic/loopback worker tests do not launch mihomo, system TUN or ordinary GUI.

The current standalone-core comparison reports are `.artifacts/native-connection/clean-core-{h2,awg,wg,h3}-2026-10-28-a/result.json`. Both baseline and clean core passed H2 (`162.159.198.2:443`) and AWG (`188.114.98.58:2408`, exact fixture parameters): HTTP/HTTPS reported `warp=on`, UDP DNS returned 5/5. WG did not confirm data; H3 returned HTTP 502 and timed out on HTTPS; both had UDP DNS 0/5. These are observations on this network, not universal transport support/failure conclusions. All comparisons confirmed process exit, runtime cleanup and unchanged registrations. Neither system TUN nor the Rust coordinator was tested in these comparisons.

The freshly rebuilt coordinator diagnostic in `.artifacts/native-connection/clean-core-owned-chain-2026-10-28-a/` did not run: Windows refused the unelevated launch and the subsequent UAC launch was cancelled. Its `elevation-cancelled.json` records that no elevated connection test began. Complete current coordinator, exact-manual/failure and ordinary app TUN/tray checks before carrying earlier successful results forward to package l. External arbitrary UDP, IPv6, browser isolation and full lifecycle acceptance remain open.

## Owned-search native acceptance — 2026-10-28

The current `tropa-warp-probe.exe` was rebuilt with `portable,tropa-probe` and
the pinned worker `source:54d61ff7ffb7c7aaeb2794448386c7c26385f8ca83c7e48007242201ad946c73`.
Its SHA-256 is `fd80589e0d6a85ed5196879fbf38fcdcfb9cf88598337e324fec5e28bb1fd572`.
An unelevated launch failed with elevation required and started no diagnostic;
the successful runs used an explicitly elevated, hidden owned helper.

Official WARP reported Disconnected before the tests, and a direct HTTPS trace
reported `warp=off`. Router settings were left unchanged. Each run used an
isolated data root with copies of existing registrations; no keys were printed
or supplied in process arguments. Ordinary application preferences were not
changed. There was no pre-existing catalog in either first-search root.

| Run | Elapsed, including observed cleanup | Result |
| --- | --- | --- |
| H2 first search | 5.603 s | `162.159.198.2:443`, HTTPS WARP true, UDP DNS 5/5 |
| H2 cached revalidation | 2.661 s | Same exact endpoint, fresh HTTPS WARP and UDP DNS true |
| Mixed WG/AWG/H3/H2 first search | 34.713 s | AWG `188.114.98.58:2408`, HTTPS WARP and UDP DNS true |
| WG-only first search | 179.087 s | Validation timeout; no fresh mihomo success |
| H3-only first search | 179.568 s | Validation timeout; no fresh mihomo success |

The separate WG/H3 matrix is saved under
`.artifacts/native-connection/protocol-matrix-2026-10-28-e/{wireGuard,masqueH3}`.
The WG catalog retained 60 measured profiles: 36 had handshake=true and
HTTP=true, none had HTTPS=true. The H3 catalog retained 14 measured profiles,
with no handshake/HTTP/HTTPS=true. These are counts of retained observations,
not a complete count of every attempted address. Unknown stages are not
converted into failures. In both matrix runs registration bytes were unchanged,
the temporary runtime was deleted and no worker/mihomo process remained.
The failures describe this network and bounded run, not universal transport
unavailability. WG's HTTP success must not be described as no data transfer.

The H2 first-search catalog was observed after 594 ms and the mixed-search
catalog after 548 ms, both before diagnostic exit. Process samples recorded
the owned worker, its disappearance, then mihomo; no sampled overlap occurred.
Sampling at approximately 400 ms is observational evidence, not a proof that
every short-lived process transition was captured. OS-confirmed shutdown is
also covered at the worker ownership boundary by the native engine tests.

The three successful runs exited with code 0, preserved registration bytes, deleted the
secret probe-runtime configuration and left no worker/mihomo process. The
mixed catalog contains 15 profiles across all four kinds, including baseline
profiles; this count does not establish successful data transfer for every
transport. Only the final AWG profile was freshly validated by mihomo in that
mixed run. The H2 result separately identified node HEL/FI and exit country RU.

Public evidence is local under
`.artifacts/native-connection/owned-chain-2026-10-28-e/first-search/` (first H2),
the parent directory (cached H2), and
`.artifacts/native-connection/owned-chain-2026-10-28-f/` (mixed). Each has
`acceptance.json`, `report.json` and `process-transitions.json`. Adjacent `data`
directories contain private registrations and must never be distributed.

These runs establish the current Rust coordinator → owned worker → streamed
catalog → mihomo proxy-validation path. They did not enable system TUN, test
arbitrary application UDP, prove IPv6, exercise browser split routing or test
games/anti-cheats. Cancellation/fault scenarios, full TUN acceptance of the
current artifact and a current ordinary portable ZIP remain open. Current
build/package commands are in [portable release](portable-release.md).

## Historical integration results — 2026-10-27

The following records describe earlier implementations and packages. Legacy
WarpScout build instructions and feature availability below are historical,
not the current build procedure.

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
