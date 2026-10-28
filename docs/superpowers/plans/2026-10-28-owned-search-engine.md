# Owned Search Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Delegation is not requested; execute inline with checkpoints.

**Goal:** Replace the production WarpScout CLI with an owned, structured search engine using protocol libraries directly.

**Architecture:** Rust owns policy, scheduling, persistence and connection decisions. A small Go worker hosts isolated userspace WG/AWG/MASQUE sessions and exchanges framed commands/events over inherited pipes. Final validation and application connection continue through the existing mihomo lifecycle.

**Tech Stack:** Rust/Tauri, Go 1.26.3 source baseline, amneziawg-go, usque/api, connect-ip-go, quic-go, existing React UI and pinned mihomo 1.19.33.

**Spec:** [Source review and proposed design](../specs/2026-10-28-native-search-engine-design.md). Architecture approved by the user on 2026-10-28. Implementation is in progress; completion is tracked below.

## Global Constraints

- Portable folder; no installer or permanently installed service. Own Go EXE is allowed; no WarpScout execution inside it.
- Keep existing registration, catalog, filters, manual pins, connection ownership and UI contracts.
- Maximum two concurrent probe jobs and eight live tunnels globally; maximum four tunnels per job.
- Quick overall budget 180 seconds; extended 600 seconds; reserve 45 / 90 seconds respectively for runtime validation. Include cleanup in the overall accounting.
- Protocol frames: 4-byte big-endian length, UTF-8 JSON, maximum 262144 bytes, version 1. Pending event queue: 128 frames.
- Cooperative worker stop: 2 seconds; owned forced-exit confirmation: up to 5 seconds. Unconfirmed exit is cleanupPending.
- No credentials in argv, environment, logs, report files, UI events, snapshots or ZIPs. Send required identity through owned inherited pipes only.
- Search cannot mutate an active Tropa connection. Finish/release worker ownership before existing mihomo validation acquires ownership.
- Preserve dependency versions listed in the spec, applicable notices and exact source attribution. No speculative all-Rust rewrite.
- Do not execute downloaded third-party applications during implementation. Native tests use our own built components; official WARP/router configuration stays unchanged.
- Preserve unrelated dirty work. No commits, PR or publication is implied by this plan. Root AGENTS issue/approval contract applies before nontrivial product implementation.

## File responsibilities

| Path | Responsibility |
| --- | --- |
| protocol/search-v1.json | Exact command/event schema and measurement semantics |
| engine/go.mod, engine/go.sum | Owned Go module `tropa.local/probe`, pinned dependencies |
| engine/cmd/tropa-probe/main.go | Pipe host and process-level cancellation only |
| engine/internal/wire/{frame,types}.go | Bounded framing and typed DTOs |
| engine/internal/probe/{session,wg,masque,measure,tuning}.go | Session-owned protocol resources and measurements |
| src-tauri/src/core/warp_engine/{mod,wire,process}.rs | Typed Rust client and owned process integration |
| src-tauri/src/core/warp_targets.rs | Versioned address pools and deterministic canonical targets |
| Existing warp_search.rs and warp_discovery.rs | Fair scheduling, result ingestion and mihomo validation |
| Existing warp_registration.rs / warp_masque_registration.rs | Retained registration ownership |
| scripts/build-tropa-engine.py | Reproducible worker build and source/hash evidence |
| Existing scripts/tropa-components.json and package-tropa.py | Replace production component and verify package contents |

Tests live beside the changed Go/Rust units and extend existing package verification tests. Add only regressions for contract loss, shared-state contamination, cancellation, search omission and packaging errors.

### Task 1: Provenance-backed minimal protocol module

**Files:** create `protocol/search-v1.json`, `engine/go.mod`, `engine/go.sum`, `engine/internal/wire/frame.go`, `engine/internal/wire/types.go`, `engine/internal/wire/frame_test.go`; record extracted-file origins in `engine/THIRD_PARTY_NOTICES.md` and `engine/source-origins.json`.

**Interfaces:** `ReadFrame(io.Reader) ([]byte,error)` and `WriteFrame(io.Writer,[]byte) error`; `Command{Version,SearchID,JobID,Type,Payload}`; `Event{Version,SearchID,JobID,Sequence,Type,Payload}`. Payload is json.RawMessage; command variants are hello/probe/cancel/shutdown. Reject unknown variants and missing required IDs. Payload schemas explicitly distinguish WG and EC identity; events contain none.

- [ ] Record SHA-256 and pinned revision of each selected source file, including Android Backend/EventSink ideas; preserve MIT notices. Inventory direct module license/source headers before extracting implementations.
- [ ] Write frame regression with `ReadFrame(bytes.NewReader([]byte{0,4,0,1}))`: it must reject an oversized length before allocating/reading payload. Add fragmented-read roundtrip and EOF-mid-frame checks.
- [ ] Run `cd engine; go test ./internal/wire -run TestFrame -count=1`; confirm the new checks fail for missing implementation.
- [ ] Implement bounded framing using `io.ReadFull`, `binary.BigEndian.Uint32`, an explicit 262144-byte limit, and full writes. No line scanner or terminal output.
- [ ] Define hello capabilities, bounded ProbeRequest and redacted Candidate/Error payloads in both schema and Go types. Encode absent metrics as pointers/null, not zero.
- [ ] Re-run targeted tests and verify serialized events cannot contain the synthetic key/token supplied in a request. Record exact linked-module licenses; flag QUIC generator clearance separately.

### Task 2: Isolated WG/AWG sessions and structured measurements

**Files:** create `engine/internal/probe/session.go`, `wg.go`, `measure.go`, `wg_test.go`; adapt selected pinned tunnel.go/bind.go/warp.go fragments with their notices.

