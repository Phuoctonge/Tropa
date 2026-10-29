# Tropa third-party notices — local development package

Tropa is a modified fork of [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev), distributed under GNU GPL version 3. The GPL text is included as `licenses/GPL-3.0.txt`. A public binary release also requires a corresponding source distribution for the actual modified build; an upstream link alone does not provide that source.

The package contains these separate executables:

| Component | Version | License text | Source |
| --- | --- | --- | --- |
| mihomo | 1.19.33 | `licenses/GPL-3.0.txt` | [v1.19.33](https://github.com/MetaCubeX/mihomo/tree/v1.19.33) |
| Tropa probe worker (`resources/tropa-probe.exe`) | Source-bound build; identity in `component-inventory.json` | `licenses/GPL-3.0.txt`, `licenses/WarpScout.txt`, `licenses/Go-modules.txt` | Tropa `engine/` source snapshot recorded by the worker build |

The worker incorporates adaptations from WarpScout and warpscout-android; their MIT notices are preserved in `licenses/WarpScout.txt`. WarpScout itself is not shipped or invoked. The worker imports pinned amneziawg-go, usque/api, connect-ip-go and quic-go libraries directly; their terms remain separate from the upstream scanners' MIT licenses. Exact module versions, checksums and collected license hashes are recorded in the corresponding `engine/dependencies.json` source. The QUIC Initial generator credited by WarpScout is not incorporated into this worker, and that tuning profile is explicitly unavailable pending provenance/license review. This does not disable the separate MASQUE HTTP/3 transport implemented using quic-go.

The Rust/Tauri/React application, mihomo and the Go worker have further transitive dependencies. This notice is an initial component inventory, not a completed dependency license audit or clearance for public release. See `component-inventory.json` for pinned executable hashes, worker build/source/protocol identities, source/release references and remaining release gates.

Cloudflare WARP is provided by Cloudflare and subject to [Cloudflare's application terms](https://www.cloudflare.com/application/terms/). Tropa is not an official Cloudflare application. Credits identify upstream work; they do not grant redistribution rights by themselves.

The current package requires an existing Microsoft Edge WebView2 Runtime. It does not include a WebView2 installer or Fixed Version runtime. No claim about redistribution of a bundled Microsoft runtime is made.

## Embedded emoji font

The frontend contains a modified Twemoji Mozilla font inherited from Clash Verge Rev. Its origin is [twemoji-colr v0.7.0](https://github.com/mozilla/twemoji-colr/tree/v0.7.0), copyright 2016–2018 Mozilla Foundation, with Twemoji artwork from Twitter and other contributors. The code is licensed under Apache-2.0; the visual artwork is licensed under CC BY 4.0, separately from the application's GPL license.

The original v0.7.0 font was added unchanged by Clash Verge Rev in commit `50ba2e3ad4eabdf701e2c87cf511809d78605c28`. Clash Verge Rev replaced it with the modified font used here in commit `b10c1d5006081d79c7517dce9fec83627f796d4a`; Tropa made no further font changes. That conversion/rebuild has not been reproduced. Full attribution and upstream notice are in `licenses/Twemoji-Mozilla.txt`; the full license texts are in `licenses/Apache-2.0.txt` and `licenses/CC-BY-4.0.txt`. These attributions do not imply endorsement.

## Additional unresolved provenance

The pinned `tauri-plugin-mihomo` / `tauri-plugin-mihomo-api` repository at commit `7a03d16f4d39b1bb885b7babe66b9e153e34b615` has no explicit LICENSE/COPYING/NOTICE file in the inspected complete tree. Its README and package metadata did not establish a license. Root licensing of another repository is not substituted for explicit terms here; this remains a public distribution gate.

The pinned mihomo executable is built from the clean v1.19.33 source tag at commit `3f27ef39f8c96bbfbc8b0b9bb10f5c9ef60149d8`, using Go 1.26.3, `with_gvisor`, CGO disabled and fixed build metadata. Embedded metadata reports `vcs.modified=false`; two local offline builds produced identical SHA-256 values. The exact source archive hash is recorded in `component-inventory.json`. This is local build evidence, not a completed source-license audit or cross-host reproducibility claim. The executable reports `GOAMD64=v2`; the current package therefore requires an x86-64-v2-capable CPU.

## Go modules and embedded Windows driver

`licenses/Go-modules.txt` preserves a conservative historical collection of 192 named license/notice files from 157 unique module/version pairs, originally collected for mihomo and WarpScout. Each module ZIP was checked against its embedded `h1` checksum before collecting the notices; actual replacement module versions are retained. All 21 module/version/checksum tuples in the current worker dependency inventory match entries in the collected module inventory. The collection also includes the Go runtime licenses for the reported toolchain tags and the README license/author declaration for `go-localereader`, which supplies no separate full license file in its module ZIP. Extra historical, nested and test notices do not imply those components are shipped; coverage of collected notices does not establish that every source notice or source-distribution obligation is satisfied.

The pinned mihomo executable contains the exact amd64 Wintun 0.14.1 DLL supplied in the [official prebuilt archive](https://www.wintun.net/builds/wintun-0.14.1.zip). It is embedded through sing-tun and has separate **Prebuilt Binaries License** terms, preserved in `licenses/Wintun-0.14.1.txt`. The MIT license of the Go Wintun binding does not license the prebuilt DLL. Compliance of the embedded loader/use with those terms, including the Permitted API and redistribution conditions, remains under review. No endorsement is implied.

The same mihomo executable also contains an EasyTier core WASM artifact. Its module supplies LGPL-3.0 terms, but the exact artifact's Rust dependencies, corresponding source and applicable redistribution requirements have not been fully audited. GPL/LGPL/MPL source and other obligations across Go dependencies remain open, including clarification of the GPLv2 statement in go-lzo. Copying license texts alone does not establish public release readiness.

## Frontend dependency candidate notices

`licenses/Frontend-candidates.txt` preserves 271 license/notice files from 273 installed production/optional/peer package candidates, plus the ahooks 3.10.0 and @vscode/l10n 0.0.18 MIT texts from their identified source revisions. Package/version/path headers retain attribution boundaries. Local manifest and notice hashes were checked against the collected inventory; the four investigated npm archives also match lockfile integrity and installed bytes. This is a conservative candidate collection, not an exact Vite bundle SBOM or completed source/license clearance.

The collection explicitly leaves client-only/server-only source attribution, the mihomo plugin license, the unresolved optional peer and complete transitive/source obligations open. The remaining public release gates in component-inventory.json still apply.

## Rust compilation dependency notices

`licenses/Rust-release-candidates.txt` contains 1,037 named notice files from 602 registry packages reported by the recorded release compilation, including build/host/proc-macro dependencies. Registry archives were checked against Cargo.lock checksums, and collected files matched the installed source bytes. A full terminfo 0.7.5 license comment is also preserved. This conservative collection is not an exact linked-code inventory.

Thirty-three registry packages lack separate named notice files. Git/local dependencies, further source-header notices, embedded data and corresponding-source obligations remain under review. Root repository texts discovered during research are not treated as blanket permission for every nested component. These notices do not close the public release gates above.
