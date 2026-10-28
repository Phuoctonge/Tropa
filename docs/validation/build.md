# Tropa first build baseline

Date: 2026-10-27. Source: upstream v2.5.8 at 4337bd85758d54d7b9044b2da3c9ac1176eae1f3. The original baseline was compiled before changing product behavior; the subsequent portable storage slice is reported separately below.

## Completed checks

| Command | Exit | Result |
|---|---|---|
| `npx.cmd --yes pnpm@12.10.0 install --frozen-lockfile` | 0 | 912 packages installed; lockfile not rewritten |
| `npx.cmd --yes pnpm@12.10.0 typecheck` | 0 | TypeScript passed |
| `npx.cmd --yes pnpm@12.10.0 test` | 0 | 11 test files, 45 tests passed |
| `npx.cmd --yes pnpm@12.10.0 web:build` | 0 | Vite production output created in dist |
| `npx.cmd --yes pnpm@12.10.0 lint` | 0 | ESLint passed with zero allowed warnings |
| `npx.cmd --yes pnpm@12.10.0 prebuild x86_64-pc-windows-msvc` | 0 | Downloaded both upstream cores and resources; did not execute service installers |
| `npx.cmd --yes pnpm@12.10.0 exec tauri build --debug --no-bundle -- --locked` | 0 | Native upstream baseline compiled in 6m 11s; executable created |

These checks establish a frontend/source baseline, not a working Windows executable or successful WARP tunnel.

## Native prerequisites

Initial `tauri info` reported missing Rust, Cargo and MSVC/SDK build components. Rust 1.99.0 was installed locally from the official rustup bootstrapper. Upstream-requested rustfmt and clippy were also obtained. The signed Microsoft Build Tools bootstrapper was verified as signed by Microsoft Corporation; installation of the VCTools workload returned exit code 0. A fresh `tauri info` detects Visual Studio Build Tools 2022, Rust, Cargo and WebView2 successfully. The C++ installation is system development tooling; Rust remains project-local.

`cargo fetch --locked --target x86_64-pc-windows-msvc` completed with exit code 0. Cargo reports an existing unused tray-icon patch for a macOS-specific branch; no dependency was updated to silence it.

Project-local Rust commands in PowerShell:

```powershell
$env:CARGO_HOME = Join-Path (Get-Location) '.tools/cargo'
$env:RUSTUP_HOME = Join-Path (Get-Location) '.tools/rustup'
$env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
$env:CARGO_NET_GIT_FETCH_WITH_CLI = 'true'
$env:GIT_CONFIG_COUNT = '1'
$env:GIT_CONFIG_KEY_0 = 'http.sslBackend'
$env:GIT_CONFIG_VALUE_0 = 'openssl'
cargo fetch --locked --target x86_64-pc-windows-msvc
```

The Git environment override is process-local; it enables verified OpenSSL HTTPS for Cargo's Git dependency fetches on this machine.

## Resource preparation

The upstream prebuild script downloads stable mihomo, alpha mihomo, the service archive, geo data, enableLoopback.exe and an NSIS plugin. The script itself was read before execution. No downloaded networking executable was launched.

For this run only, `APPDATA` was set to `.tools/build-appdata` in the prebuild process so the NSIS plugin stayed inside the project rather than modifying the user's AppData. The original process environment was not changed globally. The plugin is an upstream build dependency, not a promise that Tropa will ship an installer.

The prebuild script resolves several `latest` URLs. Actual core versions and binary hashes are recorded in [components](components.md). This one recorded download does not yet provide fully reproducible resource acquisition; release pinning remains necessary before a Tropa distribution.

## Native build and runtime status

Native baseline compilation passed. The executable was created at `target/debug/clash-verge.exe`, 87,147,520 bytes, baseline SHA256 `72cb657bae5060de4a7e8107c843b6166456f729d6792d45ae342fcbdbcbec5b`. This is a historical checksum of the unchanged baseline; subsequent feature builds can overwrite the file. The application was not launched. No service was installed by Tropa, no TUN was enabled and no network settings were changed by a Tropa run. Build Tools are development tooling and are separate from the networking service that portable Tropa must avoid requiring.

The native baseline command uses a debug profile for the first compilation, no installer, and locked dependencies. CARGO_BUILD_JOBS is 4 to limit compiler memory pressure:

```powershell
npx.cmd --yes pnpm@12.10.0 exec tauri build --debug --no-bundle -- --locked
```

The upstream updater, application identity, startup policy and storage paths are still upstream defaults. Do not distribute this baseline as a finished Tropa build.

## First portable code change

