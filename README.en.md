<p align="center">
  <img src="src-tauri/icons/tropa.png" alt="Tropa" width="112" />
</p>

<h1 align="center">Tropa</h1>

<p align="center"><a href="README.md">Русский</a> · English</p>

<p align="center">A portable Cloudflare WARP client for Windows.</p>

Tropa connects through WARP, finds usable endpoints and lets you choose between
automatic connection selection and a specific server. Selecting a game is not
required to connect. The project is based on Clash Verge Rev and uses mihomo
with an owned Go discovery worker.

**Development preview.** Packages are unsigned test builds. Public release
readiness, clean-machine compatibility and the remaining license reviews are
still in progress.

## Features

- Automatic connection selection and manual server/profile selection.
- Connection search for WireGuard, AmneziaWG and MASQUE HTTP/3 and HTTP/2.
- An expandable server table with protocol, measurements and available location details.
- Split tunneling for selected executable paths or folders, or routing the whole computer through WARP.
- A portable folder containing the application, its resources and license notices.
- Close-to-tray behavior and explicit disconnect/exit controls.

Protocol availability depends on the network and endpoint. Search results are
candidates until the application confirms that the connection passes traffic.
Automatic grouping of game launchers, related processes and shared anti-cheat
services remains research; it is not part of the current routing behavior.
Game-specific server measurements are a separate planned feature.

## Download a preview

[**Download Tropa for Windows x64**](https://github.com/Phuoctonge/Tropa/releases/download/v0.1.0-preview.1/Tropa-windows-x64.zip)

1. Open [Releases](https://github.com/Phuoctonge/Tropa/releases) and select an application release marked **Pre-release**.
2. Download `Tropa-windows-x64.zip` and extract it into a writable folder.
3. Run `Tropa.exe` and approve the Windows administrator prompt. Keep the accompanying resources and licenses.

Only `Tropa-windows-x64.zip` is needed to run the app. `Tropa-sources.zip`
contains component sources and build records; `SHA256SUMS.txt` contains the checksums.
The `tropa-build-inputs-v1` prerelease contains CI dependencies, not a Tropa
application download.

Intermediate development builds remain available as
[Actions artifacts](https://github.com/Phuoctonge/Tropa/actions/workflows/tropa-portable.yml).

## Requirements

- Windows x64 and a processor supporting x86-64-v2, required by the pinned mihomo build.
- Microsoft Edge WebView2 Runtime already installed.
- Administrator rights for the portable application and its TUN connection.

Portable means running from an extracted folder without a Tropa installer.
It does not mean a single executable or that Windows runtime dependencies are bundled.

## Development

The interface uses React and TypeScript with a Rust/Tauri application. The
owned Go worker handles discovery and protocol checks; mihomo provides the
connection runtime. WarpScout CLI is not required by the current package.

[GitHub build instructions](docs/validation/github-actions.md) explain the
pinned tools, dependency assets and portable artifact verification. CI builds
the app and worker without launching a tunnel.

For current evidence and remaining work, see:

- [Status and roadmap](docs/tropa-status-and-roadmap-2026-10-28.md)
- [Portable package validation](docs/validation/portable-release.md)
- [Connection validation](docs/validation/production-connection.md)
- [Third-party component review](docs/third-party-components.md)

## Credits and license

Tropa is derived from [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev),
which continues [Clash Verge](https://github.com/zzzgydi/clash-verge). The upstream
Git history, author attribution and applicable license notices are retained.

Thanks to [Cloudflare WARP](https://one.one.one.one/),
[mihomo](https://github.com/MetaCubeX/mihomo),
[WarpScout](https://github.com/vernette/warpscout),
[Tauri](https://github.com/tauri-apps/tauri),
[React](https://github.com/facebook/react) and their contributors.
WarpScout informed the discovery implementation; the worker's component
origins and notices are documented in [engine notices](engine/THIRD_PARTY_NOTICES.md).

Tropa is licensed under [GPL-3.0](LICENSE). Third-party components retain their
own licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
