# Tropa Search Cutover Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Continue inline; no subagent delegation is authorized for this plan.

**Goal:** Complete the owned-search behavior and remove the executable WarpScout dependency from a source-bound local portable package, then establish native acceptance of the complete search-to-mihomo path.

**Architecture:** Retain the approved Rust coordinator, owned Go protocol worker and separately owned mihomo runtime. Repair the remaining scheduling/persistence/packaging gaps without replacing registration, connection ownership or the interface. Discovery must finish and release its worker before final runtime validation.

**Tech Stack:** Rust/Tauri, React/TypeScript, Go 1.26.3, pinned amneziawg-go/usque/connect-ip-go/quic-go, pinned mihomo, existing Python build/package tools and PowerShell native fixtures.

**Spec:** `docs/superpowers/specs/2026-10-28-native-search-engine-design.md`; full-goal requirements and current status: `docs/tropa-status-and-roadmap-2026-10-28.md`.

## Global Constraints

- Windows x64 portable; no installer or permanently installed service; additional EXE/DLL files are permitted.
- Maximum two concurrent probe jobs and eight live tunnel instances globally; within each job at most four tunnel instances are active.
- Keep 180-second quick and 600-second extended overall budgets, including validation and cleanup. Reserve 45 seconds quick / 90 seconds extended for runtime validation.
- Do not scan through an active application tunnel or modify its configuration.
- Rust owns registration and persistence. No identity in argv, environment, reports, candidate frames or scanner exchange files.
- A manual profile is exact; never silently substitute endpoint, protocol, SNI or AWG parameters.
- A saved/search-successful profile still requires fresh mihomo validation; worker stages do not authorize connection.
- Keep nullable handshake/HTTP/HTTPS/UDP separate. UDP DNS is not proof of arbitrary application UDP.
- Keep node country separate from exit country; unknown required geography cannot authorize a connection.
- Preserve existing user changes and historical artifacts. Use narrow diffs and justified boundary/regression tests under repository AGENTS.md.
- No game/anti-cheat launches; no official WARP/router changes; no third-party application execution.
- No public-release claim while corresponding-source, license, signature or clean-Windows gates remain open.

---

## Current baseline and file responsibilities

The core discovery path now calls the owned runner. Saved results under `.artifacts/validation/owned-engine-2026-10-28/` show 490 passing library tests, two ignored native tests in that library run, then all 19 engine tests including native tests. The external H2 worker result is successful, but it did not run the Rust coordinator, mihomo or TUN. Re-read those evidence files; do not infer a full application pass.

| Files | Responsibility |
| --- | --- |
| `core/warp_engine/{scheduler,runner}.rs` | Immutable jobs, saved/canonical ordering, operation mode, deadlines and worker events |
| `core/warp_engine/{stream,selection,catalog}.rs` | Bounded event consumption, persisted provenance, fair final validation queue |
| `core/warp_discovery.rs` | Cached-first connection and the worker/runtime phase barrier |
| `config/warp_profiles.rs`, `cmd/tropa.rs`, `src/types/tropa.ts` | Catalog ownership and truthful public measurement fields |
| `core/warp_probe.rs` | Final pinned-mihomo HTTPS/UDP-DNS authority and its measurement semantics |
| `core/warp_scout.rs`, legacy portions of `warp_search.rs` and account helpers | Executable CLI/report/export dependency to remove after consumers are replaced |
| `scripts/{tropa_engine,tropa_build,tropa_bundle}.py` and their entry points | Source binding, compilation, package allowlist and independent verification |
| `scripts/prepare-tropa.mjs`, component inventory, `core/warp_diagnostics.rs` | Runtime staging and public component reporting |

Paths starting `core/` and `config/` in this table are under `src-tauri/src/`. Do not create a second ownership/persistence layer.

## Task 1: Finish saved-profile ordering and operation modes

**Files:** Modify `src-tauri/src/core/warp_engine/{scheduler,runner,stream}.rs`, `src-tauri/src/core/warp_discovery.rs`. Extend the existing scheduler tests in place.