**Interfaces:** `Probe(ctx context.Context, cfg SessionConfig, emit func(Candidate) error) error`; immutable `SessionConfig{JobID,Transport,Identity,Targets,AWG,SNI,Limits}`; `Candidate{Endpoint,Transport,Parameters,Stages,Metrics,NodeCountry,ExitCountry}`. Targets and limits arrive from Rust. Identity is per-session, never a package singleton.

- [ ] Write a two-session bind regression: fake device factories capture UAPI configuration for distinct synthetic private keys, endpoints and I1 values; each factory must observe only its own inputs after concurrent starts.
- [ ] Run `go test ./internal/probe -run TestSessionIsolation -count=1`; capture the failing result before adding sessions.
- [ ] Extract the existing device-bind and userspace stack logic into Session ownership. Keep `BatchSize() int { return 1 }`; assign every socket/device/goroutine to Close. Replace old package globals with cfg fields.
- [ ] Implement stage reporting for handshake, HTTP, HTTPS and UDP. HTTPS requires `warp=on|plus`; handshake alone cannot mark Candidate runtime-verified. Methods and sample counts accompany RTT/loss/jitter. Loss percent remains 0..100.
- [ ] Use context-aware target queues and a session Close wait, with a hard four-tunnel cap per request. Blocked event emission must unblock on context cancellation.
- [ ] Run isolation and cancellation checks under `go test -race ./internal/probe -count=1`. Race detector availability on Windows may require the documented host toolchain; record unsupported execution explicitly.

### Task 3: MASQUE and tuning adapters with the same contract

**Files:** create `engine/internal/probe/masque.go`, `tuning.go`, `masque_test.go`; extend SessionConfig parameter variants. Extract only protocol/tuning code, not registration, TUI, config rendering or SOCKS server commands.

**Interfaces:** retain Task 2 Probe API; transport enum is wg/awg/masqueH3/masqueH2. MASQUE identity contains enrolled EC private DER, pinned peer public DER and interface addresses. Parameters carry actual SNI and actual AWG jc/jmin/jmax/i1 values.

- [ ] Add a dial-lifetime regression: simulated CONNECT-IP remains open after dial success until session cancellation, then its pump and connection both close. Add concurrent H3/H2 session configuration isolation using distinct SNI/identity values.
- [ ] Run `go test ./internal/probe -run 'TestMasque|TestTuning' -count=1` and observe failures for absent adapters.
- [ ] Use `usque/api.ConnectTunnel` and pinned connect-ip-go/quic-go APIs as in the reviewed implementation. The dial timeout must stop after successful establishment without cancelling the lifetime context of an H2 stream. Verify peer key; no skip-verification fallback.
- [ ] Adapt finite junk/I1 and SNI searches to immutable configuration. Return actual generated values with every result. Replace Bubble Tea event messages with the Task 1 DTOs and remove terminal dependencies from imports.
- [ ] Test separate failures for connect timeout, HTTP-only success, HTTPS failure and cancellation. Confirm optional missing metrics remain null and node country is not populated from exit country.
- [ ] Run `go list -deps ./cmd/tropa-probe` after the host exists; require no charmbracelet CLI/TUI packages and no WarpScout main package. QUIC I1 inclusion requires resolved provenance; otherwise record explicit unavailable capability and keep full-parity acceptance open.

### Task 4: Owned streaming worker and Rust client

**Files:** create `engine/cmd/tropa-probe/main.go`, `src-tauri/src/core/warp_engine/{mod,wire,process}.rs`; register module in `src-tauri/src/core/mod.rs`; extend existing owned-child tracking rather than replacing service lifecycle.

**Interfaces:** Rust `EngineClient::start(root: PathBuf, cancellation: ProbeCancellation) -> Result<Self>`; async `submit(&mut self, request: ProbeRequest) -> Result<()>`, `next_event(&mut self) -> Result<EngineEvent>`, `cancel(&mut self, job_id: &str) -> Result<()>`, `shutdown(self) -> Result<()>`. Single event reader; control writes serialized. Schema enums mirror Task 1 exactly.

- [ ] Add cross-language golden frames for hello, candidate(null RTT), terminal cancelled and protocol-version rejection. Test out-of-order sequence and foreign searchId rejection before catalog mutation.
- [ ] Implement Go host with an input-reader goroutine, per-job contexts, two active-job cap, bounded event writer queue and EOF cancellation. stdout is exclusively framed events; stderr is sanitized diagnostic codes.
- [ ] Rust launches the exact pinned worker path without a shell or network listener, sends hello first, checks build/capabilities, then transmits identity. Use hidden Windows process creation and existing Job Object ownership.
- [ ] Add process regressions: cancellation while progress is flowing, worker EOF, malformed frame, panic exit and slow reader. Terminal events cannot replace OS exit confirmation. Credentials must not enter process command line or test output.
- [ ] Run `go test ./...` in engine and the targeted Rust warp_engine tests using existing portable test features. Confirm no temporary account/report/export files are created.
- [ ] Exercise the 2-second graceful / 5-second forced cleanup paths with our synthetic worker fixture. A hung owned child must leave cleanupPending and block a new core start.

### Task 5: Deterministic targets and fair search integration

**Files:** create `src-tauri/src/core/warp_targets.rs`; modify existing `warp_search.rs`, `warp_discovery.rs`, `warp_diagnostics.rs`; retain catalog/selection models and existing frontend commands.

**Interfaces:** `targets(kind: TransportKind, ipv6: bool) -> Vec<SocketAddr>` emits eligible canonical endpoints first, deduplicated with sampled expansion. `EngineClient` candidate events map into existing WarpProfile/ScannerObservation; existing `run_warp_transport_probe_profile_timed` remains the runtime authority.

