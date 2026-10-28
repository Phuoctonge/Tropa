# Tropa validation environment

Date: 2026-10-27, Europe/Moscow. No networking or game validation has been run.

| Item | Observed value |
|---|---|
| OS | Windows 11 IoT Enterprise LTSC, build 26100, x64 |
| CPU | Intel Core i5-11400F, 12 logical processors |
| RAM | 15.9 GiB reported by Windows |
| Node | 24.19.0 installed; upstream CI uses 24.21.0 |
| pnpm | 12.10.0, invoked explicitly through npx; preinstalled fallback is 11.25.0 |
| Rust/Cargo | 1.99.0, installed under project `.tools`, without modifying system PATH |
| MSVC/SDK | Visual Studio Build Tools 2022 detected by Tauri after VCTools installation |
| WebView2 | 141.0.3537.71 detected by Tauri |
| Upstream | Clash Verge Rev v2.5.8, commit 4337bd85758d54d7b9044b2da3c9ac1176eae1f3 |
| Branch | codex/foundation-validation |
| Network provider | Not collected; no transport comparison performed |
| Games | No game sessions performed; PUBG/Valorant remain intended validation targets |

The Node patch version differs from CI; passing frontend checks apply to the observed version only. Machine resource costs, FPS, game RTT, DNS recovery and portable runtime compatibility remain unmeasured.

The default Git Windows TLS backend failed with SEC_E_NO_CREDENTIALS. Repository-local OpenSSL backend succeeded with certificate verification enabled. After importing Git metadata, sandbox command startup returned `setup refresh had errors`; commands continued through the approved escalation path. Neither error required disabling TLS verification.

No Git author name/email is configured. No identity was invented, no commit or public remote was created. The upstream commit history is present locally.