**Interfaces:** Retain `SearchSchedule::admit(now) -> Option<ScheduledBatch>`, `finish(token, now) -> bool` and `baseline_profiles() -> Result<Vec<WarpProfile>>`. Add `SearchSchedule::prepend_saved(saved: &[WarpProfile]) -> Result<()>`, called only before admission. Thread the existing `advanced: bool` from discovery into runner/scheduler, rather than accepting it only at the command boundary.

- [ ] Add the following regression inside `scheduler.rs::tests`, using its existing `parameters()` fixture. Watch the missing method fail before implementing it:

```rust
#[test]
fn saved_h2_parameters_are_checked_without_losing_standard_targets() -> anyhow::Result<()> {
    use crate::config::warp_profiles::{WarpProfile, WarpTransport};
    let start = Instant::now();
    let policy = SelectionPolicy { allowed_transports: vec![TransportKind::MasqueH2], ..Default::default() };
    let saved = WarpProfile::new("162.159.199.155:443".parse()?, WarpTransport::MasqueH2 { sni: "www.google.com".into() })?;
    let mut plan = SearchSchedule::new(&policy, false, false, start, 38, &parameters())?;
    plan.prepend_saved(&[saved.clone()])?;
    let first = plan.admit(start).unwrap();
    assert_eq!(first.transport, saved.transport);
    assert_eq!(first.targets[0], saved.endpoint);
    plan.finish(first.token, start + Duration::from_millis(1));
    let baseline = plan.admit(start + Duration::from_millis(1)).unwrap();
    assert!(baseline.targets.contains(&"162.159.198.2:443".parse()?));
    assert!(baseline.targets.contains(&"162.159.198.1:443".parse()?));
    Ok(())
}
```

- [ ] Run `cargo test --locked -p clash-verge --lib --features clippy,portable,tropa-probe --offline saved_h2_parameters --quiet`; confirm the failure is the missing scheduling behavior.
- [ ] Implement initial saved streams with the exact validated transport tuple and family. Derive allowed saved profiles from `WarpCatalog::profiles_to_validate`; exclude profiles with known conflicting geography. Do not put a saved Google SNI into a default-SNI batch. Retain canonical baseline streams and the initial cross-protocol barrier. Bound saved work within the existing total budget.
- [ ] Carry `advanced` through the existing call chain. Normal work retains baseline WG/AWG and fixed I1 alternatives; explicit tuning adds the advertised junk/SNI variants. Add scheduler cases where advanced=false cannot admit a wide-junk or alternate-SNI stream and advanced=true can, without changing a manual pin. Use actual names from `protocol/fixtures/search-v1-tuning.json`, not invented capabilities.
- [ ] Define a basic-connection early handoff only after every enabled initial transport/family has completed or been explicitly disabled by a typed registration failure, and a candidate has HTTPS=true. Refresh/tuning continues its bounded comparison. Test the early-handoff condition in the scheduler/runner consumer, including a late H2 initial batch; do not infer it from a raw count of discovered endpoints.
- [ ] Rerun the targeted scheduler/runner regressions. Record actual time-to-first-result and total duration in Task 5 before describing the change as faster.

**Acceptance:** saved exact parameters and canonical addresses precede expansion, operation modes genuinely differ, all allowed initial transports receive work, and no positive cache entry skips fresh runtime validation.

## Task 2: Preserve measurement meaning across catalog and interface

**Files:** Modify `src-tauri/src/config/warp_profiles.rs`, `src-tauri/src/core/{warp_probe,warp_discovery}.rs`, `src-tauri/src/core/warp_engine/selection.rs`, `src-tauri/src/cmd/tropa.rs`, `src/types/tropa.ts`, `src/utils/tropa-server.ts`, and the existing Tropa frontend tests.

**Interfaces:** Retain `record_scanned(profile, observation)`, `record_probe(profile, observation)` and `server_view(catalog, policy, context)`. SearchMeasurement already owns exact worker values, method, stages and build ID. Runtime values must identify UDP-DNS timing separately; do not relabel them as ICMP.