- [ ] Add the observed regression: H2 canonical `162.159.198.2:443` remains in the target set even when a deterministic random sampler yields only .199.38 and .199.155. Include .198.1:443 and explicit IPv6 hosts for the appropriate family, never enumerate /48 exhaustively.
- [ ] Add policy/fairness regression: when all four transports are allowed, each gets an initial batch before expansion; when only H2 is allowed, WG/AWG receive no identity/job. Unknown required geography cannot authorize connection.
- [ ] Run new target/scheduler tests red. Implement immutable Rust batches with per-transport budgets, two active jobs, network-context tagging and streamed candidate persistence within existing catalog limits.
- [ ] Remove scan command construction from the production discovery path. Finish the worker and confirm teardown before running mihomo probes. Preserve canonical candidates and measured alternatives, not one exported winner.
- [ ] Reserve 45/90 seconds for validation; interleave canonical and measured candidates across allowed transports, with linked cancellation and existing cleanup. Validate through mihomo HTTPS and UDP; do not treat worker Working=true as final verification.
- [ ] Extend existing manual-pin, restriction and incomplete-check regressions. Reuse the current UI fields; only update transport-progress mapping if necessary. No unrelated interface redesign.
- [ ] Run the full existing Rust library test command, frontend tests and TypeScript checks if shared contracts changed. Report tests actually run, not projected totals.

### Task 6: Source-bound packaging and removal of the CLI

**Files:** create `scripts/build-tropa-engine.py`; modify existing build/package/component-manifest verification, `src-tauri/src/utils/resolve.rs`, `warp_diagnostics.rs`, `THIRD_PARTY_NOTICES.md`, `docs/third-party-components.md`, portable build docs; delete production `warp_scout.rs` after replacing remaining presets/tuning types with owned engine types.

**Interfaces:** worker build record stores exact source snapshot, Go toolchain, module graph, flags, source origin and output SHA-256. Package includes `resources/tropa-probe.exe` with verified build record; normal EXE is built without diagnostic flags.

- [ ] Extend existing package tests to fail when warpscout.exe or old scanner account/report/export files are included, or when worker bytes do not match source-bound build evidence.
- [ ] Build worker using pinned module sums and `go build -trimpath -mod=readonly -o ../.artifacts/engine-build/windows-x64/tropa-probe.exe ./cmd/tropa-probe` from `engine/`; the build script creates and resolves that staging directory first. Capture `go version -m` and build metadata; never interpolate a user-supplied shell path.
- [ ] Remove legacy scanner download/pin requirements from production packaging only. Keep historical archives and research evidence intact. Preserve applicable WarpScout/Android notices for adapted code after removing the executable.
- [ ] Audit actual new dependency graph and embedded assets. Do not reuse the old CLI notice collection as proof of the new binary's obligations or clear unrelated release gates.
- [ ] Run package omission/substitution regressions, create a new uniquely named portable archive and independently inspect contents. Do not label this a public-ready release while signing/source/license/clean-Windows gates remain open.

### Task 7: Native parity and cutover acceptance

**Files:** extend `docs/validation/split-tunneling-udp-ipv6.md` with new exact build/run evidence; preserve reports under a new `.artifacts/native-connection/` run directory.

**Interfaces:** same existing `--tropa-smoke --tropa-split-smoke` diagnostic workflow on a newly source-bound build; new engine is the only production search path.

- [ ] Run default search and individually restricted WG/AWG/H3/H2 using the same network conditions. Record unavailable transports honestly; compare identical explicit targets/parameters when using original WarpScout as a separately authorized research reference.
- [ ] Reproduce standard H2 HTTP/HTTPS success through the worker and mihomo; then run the native selected/direct EXE TCP smoke on H2. Require before/connected/after observations and network restoration, not only warp=on through a loopback proxy.
- [ ] Verify cancellation during both protocols and app exit leaves no worker/core/scan files; verify account hashes are unchanged and manual pins are not replaced.
- [ ] Compare runtime and memory for bounded identical workloads; require deterministic inclusion of canonical targets and fair four-transport work before evaluating speed claims.
- [ ] Keep external UDP/IPv6, browser, long-session and anti-cheat gates explicitly unproven without their required facilities. This migration does not finish the full Tropa objective.
- [ ] Search production code/build inputs for warpscout.exe, old CLI flags, report parsing and scanner secret-export paths. Review each surviving attribution/research mention; executable dependencies must be absent. Record cutover only after source, package and native evidence agree.

## Review and execution order

Review the proposed worker boundary and isolated-session contract first. Execute Tasks 1–4 as the reusable engine foundation, then Task 5 switches search, Task 6 removes the distribution dependency, and Task 7 establishes native behavior. Changes to the old CLI adapter beyond urgent correctness fixes are outside this plan. No recommendation requires first optimizing a temporary WarpScout integration and then throwing that work away.


## Initial execution checkpoint: 2026-10-28

Task 1 is in progress. Added the owned Go module, framed I/O, command decoding, identity/transport DTOs, public result DTOs, JSON Schema and source-reference notices. The module currently uses only the Go standard library, so there is no go.sum yet. Official Go 1.26.3 Windows archive was SHA-256 verified and extracted to ignored .tools/go1.26.3; system installation was not changed.

Eight targeted Go tests and go vet pass. Missing functions were observed failing before implementation; invalid UTF-8 and empty-identity regressions were separately observed failing before correction. JSON Schema itself and valid requests for all four transports were validated with Python jsonschema; an error event containing a privateKey field was rejected. Evidence: .artifacts/validation/search-wire-2026-10-28/schema.json.

Protocol adapters, cross-language client/host, source-bound worker packaging and production cutover are not implemented. Current Tropa still uses the existing WarpScout integration. The next checkpoint completes exact schema/decoder contract parity and begins session-owned protocol adapters. No network or elevated application test was run for this checkpoint.

## WG/AWG foundation checkpoint: 2026-10-28

