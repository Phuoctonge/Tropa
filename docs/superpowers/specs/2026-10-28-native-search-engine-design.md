# Tropa owned search engine: source review and proposed design

Status: architecture approved by the user on 2026-10-28. Protocol adapters, the worker/client and discovery source integration now exist; package removal of the legacy CLI and full application acceptance remain open. See `../../tropa-status-and-roadmap-2026-10-28.md` for current evidence and remaining work; historical findings below describe their original review stage.

## Required outcome

Remove the shipped WarpScout CLI, command construction, terminal-output parsing and temporary scanner account/report/config exchange. Tropa owns its search API and calls protocol libraries directly through its own Go component. Preserve WG, AWG, MASQUE H3/H2, parameter tuning, IPv4/IPv6 endpoint pools, manual selection, restrictions, progress, cancellation and portable lifecycle. Do not rewrite existing registration, UI, catalog, health monitoring or connection ownership.

This proposal supersedes the external-WarpScout architecture assumption in `../plans/2026-10-28-tropa-next-milestones.md` for search implementation only. That plan's other requirements and unverified native gates remain in force. The user explicitly requested this analysis and migration plan; implementation and public distribution are separate actions.

## Evidence and limits

Existing pinned WarpScout v0.16.1 source: commit `9e2ce9570625757ba24b390ad680dccd5e7f2707`, `.artifacts/warpscout-source/`. The runtime binary remains SHA-256 `01f11965da6415b08a23c334ea055edbda7298d46f717c2065ca94c538fb730c` until cutover. New source-only research is under `.artifacts/research/scanner-replacement-2026-10-28/`; each candidate has `origin.json` and `tree.json`. Source downloads were read as data, not compiled or run.

The GitHub search covered the first 50 newest upstream forks and repository searches for `warpscout language:Rust` and `warp scanner language:Rust`. The first query returned no repository; this is not proof that no Rust rewrite exists. Candidate source review was selective, not a complete security, license or Windows compatibility audit.