Added feature `portable` in src-tauri/Cargo.toml and a Windows-only branch in the shared data-root resolver. With the feature enabled, startup and runtime data resolve beside the executable in `data`. Default builds retain their original paths.

The test command was first run against the unchanged resolver and exited 101 with an assertion showing AppData instead of the executable-adjacent directory. After implementation, the same command exited 0:

```powershell
cargo test --locked -p clash-verge --lib --features clippy,portable portable_data_tests -- --nocapture
```

The complete app-library suite was then run with `cargo test --locked -p clash-verge --lib --features clippy,portable`: 358 passed, 0 failed, exit 0. Existing upstream warnings about an unused macOS tray-icon patch and the clippy test build mode remain; they are not portable regressions.

Production-mode verification uses `npx.cmd --yes pnpm@12.10.0 exec tauri build --debug --no-bundle --features portable -- --locked`. Application runtime is still untested; storage portability alone does not satisfy the service, shutdown, updater and packaging requirements.


The storage-only portable feature build completed with exit code 0 in 2m 23s. Artifact: target/debug/clash-verge.exe; size 87147520 bytes; SHA256 0ceaec714be6812104a583737d6302fd77820e67e31d95a4c6b53c094eedb300. This debug binary is not a distributable portable product. Rust formatting and git diff checks passed; lockfiles and frontend source are unchanged.

## Portable lifecycle build

The Windows portable lifecycle change selects the owned child core, rejects service management, and routes main-window close through controlled quit. The startup/close regressions failed against upstream behavior before the implementation. Service rejection was also observed failing before its policy was added.

Final portable app-library verification: `cargo test --locked -p clash-verge --lib --features clippy,portable`, 360 passed, 0 failed, exit 0. Normal configuration verification: `cargo test --locked -p clash-verge --lib --features clippy`, 357 passed, 0 failed, exit 0. Existing elevated TUN capability, controlled-stop failure and Windows Job Object tests passed within these suites. An independent read-only review found no material lifecycle issues.

Native feature build: `npx.cmd --yes pnpm@12.10.0 exec tauri build --debug --no-bundle --features portable -- --locked`, exit 0, native compilation 1m 56s. The command also ran TypeScript checking and the Vite production frontend build. Logs: `.artifacts/portable-lifecycle-tests.log`, `.artifacts/standard-lifecycle-tests.log`, `.artifacts/portable-lifecycle-build.log`.

Artifact: `target/debug/clash-verge.exe`, 87183360 bytes, SHA256 `2db1bd0282f9b8f19bf3ee58af78f010933404249bd3d152c66617683de017c4`. This overwrites the earlier storage-only binary. Rust formatting and `git diff --check` passed. Dependency lockfiles and frontend source remain unchanged. The application was not launched; no service or network changes were made by running Tropa. Upstream identity/updater isolation, connect-time elevation, archive packaging and real network/game validation remain unfinished.

## Revised close behavior: hide to tray

The user approved X hiding the window while retaining the connection, with tray Exit performing shutdown. The close dispatcher now hides without setting the exit flag; the unused close-triggered quit callback and atomic reservation were removed. During an existing portable shutdown, window destruction stays prevented. The upstream tray Exit handler still calls `feat::quit()`.

The revised close test failed against the previous close-to-exit behavior, then passed after the change. Final app-library verification with `--features clippy,portable`: 360 passed, 0 failed, exit 0. Native portable build completed with exit 0, compilation 1m 44s. The build also completed TypeScript checking and the Vite production build. A focused independent source review found no material issues; formatting and documentation link checks passed.

Latest artifact: `target/debug/clash-verge.exe`, 87148544 bytes, SHA256 `512f6818600d0c63556e2421f9b5d06eab41586964f6392d698074b2db1c2f95`. Logs: `.artifacts/portable-tray-close-red.log`, `.artifacts/portable-tray-close-tests.log`, `.artifacts/portable-tray-close-build.log`. Earlier build hashes above are historical. No app runtime or network recovery validation was performed.

## Preliminary minimal interface

The Tropa interface and backend-owned selected-game preference are implemented. Frontend typecheck/lint and 45 tests passed; the portable backend suite passed 362 tests. See [interface validation](interface.md) for browser checks, regression evidence and product limits.

Final native portable interface build passed with exit 0; compilation 1m 20s. Artifact: `target/debug/clash-verge.exe`, 87222784 bytes, SHA256 `99283adaea7325e83497636d52f5184616fa8f9c9f43e81debde382e34218dca`. Log: `.artifacts/tropa-interface-build.log`. The build includes the final screen-history and technical-navigation fixes. It remains a debug build without an installer or portable ZIP.