Task 2 is in progress. Added session-owned userspace WG/AWG device creation using the pinned amneziawg-go library and gVisor version from the reviewed WarpScout dependency graph. UAPI construction has no package-global identity or tuning state; keys must decode to 32 bytes and I1 cannot inject UAPI lines. Each session owns its single-packet UDP bind, device and cancellation lifecycle. WARP reserved/client-ID bytes are applied on send and normalized on receive. AWG I1-only profiles omit explicit jc=0 because the library rejects that UAPI assignment while its default is zero.

Fourteen Go tests pass with `go test ./... -count=1`; `go vet ./...` passes. Coverage added here includes concurrent configuration isolation, invalid keys/UAPI injection, cancellation releasing a real device's UDP socket, a local UDP reserved-byte round trip, and starting an actual I1-only AWG device. Missing adapter functions and the I1-only rejection were observed before implementation/correction. Required-field/reserved-array wire regressions now cover eight malformed requests. `go test -race ./internal/probe -count=1` could not execute because CGO is disabled; neither gcc nor clang was found on PATH. Race verification remains open.

The source adaptations and original file hashes are recorded in engine/source-origins.json. Nine imported module versions/checksums and their root license texts are recorded in engine/dependencies.json and engine/licenses. This is not final distribution clearance. No external WARP handshake or data-transfer test was run for this checkpoint; local userspace lifecycle tests do not prove transport reachability. Structured stage measurements, job concurrency limits, MASQUE adapters, worker host, Rust integration and production cutover remain to be implemented. The running product still depends on WarpScout CLI.

## WG/AWG probing checkpoint: 2026-10-28

Task 2 now includes a bounded target queue, immutable request snapshots, up to the request's four concurrent sessions, handshake polling and independent HTTP/HTTPS trace checks. `Probe(ctx, request, results chan<- wire.Candidate)` uses a cancellation-aware channel rather than an arbitrary emission callback so blocked result delivery cannot prevent cleanup. Each target's device is closed before its result is delivered. HTTP success cannot populate HTTPS success; HTTPS must return an exact warp=on or warp=plus trace with normal certificate validation. Redirects and oversized trace responses are rejected. Exit country is taken only from a verified HTTPS trace; node country and UDP remain null when unmeasured.

Successful HTTPS candidates additionally receive userspace IPv4 ICMP measurements to 1.1.1.1. Results include actual attempted samples, mean RTT, loss percent and mean absolute difference between successful RTTs, with an explicit target/method identifier. These are ICMP measurements, not game latency or proof of UDP forwarding. An unavailable ICMP response does not override a successful HTTPS stage.

Two actual local userspace peers, each with its own X25519 identity, completed encrypted handshake, five ICMP echo exchanges and TCP/HTTP transfer in both WG and AWG modes. This checks the real adapter/library/stack path without a system TUN or external server. It does not prove Cloudflare reachability or native application routing. Added regressions also reproduced and corrected shared mutable AWG parameters in delivered results and accidental rewriting of AWG I1 bytes when an I1 packet resembled a WG initiation header. The pinned library sends I1/junk before the real initiation in a single batch; only the actual initiation receives the WARP reserved bytes.

Fresh `go test ./... -count=1` passes all 23 top-level tests, including the WG/AWG peer subcases; `go vet ./...` and `go mod verify` pass. The earlier CGO/race limitation remains open. MASQUE/tuning, a streaming worker host, Rust client/scheduler integration, final mihomo validation, external UDP/IPv6 and production cutover remain outstanding. Global two-job/eight-tunnel enforcement belongs to the worker host and is not claimed from the per-job queue.

## MASQUE adapter checkpoint: 2026-10-28

Task 3 now has enrolled EC identity parsing, independent TLS configurations with mandatory peer public-key pinning, direct H3 CONNECT-IP, direct H2 CONNECT-IP, userspace network stacks and owned packet pumps. All four transport variants are dispatched by the common Probe queue. A successful CONNECT-IP response remains only the handshake stage; the same independent HTTP/HTTPS and ICMP measurement pipeline is used afterward. The dial deadline is stopped on success, while its context remains alive until session cancellation. Close cancels the request, closes transport/device resources and waits for both packet pumps.

Inspection found that upstream usque/api.ConnectTunnel's H2 branch honors environment proxies and does not expose the owning HTTP/2 transport. The owned H2 connector therefore adapts its direct-endpoint branch using connect-ip-go.DialH2 and x/net/http2, tracks the actual TCP connection and closes it explicitly. It never consults proxy environment variables. H3 continues to call usque/api.ConnectTunnel directly. No global environment or logger mutation was added to the probe library. The TLS wrapper additionally rejects an empty peer certificate chain, which upstream's pin callback accepts.

Real local TLS/HTTP2 and QUIC/HTTP3 fixtures exchanged IP datagrams using the production connectors. The H2 fixture continued transferring after the dial deadline expired and succeeded with an intentionally unusable HTTPS_PROXY in the test process. A separate H2 userspace-session cancellation test confirmed that both packet pumps finish. Two concurrent H2 sessions used distinct enrolled client identities and SNI; the server checked the correspondence and cancelling one did not stop the other. These are local protocol fixtures, not Cloudflare enrollment, external reachability, native TUN routing or external UDP evidence.

Fresh full Go verification passes 30 top-level tests, go vet and go mod verify. Test events are saved in .artifacts/validation/owned-engine-2026-10-28/masque-tests.jsonl. The actual imported graph has 21 third-party modules; versions/checksums/root license texts and copied-source origins were updated. No cobra, charmbracelet or WarpScout package appears in `go list -deps ./internal/probe`; this does not yet assert a finished worker binary dependency graph. Race verification still needs CGO. Automated I1/junk/SNI tuning, QUIC-generator provenance clearance, worker/Rust integration, global scheduling, native acceptance and removal of the production CLI remain unfinished.