- [ ] Extend `worker_candidate_keeps_exact_parameters_and_unverified_stages` and the existing `server_view` test with search exitCountry=DE, nodeCountry=FI, HTTPS=false and UDP=null. Assert the row remains `untested`, not `verified`, and the two country fields differ. Observe the currently missing projection before changing the DTO.
- [ ] Add explicit measurement-method/target fields to the public view only where the UI consumes them. Preserve absent fields for old catalogs. Label existing runtime median/jitter/sample counts as UDP DNS, or show the separate ICMP search fields; do not silently compare the two.
- [ ] Extend selection tests with a stale low-latency observation and a fresh comparable observation, and with a different measurement method. The new sorter must not award an advantage by comparing incompatible RTT/loss samples. Test actual queue order, not source strings or constants.
- [ ] Update field labels and frontend sorting in the existing server panel; keep unknown values visible as unknown. Do not redesign the main window or add game-specific controls.
- [ ] Run the Rust profile/selection/command tests, `pnpm.cmd exec vitest run src/services/tropa.test.ts src/utils/tropa-server.test.ts`, and `pnpm.cmd typecheck`.

**Acceptance:** search history survives verification without claiming arbitrary UDP or game latency; geography and validation status remain truthful in the actual table.

## Task 3: Make the worker mandatory in build/package contracts

**Files:** Modify `scripts/{build-tropa.py,package-tropa.py,tropa_bundle.py,prepare-tropa.mjs,tropa-components.json}`, `scripts/test_tropa_{bundle,build,engine}.py`, `src-tauri/src/core/warp_diagnostics.rs`, relevant notices and documentation. Retain `tropa_engine.verify_engine_build(root, output, go)` and `runtime_pin(summary)` as the worker authority.

**Interfaces:** `build-tropa.py --engine-build PATH` becomes required for ordinary portable builds. Add `package-tropa.py --engine-build PATH`; verify that worker evidence with `verify_engine_build` and require equality with the app build record's engine pin before copying bytes. Extend `verify_archive(path, *, pins=None, engine_build=None)` so the independent verifier accepts trusted engine build evidence, not an unchecked manifest claim. Update the CLI `verify-tropa-package.py` to pass that evidence explicitly.

- [ ] Add a regression to existing `BundleVerificationTests` before changing the allowlist:

```python
def test_rejects_legacy_scanner_even_with_consistent_manifest(self):
    self.contents['resources/warpscout.exe'] = b'legacy scanner'
    self.pins['resources/warpscout.exe'] = hashlib.sha256(b'legacy scanner').hexdigest()
    self.write_archive()
    with self.assertRaisesRegex(ValueError, 'contents'):
        verify_archive(self.path, pins=self.pins)
```

- [ ] Run `python -m unittest discover -s scripts -p test_tropa_bundle.py`; observe that a currently accepted legacy package fails the new requirement.
- [ ] Replace the executable inventory/staging entry with the source-bound owned worker. Keep MIT attribution for adapted WarpScout code, but do not ship its executable. Require worker binary, build record/source binding and applicable Go notices in the actual package allowlist.
- [ ] Extend the existing replaced-core test pattern to a substituted worker with a recomputed archive manifest. Verify against trusted worker evidence. Also run the actual packager with an app pin from one worker build and bytes from another; require rejection before an apparently valid ZIP is published.
- [ ] Remove the optional no-worker branch in ordinary app builds. Add a build-tool regression for omitted `--engine-build` and another for changed worker evidence during compilation, using the existing fake compile fixture rather than a real Cargo build per unit test.
- [ ] Update diagnostics to report the actual owned worker build/source ID, integrity and availability. Remove the hardcoded WarpScout component report and outdated README measurement claims.
- [ ] Run the existing Python test families: `python -m unittest discover -s scripts -p test_tropa_*.py`. Inspect each failure; do not update expected hashes merely to make tests green.

**Acceptance:** ordinary builds cannot accidentally omit the search engine; packaging/verifier reject missing/substituted/unbound workers and any production warpscout.exe.

