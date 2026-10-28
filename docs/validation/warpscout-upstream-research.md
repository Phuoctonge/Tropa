# WarpScout upstream research — 2026-10-27

Scope: read the linked projects' available README files and selected implementation/license files, and compare them with the locally pinned WarpScout 0.16.1 source. This is a source assessment, not a test of newly integrated transports. No downloaded code was executed, no external relay was contacted with credentials, and no application behavior was changed.

## Findings and relevance

| Source | Verified role | Implication for Tropa |
| --- | --- | --- |
| [Cloudflare WARP](https://one.one.one.one/) | Service/network being accessed; not a dependency that supplies a ready Tropa client. | Keep device registration and server selection separate. A reachable endpoint is not proof of working application traffic. |
| [puzige/CloudflareWarpSpeedTest](https://github.com/puzige/CloudflareWarpSpeedTest) | Port enumeration, randomized address sampling and RTT/loss-related methods credited by WarpScout. Current `task/warping.go` explicitly measures repeated WireGuard handshake responses; its netstack creation also builds initiation packets. | Do not infer a usable tunnel from handshake RTT or blindly equate the current project's metrics with WarpScout's `-P`. Our table takes endpoint ICMP and in-tunnel measurements from the pinned WarpScout report; mihomo separately checks HTTPS and UDP DNS. Adding another speed-test EXE is unnecessary now. |
| [ampetelin/warp-endpoint-checker](https://github.com/ampetelin/warp-endpoint-checker) | Credited IPv4 prefix source. Both public repository metadata and README requests returned HTTP 404 during this assessment. | Original current implementation could not be independently inspected. Verified the actual 14 prefixes in pinned WarpScout `pools.go` against Tropa's endpoint allowlist instead. |
| [TheyCallMeSecond/WARP-Endpoint-IP](https://github.com/TheyCallMeSecond/WARP-Endpoint-IP) | Shell script samples IPv6 endpoints in `2606:4700:d0::/48` and `2606:4700:d1::/48`, matching WarpScout's built-in IPv6 pools. | Candidate for a separate IPv6 feature. Tropa currently validates IPv4 endpoints using `SocketAddrV4`; it does not gain IPv6 support merely because WarpScout can scan it. Test actual IPv6 connectivity and retain IPv4 fallback before exposing it. |
| [SagePtr/mini_quic_generator](https://github.com/SagePtr/mini_quic_generator) | QUIC Initial packet builder; WarpScout has a Go port in `quic.go`, generates a CPS I1 expression and tests packet decryption. | Already reached via `-gen-i1 quic` and exact I1 preservation. This is a pre-handshake AWG signature packet, not a QUIC/MASQUE data tunnel or evidence that game traffic becomes HTTP/3. No separate browser/JavaScript generator is needed. |
| [Diniboy1123/usque](https://github.com/Diniboy1123/usque) | WARP MASQUE implementation with HTTP/3/QUIC and HTTP/2/TCP options. Pinned WarpScout imports its API and carries separate MASQUE endpoint pools and ECDSA P-256 enrollment logic. | Primary reference for future MASQUE transport. Needs its own identity/configuration handling and a supervised runtime adapter. For games, verify UDP forwarding using full SOCKS5 or native TUN; the documented L4 proxy modes are TCP-only. HTTP/2 can provide an outer TCP alternative, but its latency/loss behavior must be measured rather than assumed better. |
| [nellimonix/base-relay](https://github.com/nellimonix/base-relay) | Reverse HTTP proxy for a configured upstream, with Vercel/Netlify/Workers adapters. WarpScout's registration fallback tries direct API, then configured relay, then its tunnel fallback. | Addresses API reachability, not game transport. Current Tropa registrar calls Cloudflare directly and does not inherit this fallback just by using the scanner. If added, make the relay explicit/configurable and account for forwarded bearer credentials. Keypairs should remain locally generated; only enrollment/public-key material belongs in the API request. No hosted relay was added or deployed. |
| [amnezia-vpn/amneziawg-go](https://github.com/amnezia-vpn/amneziawg-go) | Userspace AWG engine used in WarpScout. Latest upstream README describes features beyond the pinned implementation, including newer header/padding options. | Already used indirectly during scanning; production connections use pinned mihomo. Preserve only the parameters the Cloudflare side and our runtime support. Arbitrary server-side header/padding changes cannot be assumed compatible with Cloudflare WARP. A successful WarpScout check still requires validation using Tropa's actual runtime. |
| [charmbracelet/bubbletea](https://github.com/charmbracelet/bubbletea) | Terminal UI framework. Pinned WarpScout uses v1.3.10; current upstream README describes v2. | Not a networking component or a replacement for our React/Tauri GUI. Retain the scanner's noninteractive plain/report integration. No additional terminal window or TUI embedding is needed. |

## Why a scan tests 70 addresses

Pinned WarpScout contains 14 WG/AWG IPv4 `/24` pools and defaults to five sampled addresses per pool: 14 × 5 = 70. `expandV4` shuffles addresses before sampling. A later run can choose other addresses. This count concerns probe candidates, not a complete list of physical servers. The same subnet can reach multiple nodes; a node's location is distinct from the exit IP's observed country. MASQUE uses other dedicated pools and must not reuse the WG/AWG address assumptions.

Tropa's earlier two-result cutoff was its own integration policy, unrelated to those pools. The expanded table integration removed that cutoff. Different working counts between standalone WarpScout and Tropa can still reflect sampled addresses, test time, transport parameters, different runtimes, additional HTTPS/DNS criteria and the extended-search time budget. Reproduce with the same exported profile, account, endpoint and measurement target before concluding that a lower count is a scanner defect.

## Versions and source availability

WarpScout's pinned `go.mod` names `amneziawg-go` commit `c1e9bb3758e7`, `usque` commit `6aa03fc97d12`, and Bubble Tea v1.3.10. The available current README versions are not proof those exact versions implement every advertised newer feature.

Additional reviewed source snapshots:

- CloudflareWarpSpeedTest: `4d00b536b9fd883c4410f08a2e119b530ab8e4d5`, `task/warping.go` and `task/ip.go`.
- WARP-Endpoint-IP: tree `406388957f595a139fceb45fc0753f014940a67d`, `ip.sh`.
- mini_quic_generator: `21d15960bc9cf869114d214aa114384d0fcca92f`, `script.js` and `LICENSE`.
- base-relay: `3ede6508496a5183140d85c262d53b4d50a304da`, `src/proxy.js`.

Research copies and README blob identifiers are in `.artifacts/research/warpscout-credits-2026-10-27/`. The IPv4 source's HTTP 404 is an access result, not proof of why the repository is unavailable.

## Licensing observations

Credits are not a complete license inventory. CloudflareWarpSpeedTest currently declares GPL-3.0. The pinned AWG and usque license files were inspected and contain MIT terms. The current mini_quic_generator LICENSE includes an MIT text plus additional research/use clauses, and GitHub classifies it as NOASSERTION. base-relay's inspected root tree has no license file and repository metadata provides no license. These facts require source/version provenance review before directly copying their code or presenting the complete distribution as license-audited; they do not establish the terms of every historical version used by WarpScout. No licensing compatibility conclusion is inferred solely from credits or current repository metadata.

## Recommended follow-up

First compare repeated same-profile WarpScout and mihomo measurements to explain result-count differences and tune extended-search time. Next design a separate MASQUE identity/runtime adapter with explicit UDP testing, then IPv6 discovery/runtime support. Consider configurable registration fallback if direct registration actually fails. Continue using WarpScout for scanning rather than adding every credited executable as a new Tropa dependency.