## Streaming worker checkpoint: 2026-10-28

Task 4 now has an owned Go executable at engine/cmd/tropa-probe and a host using inherited input/output pipes. The control reader remains separate from probe jobs and the bounded 128-frame output queue. Hello is required before probes; one search identity is accepted per process, job IDs cannot be reused, two jobs may be active and each request is already limited to four tunnels. A third concurrent job receives a retryable scheduler/busy error. EOF and shutdown cancel all jobs; cancelled terminal events are emitted after the probe function returns and frees its resources. Slow result readers do not prevent the input reader from processing cancel. The process suppresses third-party standard logging and reports only a static failure code on stderr.

Host tests cover streaming candidate/progress/cancel ordering, terminal-after-cleanup, concurrent-job rejection, EOF, slow-reader cancellation, incompatible version, probe-before-hello, foreign search identity, and sanitization of returned errors and a panic from the injected probe entrypoint. This does not prove recovery from arbitrary library goroutine panics or the Rust forced-exit/cleanupPending paths, which remain open.

A development EXE was built with `go build -trimpath -mod=readonly -o ../.artifacts/engine-build/windows-x64/tropa-probe-dev.exe ./cmd/tropa-probe`. Its SHA-256 is 32289496765883e5a46444ac3a22da05cfd79f472ebf5971f9f8a4a2bcab301c and it advertises buildID=development, not a production source binding. Real subprocess pipe tests observed a WG initiation at a local UDP peer, cancelled the job and shut down; a second run closed stdin during probing, and a third rejected protocol version 2. All three OS exits were confirmed, no credential appeared in events/stderr, and the isolated working directories remained unchanged. Evidence: .artifacts/validation/owned-engine-2026-10-28/native-host/result.json; harness: .artifacts/engine-build/native-host-smoke.py. No external Cloudflare or native TUN routing test was performed.

Four shared framing fixtures now cover hello, a candidate with null metrics, cancellation and an incompatible command version. Exercising the candidate fixture against JSON Schema exposed a missing #/$defs/stages target; it was added with explicit boolean-or-null values and required keys. All schema references, shared fixtures and captured native events now validate. This closes that observed schema defect, not all future Go/Rust semantic parity work.

Fresh full Go verification passes 37 top-level tests, go vet and go mod verify. Logs: .artifacts/validation/owned-engine-2026-10-28/host-tests.jsonl and host-schema.json. The actual `go list -deps ./cmd/tropa-probe` graph matches the 21 recorded third-party module inventories and has no WarpScout, cobra or charmbracelet imports. Rust frame/sequence validation, owned-process integration, tuning, source-bound worker packaging and production cutover remain unfinished. The application still uses its existing WarpScout path.

## Rust IPC and process ownership checkpoint: 2026-10-28

Task 4 now includes Rust command serialization, bounded frame reading, typed event decoding and per-job state validation. Shared Go fixtures decode with nullable measurements preserved. Foreign search IDs, replayed/skipped sequences, mismatched candidate endpoints/parameters, post-terminal events, unknown secret fields and missing required nullable stage fields are rejected before catalog integration. Control payloads are objects, and identity is serialized only inside probe frames. Invalid or oversized commands are rejected before writing a frame header; errors do not include request values.

The Windows EngineClient hashes the supplied pinned artifact before launch, creates a hidden owned process with inherited pipes, reserves the existing core execution guard and assigns a kill-on-close Job Object. Hello/build/capabilities are checked before any identity can be submitted. A single reader and a bounded 128-event channel serve independently serialized control writes. Shutdown drains output while waiting, allows two seconds for cooperative exit, then closes the Job Object and requests termination with up to five seconds for OS confirmation. The existing process supervisor retains the execution lease after caller drop until Windows confirms exit. This uses a supplied artifact binding; the production source-bound build record and its package loader are still outstanding.

The native Rust-to-Go regression used the previously built worker with SHA-256 32289496765883e5a46444ac3a22da05cfd79f472ebf5971f9f8a4a2bcab301c and development build ID. It observed an actual 148-byte loopback WG initiation with reserved bytes, received cancellation after probe cleanup and confirmed OS exit. Mismatched hash/build bindings were rejected, and the isolated working directory stayed empty. Separate owned fixtures exercised a hung worker, oversized output header, unexpected exit and panic. An attempted concurrent core reservation failed while the worker was owned and succeeded after its confirmed termination. These fixtures are under engine/internal/host/testdata/worker, outside the production import graph, and are explicitly built with `go build -trimpath -mod=readonly -ldflags "-X main.mode=fixture-<hang|malformed|exit|panic>" -o ../.artifacts/engine-build/windows-x64/fixture-<mode>.exe ./internal/host/testdata/worker`.

A real blocked-pipe regression exposed reuse after an externally cancelled partial command write: the following cancel command stalled instead of rejecting the incomplete stream. The client now marks the stream unusable before awaiting a write and only clears that state on success. The regression was observed failing before the fix and passes afterward. Evidence: .artifacts/validation/owned-engine-2026-10-28/rust-native-write-cancel-red.txt and rust-engine-final.txt. All nine targeted Rust engine tests passed with the two normally ignored native fixtures explicitly included and serialized. Fresh Go verification again passed all 37 top-level tests, go vet and go mod verify; the standalone fault fixture also passed go vet. Go events are recorded in rust-client-go-tests.jsonl. Compiler warnings remain for fields and the artifact constructor not yet consumed by production integration; they were not suppressed.

This checkpoint does not prove an unkillable-process cleanupPending branch, external WARP reachability, native TUN split tunneling, full application exit, CGO race verification or production cutover. Tuning, source-bound packaging, deterministic targets/fair scheduling and catalog/runtime integration remain open. The normal application and existing ZIP still use the previous WarpScout integration.