## Task 4: Remove unused CLI code and secret-export paths

**Files:** Delete `src-tauri/src/core/warp_scout.rs` after all remaining legitimate uses are migrated; update its registration in `core/mod.rs`, obsolete task/preset code in `warp_search.rs`, account export helpers in `config/{warp_account,warp_masque_account}.rs`, and related tests/docs.

**Interfaces:** Existing account stores, `RuntimeAccount::{ensure,load,identity,bind_catalog}` and `profile_worker_request` remain the only registration-to-runtime boundary. Do not remove a legacy account parsing helper still required to read saved user data merely because its name mentions WarpScout.

- [ ] Inventory every caller with `rg -n 'warp_scout|ScoutPreset|scanner_args|scout_account|warpscout.exe' src-tauri/src scripts`; classify production execution, persisted-data compatibility, attribution and research separately.
- [ ] Replace remaining production preset types with existing owned `TransportKind`, `WarpTransport` and parameter tuples; remove CLI-only task-budget/preset parsing tests together with the discarded behavior. Keep equivalent behavioral coverage at the scheduler boundary.
- [ ] Remove scanner account/report/config writers and their unused call sites. Retain the secret runtime config needed by mihomo and verify its established teardown; it is not a scanner exchange file.
- [ ] Run `cargo test --locked -p clash-verge --lib --features clippy,portable,tropa-probe --offline --quiet` and the package test families. Also run `cargo check --locked --offline -p clash-verge --features portable` to cover the ordinary feature combination.
- [ ] Repeat the `rg` inventory and review all surviving mentions. A license notice/reference is permitted; executable staging, process launch, CLI arguments and report scraping are not.

**Acceptance:** no legacy scanner execution path or scanner secret-export path remains in production; account compatibility and attribution remain intact.

## Task 5: Establish successful native pipeline and failure acceptance

**Files:** Extend existing native worker tests in `core/warp_engine/process.rs` only where necessary, reuse `stream.rs` regressions and `src-tauri/src/bin/tropa-warp-probe.rs`. Write factual results to `docs/validation/production-connection.md` with a uniquely named `.artifacts/native-connection/` run directory.

**Interfaces:** `stream::search(...) -> Result<CollectedSearch>` must await producer shutdown even on consumer failure. The diagnostic CLI accepts `<absolute-data-directory> <pinned-mihomo-executable> <source-bound-runtime-directory> [--refresh]`; it does not take a WarpScout path.

- [ ] Re-run the saved external userspace H2 check only if its relevant Go/protocol inputs changed; otherwise retain its exact build evidence. It already proves default/Google SNI HTTP/HTTPS on `.198.2:443`, not the complete pipeline.
- [ ] Create an isolated native data directory using existing authorized registrations without printing/copying them into reports or archives. Preserve before/after registration-byte equality. Restrict `selectionPolicy.allowed_transports` to `masqueH2` in its `tropa.yaml`; never change the user's ordinary preferences to simplify a test.
- [ ] Build and run the existing diagnostic binary against the exact source-bound worker and pinned mihomo. Run refresh in this isolated root, then an ordinary cached connection. Require candidate stages/parameters, streamed catalog persistence, OS worker exit before mihomo reservation, and fresh runtime validation. If UDP DNS fails while HTTPS passes, preserve that distinction rather than declaring the entire transport unusable.
- [ ] Repeat with allowed WG/AWG/H3/H2 together; check initial work, separate outcomes, bounded parallelism and exact parameter reconstruction. Record unsupported/unreachable modes without making a universal network claim.
- [ ] Force bounded slow consumption and a catalog write error using owned fixtures. Cancel during registration, during two jobs and before final validation. Require that no new mihomo starts while owned worker cleanup is unresolved and that the original persistence error is not masked by cooperative cancellation.
- [ ] Exercise exact manual failure without substitution, geography rejection and a changed network context. Verify the saved manual selection bytes remain intact after refresh/cancellation.
- [ ] Compare process lists, runtime/scanner-file inventory and registration bytes before/after. Never stop unrelated processes. Keep external UDP/IPv6/browser/game claims outside this task.