| Project and inspected revision | Verified source findings | Decision |
| --- | --- | --- |
| [warpscout-android](https://github.com/openwarpkit/warpscout-android/tree/463b897813c6fc740f9868b82bf5d609ba7b39e7) | `core/engine.go` defines Backend, Scan, FindJunk, FindSNI, EventSink and Cancel; `core/types.go` has structured results; `mobileapi/bridge.go` dispatches schema-versioned requests; `internal/warpscout/mobile_backend.go` calls scan functions without launching a CLI. | Strongest API/extraction reference. Reuse selected code with attribution, not Android application code. |
| Same Android backend | Engine permits one operation; backend methods lock a mutex and mutate package globals; it still imports Bubble Tea messages. Its bridge declares upstreamVersion v0.16.0 despite the shared Go module version requirements. | Do not assume thread safety, latest upstream parity or freedom from TUI dependencies. Compare against our pinned v0.16.1. |
| [warpscout-gui](https://github.com/3inker/warpscout-gui/tree/cb763439d521735c377e65623533f476a60749ab) | README describes Windows-only portable GUI based on Android work; `pkg/warpscout/mobile_backend.go` retains the same serialized backend pattern. | Useful Windows extraction reference; no demonstrated solution to shared-state parallelism. Root license provenance needs checking before copying fork-specific changes. |
| [cf-scanner](https://github.com/mahyartdb/cf-scanner/tree/3fa7c9b4911e45b5192a7d56b61882c95de72e8e) | Rust library; `warp.rs` uses boringtun, socket caching, bounded sends and Amnezia parameter transforms. Manifest declares MIT. | Useful Rust probing reference. Full parity with our AWG I1 profiles, MASQUE H3/H2 and runtime verification is not established. Do not adopt as a complete replacement. |
| [Aether](https://github.com/najishab/Aether/tree/8f3a9eb679eea3e2837e779bd119462001235ac8) | Rust lib/staticlib/cdylib; exported C functions and job handles in ffi.rs, WG prober, H2 implementation; depends on local quiche, boring/tokio-boring, smoltcp and boringtun. Root license is AGPL-3.0-or-later. | Demonstrates a Rust implementation path; not a low-cost Windows replacement or a license-neutral transplant. No code adoption in this proposal. |
| [warpscout-multi](https://github.com/MimiKemo/warpscout-multi/tree/b49ef5e8a466164a64d669951521b58944346bc8) | Inspected README, manifest and tree. | Discovery lead only; those files do not prove a reusable API or isolated parallel sessions. |

## Component map

| WarpScout files or dependency | Responsibility | Tropa destination |
| --- | --- | --- |
| pools.go, discovery.go, workers.go | Address/port sampling, reachability, worker limits | Rust owns deterministic targets and budgets; Go performs bounded network probes. |
| tunnel.go, bind.go, warp.go; `amneziawg-go` | WG/AWG devices, userspace stack, handshake and traffic | Go session adapters, direct library calls. Preserve BatchSize=1 scan bind behavior where applicable. |
| masque.go; `usque/api`, `connect-ip-go`, `quic-go` | EC TLS identity, CONNECT-IP H3/H2, packet pumps | Go MASQUE session adapter using the existing enrolled identity and peer verification. |
| gVisor netstack via AmneziaWG | TCP/UDP/IP traffic within a probe tunnel | Retain supported Go stack initially; no Windows TUN or route modification during discovery. |
| i1gen.go, quic.go, findjunk.go, findsni.go | AWG I1/junk and MASQUE SNI search | Owned Go tuning adapters; Rust schedules variants and stores actual parameters. |
| meta.go, geo.go, report.go | Node/exit metadata, sorting, presentation | Structured Go measurements; Rust maps to existing catalog. No formatted report parser. |
| register.go; registration portions of masque.go | Account creation and fallback | Keep existing Rust WG/EC registration. Worker receives only the identity needed for its session. |
| Bubble Tea, Bubbles, Lip Gloss, CLI flags, console and Android UI | Presentation and platform bridge | Exclude from the worker dependency graph. Replacing tea.Msg is necessary, not merely deleting an import. |
| base-relay | Remote registration fallback reference | No local program to embed; do not add another registration owner. |

Start from the pinned versions already used: Go 1.26.3 source baseline; amneziawg-go `v0.2.20-0.20260709022233-c1e9bb3758e7`, usque `v1.5.1-0.20260720063354-6aa03fc97d12`, connect-ip-go `v0.0.0-20260613064811-66cba32d7d33`, quic-go `v0.60.0`. Verify their exact API and module checksums while extracting; do not update dependencies as a side effect.

## Architecture choice

1. **Recommended: Rust coordinator plus owned Go worker.** Go imports protocol libraries directly. Rust communicates over inherited anonymous pipes using a versioned machine protocol. There is no WarpScout executable, shell command interface, report scraping, listening TCP service or permanent daemon. The worker is a permanent architecture component, not an intermediate wrapper around WarpScout main().
2. **Go shared DLL via C ABI.** Also permits direct library reuse, but requires explicit memory ownership, callback lifetimes, cancellation and Go runtime handling inside the Tauri process. A fatal worker runtime fault can affect the whole app; unloading is not a routine cancellation mechanism. No current requirement justifies this boundary.
3. **Full Rust protocol engine.** Possible, with references above, but requires parity work for AWG transforms, QUIC/CONNECT-IP, TLS identity, IP stack and Windows builds. This creates the most protocol maintenance and dependency change now.

An owned worker still is a separate EXE in the portable folder. The difference is that its implementation is our library host, and its sole public interface is structured IPC. It must never execute WarpScout internally or render its terminal dashboard. This boundary lets future Rust adapters replace individual Go adapters without rewriting UI, catalog or scheduling.

## Ownership and concurrency

Rust remains the sole owner of registration, restrictions, target generation, global deadlines, selection, catalog persistence and final mihomo validation. Go owns transport session resources, per-request packet pumps and bounded probe measurements. Request-local SessionConfig replaces package globals including identity, pools, protocol, outer transport, SNI, I1 and meta target.

Use one worker per search session, maximum two concurrent probe jobs and eight live tunnel instances globally. Round-robin initial work across all allowed transports before expansion; IPv6 has its own target family. Within each job at most four tunnel instances are active. A job is one immutable protocol/parameter/family batch. Rust alone issues additional jobs. Keep 180-second quick and 600-second extended overall budgets, including validation and cleanup. Reserve 45 seconds quick / 90 seconds extended for runtime validation; stop discovery earlier to retain this reserve. These are upper bounds, not promised connection times.

Initially, keep the existing search/core mutual exclusion. Do not scan through an active application tunnel or modify its configuration. The worker finishes and releases its reserved ownership before mihomo validation starts. Concurrent scan jobs are userspace sessions inside that search ownership, not a second service/core lifecycle. This deliberate phase boundary avoids fighting the existing reserve_sidecar lock. Future overlap requires separate explicit design and route-context verification.

## API contract

`protocol/search-v1.json` will be the shared wire contract. Frames are 4-byte big-endian payload length followed by UTF-8 JSON, maximum 262144 bytes. All commands and events include `version=1`, searchId, jobId and a monotonically increasing per-job event sequence. Commands: hello, probe, cancel, shutdown. Events: hello, accepted, progress, candidate, jobFinished, error. Candidate frames carry normalized endpoint, transport, concrete tuning parameters and measurements; never credentials. Terminal errors use stage/reason/retryable fields mapped into WarpFailure, not backend error strings.

Hello returns exact build/source identity and capabilities, including transports and supported tuning types. Reject a version/capability mismatch before transmitting account material. Identity travels only through inherited pipes to the owned child, not argv, environment, reports or temporary scan account files. The worker never persists registration or logs request bodies. Raw internal failures remain outside UI and diagnostic exports.

The implemented v1 hello optionally carries `parameterSets` (at most 32 named concrete transport/parameter tuples) and `unavailableTuning` (named limitation/reason pairs). They contain no identity. The Go component prepares a finite catalog; Rust validates each tuple against its runtime profile rules and retains exact values before scheduling immutable probe batches. A missing catalog remains compatible with older handshake fixtures but cannot establish tuning support. Available names cannot collide with unavailable names. DNS/SIP/STUN I1, bounded junk tiers and default-first MASQUE SNI values are represented; the copied mini_quic_generator implementation remains excluded pending its provenance/license gate.

Control input remains readable while probes run. Cancel returns a terminal cancelled job only after its sockets, devices and pumps close. EOF cancels all jobs. On cancellation allow 2 seconds for cooperative termination, then stop the owned Job Object and wait up to 5 seconds for OS exit confirmation; unresolved exit becomes cleanupPending and blocks further core startup. Apply backpressure to result delivery; coalesce progress counters, never silently drop candidate or terminal events. The pending event queue is bounded at 128 frames.

## Search and measurement semantics

Always check eligible cached profiles and standard MASQUE endpoints deterministically, including H2 `162.159.198.2:443` and `162.159.198.1:443`, before random expansion. Include the four known explicit IPv6 MASQUE hosts only when outer IPv6 is available. Standard endpoints remain subject to transport, node and country restrictions and full runtime checks; they are not privileged success results.

Probe results distinguish address reachability, handshake, HTTP data, HTTPS data and UDP data. Record absent measurements as null; an ICMP timeout is not proof of tunnel failure. Node country and exit country are different fields. Measure loss, jitter and sample counts with the same target and method before ranking. Carry network freshness context and measurement provenance into the existing catalog.

Do not return only one scanner winner. Preserve all measured candidates up to the existing catalog limit (256 scanned, 128 verified), with rejected and untested states. For quick validation interleave canonical candidates with the best measured alternatives per allowed transport; never exhaust the reserve on one failed candidate. All candidates used for connection must pass the existing mihomo HTTPS/WARP and UDP checks. Show best of checked, not a global-best promise. Manual pins remain exact; search never silently replaces them.

The native evidence in `../../validation/split-tunneling-udp-ipv6.md` motivates this requirement: our EC registration and pinned mihomo passed HTTP/HTTPS on standard H2 .198.2:443, while scanner-selected .199.38:4443 passed HTTP but timed out on HTTPS. This proves a selection/verification mismatch, not complete H2 TUN support.

## Licensing and provenance

WarpScout's inspected root MIT permits adaptation with preservation of its copyright and permission notice. Porting code to Rust does not remove those obligations. Preserve Android and other contributors' applicable notices for copied changes, with file-level source revision tracking. A credits link alone is insufficient. Inspect exact module licenses and source headers for the worker's actual build graph, not the old CLI graph.

The QUIC Initial generator has an existing unresolved provenance/license question. Do not remove its profile silently or assert clearance. Verify the exact copied implementation and terms before shipping it; record the feature as unavailable if clearance is unresolved, making that a failed full-parity release gate. Aether code is not incorporated by this design. Removing WarpScout does not remove unrelated mihomo, font or frontend license gates.

## Cutover and acceptance

Build the new worker from pinned source; do not improve or fork the old CLI as an intermediate production backend. Retain the original executable only in research fixtures for bounded reference comparison. Replace the production adapter only after the structured pipeline passes contract, four-transport, cancellation, exact-policy and native cleanup checks. Delete the CLI parser and production report/account export path at cutover; update package allowlists and provenance rather than merely renaming warpscout.exe.

Required evidence: two simultaneous sessions with distinct accounts/parameters do not interfere; cancellation is observed while events are emitted; job terminal frames and OS exit agree; no scanner secret/report files appear; all allowed protocols receive budget; canonical H2 is tested deterministically; every chosen profile is reproducible in mihomo; manual pins and restrictions survive; UAC native TCP split returns to baseline after disconnect. External UDP/IPv6 and clean-Windows gates remain explicit when no test facility exists. Do not claim those from userspace DNS or loopback tests.