## Source-bound worker checkpoint: 2026-10-28

Task 6 now has `scripts/build-tropa-engine.py` and `scripts/tropa_engine.py`. The controlled offline build fixes Go 1.26.3, windows/amd64/v1, CGO=0, trimpath, readonly modules and disabled VCS stamping. It snapshots owned Go sources, protocol fixtures/schema, dependency/license/source-origin records and the worker build scripts. Go's actual owned-package input lists must be covered by that snapshot; an unrecorded embedded file cannot silently enter the build. Module verification and the 21-module imported graph are checked against the existing inventory and license hashes. The record captures exact Go executable/compiler/linker/assembler/VERSION hashes, actual binary build metadata and normalized command arguments. It is local build evidence, not a signature, full toolchain distribution attestation or final license clearance.

The general application source snapshot now admits owned Go sources/module locks, protocol JSON and recorded notices while excluding engine runtime accounts/binaries. Rust's packaged-worker constructor reads a compile-time `TROPA_ENGINE_PIN`, checks protocol/source identity consistency and uses the fixed resources/tropa-probe.exe path. Application build tooling accepts `--engine-build`, verifies that worker record before and after compilation, clears any ambient pin and records the selected pin in application build evidence. This is preparation for cutover; discovery and package allowlists have not yet switched from WarpScout.

The actual worker at .artifacts/engine-build/source-2026-10-28-b/tropa-probe.exe has build ID source:f642b0033b0fa080c9681b76ba66a78004063758b30ca01784716d6ef6a04ce6, SHA-256 16520c0339f1072986d2c1eb70e52ef5b31c148fb65ca990e4ee8225f37a2ae9 and size 17,401,856 bytes. An independent repeat build at source-2026-10-28-b-repeat produced identical executable bytes. Both directories preserve source.zip and build.json. Native Rust tests compiled with this exact pin used a staged copy under the first directory's runtime/resources, observed its real loopback WG initiation and cancellation, and rejected altered hash/build identity before successful job use. All ten Rust engine tests, including the native fault fixtures, passed. Evidence: .artifacts/validation/owned-engine-2026-10-28/rust-source-bound-worker.txt.

All 31 Python build/source/PE/package tests passed; six new worker tests invoke the actual Go toolchain rather than mock executable bytes. Regressions first reproduced omitted embedded inputs and unchecked recorded build arguments, then passed after correction. Existing application source/binary and portable omission/substitution checks still pass. Evidence: python-source-bound-final.txt. This proves the tested build boundary and repeatability on this toolchain, not native external reachability or clean-Windows portability. No ordinary Tropa release EXE or ZIP was rebuilt at this checkpoint, and existing archives remain historical.

## Targets, fair scheduling and typed enrollment checkpoint: 2026-10-28

Task 5 now has the Rust target generator and two-job scheduler. Canonical H2 .198.2:443 and .198.1:443 precede sampled expansion; literal H3 IPv6 hosts and bounded WG/AWG/H2 pools retain the existing endpoint allowlists. Target sets are deterministic for a seed, deduplicated, family-specific and bounded below 5,000 entries. All enabled transport/family streams receive an initial batch before expansion. Per-transport active-time quotas and the discovery deadline preserve the overall 45/90-second runtime-validation and cleanup reserve. These modules are not yet connected to production discovery. Adapted pool/port file revisions and SHA-256 values are recorded in docs/third-party-components.md.

Runtime account objects can now produce typed ProbeRequest values directly from stored WG or enrolled EC keys. MASQUE retains base64 SEC1 private DER and SPKI peer DER instead of converting the peer to scanner PEM. Registration IDs/tokens are excluded; actual AWG and SNI parameters, assigned inner addresses, selected outer family, bounded limits and eligible targets are preserved. Identity-family mismatches and mixed outer families fail before submission. Existing WG registrations do not store client-ID/reserved bytes; this conversion retains their previous zero-reserved behavior and does not claim to recover unrecorded enrollment fields.

A new regression reproduced catalog rejection of I1-only AWG profiles (jc=0). Catalog validation now permits zero junk count when a valid nonempty I1 chain is present, while an AWG profile with neither I1 nor junk remains invalid. The inspected pinned mihomo source omits zero JC from UAPI; the owned Go adapter already does the same, avoiding the library's rejection of an explicit jc=0 assignment. The regression checks both runtime configuration and the worker request; it does not prove external transport success.

Fresh verification: core::warp_ passed 81 tests with two explicitly ignored native tests; config::warp_profiles passed 15 tests. Output is preserved under .artifacts/validation/owned-engine-2026-10-28/{rust-targets-identity-final.txt,rust-profile-i1-final.txt}. Scheduler warnings for fields awaiting integration and existing wire consumer warnings remain. Earlier targeted runtime verification passed seven tests before the I1-only regression was added. No external network, TUN, ordinary application build or portable ZIP was exercised here.

Remaining work includes finite AWG/I1/SNI tuning, the actual scheduler/client/account discovery loop, candidate provenance persistence, teardown before final mihomo validation, production CLI removal, source-bound packaging and native acceptance. The current shipped application still uses WarpScout; this checkpoint is implementation progress, not cutover or goal completion.

## Finite tuning capability checkpoint: 2026-10-28

Task 3 now includes engine/internal/probe/tuning.go: DNS with EDNS padding, SIP OPTIONS and STUN Binding CPS generation; the pinned default iCloud I1; I1-only and two bounded junk tiers; and five default-first SNI choices for each MASQUE transport. Every call returns independent parameter objects. There are 29 named concrete profiles across the four transports; no threshold-based global winner or unbounded random loop is carried over. QUIC I1 is explicitly unavailable with reason provenance-unresolved. The saved upstream mini_quic_generator license restricts use to isolated/user-owned infrastructure or prior written owner consent; the root MIT label alone does not close this gate. A direct existing-library implementation could be investigated separately without incorporating that generator.