**Acceptance:** successful current Rust → worker → catalog → worker OS exit → mihomo path, plus explicit evidence for the failure/cleanup boundaries. A worker-only Python result cannot substitute for this task.

## Task 6: Produce the current local portable package and hand over remaining gates

**Files:** Existing build/package tools; update `docs/validation/portable-release.md`, the main status/roadmap and component notices from their actual outputs.

- [ ] Build frontend with the pinned project toolchain and record its inputs/output. Verify the worker build before the app build; choose new output names because the tools reject overwriting historical artifacts.

```powershell
$python = 'C:/Users/Vanya/AppData/Local/Programs/Python/Python314/python.exe'
pnpm.cmd run web:build
& $python -X utf8 scripts/build-tropa-engine.py verify --name source-2026-10-28-scheduled-a
& $python -X utf8 scripts/build-tropa.py --name Tropa-owned-search-2026-10-28-c --engine-build .artifacts/engine-build/source-2026-10-28-scheduled-a
```

If Go/protocol/build inputs changed, first create and verify a new worker with `build-tropa-engine.py build --name source-2026-10-28-cutover-c`, then use that exact directory consistently. Do not use the scheduled-a record after such a change.

- [ ] Package the ordinary EXE with the source/app/worker records. The following `--engine-build` flags are the new interfaces defined in Task 3; they are not supported by the pre-task packager:

```powershell
& $python -X utf8 scripts/package-tropa.py --binary target/release/clash-verge.exe --name Tropa-owned-search-2026-10-28-c --build-record .artifacts/validation/Tropa-owned-search-2026-10-28-c/build.json --source-snapshot .artifacts/validation/Tropa-owned-search-2026-10-28-c/source.zip --engine-build .artifacts/engine-build/source-2026-10-28-scheduled-a
& $python -X utf8 scripts/verify-tropa-package.py .artifacts/portable/Tropa-owned-search-2026-10-28-c.zip --engine-build .artifacts/engine-build/source-2026-10-28-scheduled-a
```

- [ ] Independently inspect ZIP contents and PE bytes: GUI, x64, requireAdministrator, ordinary release features, no diagnostic executables/flags, no private data, no warpscout.exe. Verify the copied worker bytes and source pin match those compiled into the app; inspect signatures separately.
- [ ] Run the actual unpacked package on the current Windows with own authorized smoke workflow, including cancellation, tray hide/exit and initial connection. Do not treat an older source-bound archive as this artifact.
- [ ] Update the complete requirement matrix in `docs/tropa-status-and-roadmap-2026-10-28.md`: this milestone does not close arbitrary UDP, external/inner IPv6, browsers, 20 cycles/60 minutes, clean Windows/WebView2, licensing/signing, game measurements or final cleanup. Keep `publicReleaseReady=false` until the applicable full-goal gates are closed.

**Acceptance:** current source-bound local portable package with no executable WarpScout dependency and a native result for that exact artifact. Further full-goal milestones remain explicit.

## Verification environment and execution handoff

Use the existing local toolchains; do not install or upgrade dependencies as a side effect. Set `CARGO_HOME=.tools/cargo`, `RUSTUP_HOME=.tools/rustup`, prepend the Cargo bin directory and keep `CARGO_BUILD_JOBS=4`. For source-bound native tests set `TROPA_ENGINE_PIN` from `runtime_pin` fields in the chosen verified build record and `TROPA_ENGINE_FIXTURE_ROOT` to its `runtime` staging directory; pin data contains no credentials. Go uses `GOTOOLCHAIN=local`, `.tools/go-cache` and `.tools/go-path`.

Execute inline under the previously approved workflow. Before a PR, follow repository issue/accountability/disclosure rules; this plan does not invent an issue number, authorize publication or justify unrelated cleanup. Report verified outcomes and remaining gates after each task. Do not reset the full active goal to this smaller milestone.