Hello exposes optional bounded parameterSets/unavailableTuning fields before any registration is sent. Rust checks profile names, duplicate/colliding capabilities, transport advertisement, parameter family, CPS validity and the same runtime limits used by mihomo configuration. Validated tuples are retained in EventTracker; each can produce the exact runtime WarpTransport. The shared schema and protocol/fixtures/search-v1-tuning.json cover the actual Go-generated hello. Probe jobs retain immutable concrete parameters; this addition does not permit a worker to replace requested values.

Tests first failed for missing generators/host fields/Rust catalog handling. Real AWG devices then emitted DNS, SIP and STUN preambles to owned loopback UDP listeners; DNS decoding checked the question and EDNS padding, while SIP/STUN headers and declared sizes were checked independently. This proves actual CPS expansion through the imported device, not external WARP access. Full Go verification passed 41 top-level tests, go vet and go mod verify. A real worker hello/shutdown returned 8,990 bytes with empty stderr and passed the shared JSON Schema. The sanitized generated event is the cross-language fixture; the original is .artifacts/validation/owned-engine-2026-10-28/tuning-hello.json.

Fresh Rust core::warp_ verification passed 82 tests, with two native tests ignored in that run. All 14 warp_engine tests then passed with --include-ignored and one test thread against the newly source-bound worker, including actual Go loopback initiation/cancellation, malformed/exit/panic/hang fixtures, hash/build rejection and confirmed OS exit. Evidence: tuning-go-tests.jsonl, rust-tuning-final.txt and rust-tuning-native-final.txt under the same validation directory. Warnings for pending scheduler/event consumer integration remain; these results do not prove external connectivity, TUN routing or application cutover.

Final worker build: .artifacts/engine-build/source-2026-10-28-tuning-b/tropa-probe.exe; build ID source:0a62b9ab73c1f25cc0f3f71a7e6f5a55169aff3474b237a36d7fc7d81bae4e27; SHA-256 5adcc7a12bf0b9491fdb164f3f63411b004f07b256b4905645600103d3513e7d; size 17,419,264 bytes. Its source.zip/build.json were independently verified before native use. The earlier source-2026-10-28-b worker pin is historical after these engine/protocol changes. No ordinary Tropa release executable or ZIP was rebuilt. Discovery still calls WarpScout; remaining Task 5 work must connect the finite catalog to fair scheduling, preserve streamed provenance, stop the worker before mihomo validation, and then remove production CLI/package inputs.

## Concrete scheduling and search runner checkpoint: 2026-10-28

SearchSchedule now consumes the worker's validated parameter catalog. Each batch carries the exact named WarpTransport and family; duplicate concrete variants are deduplicated, missing allowed transport capabilities fail construction, and variant streams interleave protocols. Every allowed protocol/family receives its baseline initial batch before expansion. The four scheduler tests cover exact parameter retention, all advertised variants, family isolation, restrictions, two-job limits and the validation reserve.

The new Windows runner in src-tauri/src/core/warp_engine/runner.rs starts the pinned worker, validates capabilities before account material, prepares each required identity family once within the original overall discovery budget, and drives at most two jobs. Registration is prepared before admission so enrollment delay does not consume an initial batch's probe time. A failed family is disabled and reported without substituting another identity. Candidate events retain actual parameters, nullable stages/metrics and a network-context timestamp; delivery is bounded and cancellation-aware. Batch deadlines send cancel, allow two seconds for a terminal event, and escalate an unresponsive worker through the existing shutdown path. The outer run_search consumes/shuts down the worker before returning; unresolved cleanup takes precedence over a prior operation error.

Integration exposed a real Go host race: a terminal event could reach Rust before the completed job was removed from the active-slot map. A deterministic test blocked terminal delivery and first observed the completed slot remaining occupied; the host now releases that slot before publishing terminal credit. Full Go verification passed 42 top-level tests, go vet and go mod verify. Evidence: .artifacts/validation/owned-engine-2026-10-28/scheduled-go-tests.jsonl.

All 15 warp_engine tests passed with --include-ignored and one test thread against the newly source-bound worker. The extended native test reused a synthetic stored WG identity, admitted actual scheduled jobs, cancelled after a bounded interval, confirmed OS exit/ownership release, and checked unchanged registration bytes and absence of scanner exchange files. This run may send WG initiation packets with synthetic keys to scheduled public endpoints; it does not establish a successful external tunnel or real-account reuse. The original owned loopback initiation and fault-process tests also remain included. Evidence: rust-scheduled-native-final.txt. Cold registration, successful streamed candidate delivery, output-consumer backpressure and mixed-mode runner acceptance still require further validation; unused-event/run_search warnings reflect the pending production consumer.

Current worker: .artifacts/engine-build/source-2026-10-28-scheduled-a/tropa-probe.exe; build ID source:54d61ff7ffb7c7aaeb2794448386c7c26385f8ca83c7e48007242201ad946c73; SHA-256 3e79dfc254f18b2202912611b204bd24e7b5279b5ffd2a7c1e7d30c981c23851; size 17,419,264 bytes. Source binding was independently verified before native tests. The tuning-b worker is now historical. Production discovery still uses WarpScout: streamed catalog persistence, full measurement provenance, fair final mihomo validation and CLI/package cutover are unfinished. No ordinary application EXE or portable ZIP was rebuilt.

The user's additional concern about game/launcher/anti-cheat IP consistency is recorded in docs/validation/split-tunneling-udp-ipv6.md. It adds connection-owner and per-family path checks before game-specific recommendations; it does not authorize game/anti-cheat launches or establish that any route combination is ban-safe.

## Structured search evidence checkpoint: 2026-10-28

Task 5 now retains typed searchMeasurement metadata in both scanned and runtime-verified catalog observations. It carries the source-bound worker build ID, independent nullable handshake/HTTP/HTTPS/UDP stages, fractional endpoint/tunnel/loss/jitter measurements, sample count/method and the search-time exit country. Runtime verification retains its own exit country and measurement fields. record_probe preserves search evidence when replacing a scanned or previously verified entry; registration invalidation retains it as historical scanned evidence while clearing network freshness. Legacy observations remain readable with no invented search evidence.

The worker-to-catalog adapter reconstructs the exact endpoint/transport/parameters and validates public metadata before producing a ScannedProfile. Runner candidate events now carry this complete typed observation using the client's pinned build ID. Integer fields remain compatibility projections for the existing table; unrounded values remain in searchMeasurement. HTTP-only candidates and unknown UDP results remain unverified. Malformed build provenance, impossible data stages, invalid loss/country, missing sampling method and non-finite measurements are rejected before catalog mutation.

The persistence regression first failed because searchMeasurement was unknown; after implementation all 16 catalog tests passed. The adapter regression first failed for the missing conversion and then passed for both null and fractional measurements, retaining HTTPS=false and UDP=null. The final broader Rust warp_ run passed 114 tests with two native tests ignored. A separate source-bound core::warp_engine run included both native tests and passed all 16 tests. Evidence: .artifacts/validation/owned-engine-2026-10-28/{rust-catalog-final.txt,rust-catalog-native-final.txt}. Existing unused runner-event/run_search warnings remain because production discovery has not yet switched consumers.

The Go/protocol inputs did not change, so source-2026-10-28-scheduled-a remains the worker used for native validation. These tests do not prove a successful external search, mixed-mode candidate delivery, TUN routing or final application integration. Production discovery still uses WarpScout; the next step is the bounded streaming catalog consumer, fair final mihomo validation after worker teardown, and then CLI/package removal. No ordinary application EXE or portable ZIP was rebuilt at this checkpoint.

## Discovery source cutover checkpoint: 2026-10-28

Task 5 now routes run_profile_discovery through the owned runner and bounded stream consumer. The connection caller and diagnostic CLI supply the source-bound runtime directory separately from the account/data root; neither constructs a WarpScout path or scan command. Two producer/consumer futures are awaited together. A disk-consumer error cancels the producer, shutdown is awaited, and cleanupPending takes precedence before runtime ownership can begin. Startup failure closes the stream rather than leaving its receiver waiting. The old warp_scout module, build/package dependency and component diagnostics still exist and require Task 6 removal; this is source-path cutover, not a completed portable distribution.

Prepared events retain standard endpoints with the exact advertised baseline parameters. Streamed candidates are persisted individually within the existing catalog limits, with registration-family binding and geography restrictions applied; unknown required geography remains subject to final runtime validation. Search progress retains per-transport state and explicitly unavailable tuning. A regression reproduced eviction of an earlier HTTPS candidate after 256 later failed probes. Catalog eviction now removes non-HTTPS candidates before successful search observations while preserving manual pins; none of these observations becomes runtime-verified by eviction priority.

The final validation queue interleaves protocols, standard endpoints and measured alternatives. Comparable fractional ICMP metrics retain their method/target, sample count, network context and freshness when ordering alternatives. Manual queue construction returns the exact pinned profile. Refresh comparison uses an automatic view without saving over a manual selection. Automatic cached probes are bounded to 15 seconds each (at most three); the original overall clock still governs discovery and its 45/90-second validation reserve. Every connection continues through fresh mihomo checks; a saved address or positive worker stage cannot bypass them. Single-slice timeouts remain untested rather than permanent endpoint rejection.

The fair-queue and stream-consumer tests first failed for missing implementations; the catalog-pressure test failed by losing the HTTPS profile before its fix. Final full Rust library verification passed 490 tests with two native tests ignored, followed by all 19 core::warp_engine tests with --include-ignored and one test thread against the scheduled-a source-bound worker. Evidence: .artifacts/validation/owned-engine-2026-10-28/{rust-cutover-library-final.txt,rust-cutover-native-final.txt}. Existing legacy scanner dead-code warnings remain. TypeScript checking passed and both existing Tropa frontend test files passed all seven tests after the additive unavailableTuning contract field.

A separate userspace-only external worker check ran two concurrent MASQUE H2 jobs against 162.159.198.2:443 with the stored EC registration and default/Google SNI. Both independently returned handshake=true, HTTP=true and HTTPS=true; each completed five ICMP-v4 samples with zero observed loss. Mean RTT was about 24.66/25.53 ms, node HEL, exit country RU, node country unknown and UDP null. The run completed in about 1.31 seconds, returned exit code zero with empty stderr, confirmed OS exit and unchanged registration bytes. The JSON Schema validated actual received events; exact worker hash/build, targets and parameters were checked. Evidence: .artifacts/validation/owned-engine-2026-10-28/native-h2-worker.{py,json}. No system TUN or mihomo was involved; this proves external transfer in the owned Go adapter, not the complete Rust search pipeline, game latency or UDP routing.

The user clarified that stored IPs must never replace current reachability checks and requested parallel scanning. The catalog is a bounded cache of address/transport/parameter observations, not a downloaded list of guaranteed working servers. The maintained subnet/canonical target generator remains the search source; the worker enforces two jobs and four sessions per job (eight globally). The new source integration, fresh check requirement and parallel limits remain subject to full application/native acceptance. No ordinary application EXE or ZIP was rebuilt. Pending work includes successful mixed-mode Rust pipeline acceptance, cold registration/slow-consumer paths, stricter native teardown verification, removal of the legacy module/package inputs, source-bound app packaging and the remaining full-goal validation gates.
