# WG/AWG search scheduling — 2026-10-28

The previous discovery loop ran standard/quic/stun/sip/dns AWG before WG, all against one deadline. A regression simulated a scanner consuming its allowed duration and failed with `WG was starved: ["standard"]`.

`core/warp_search.rs` now supplies bounded tasks consumed by discovery. Extended search starts with standard AWG and WG, each with a 120-second slice and `-sample 1`. It then expands standard AWG, WG and the four other AWG presets with `-sample 5` and 60-second slices. Quick search uses 30-second initial and 20-second expansion slices. The overall 600/180-second deadline remains authoritative. Tasks run sequentially, retaining existing execution reservations, Windows Job Objects and per-candidate persistence. Policy-disallowed transports are skipped.

Scanner timeout is a typed timeout rather than an empty successful scan. Typed per-scan timeout, no-candidate, invalid-output, process-exit and unreachable errors allow subsequent tasks; registration, ownership and cleanup errors still terminate discovery. Unknown failures remain fatal rather than being treated as ordinary rejected endpoints. Successfully persisted observations from earlier tasks remain available if a later task fails.

Each candidate probe receives child cancellation linked to both the task deadline and user cancellation. Local expiry does not cancel the whole search. Discovery awaits the probe's existing teardown; it does not drop its future to satisfy the timer. Only its typed cancellation result permits proceeding after local expiry; cleanup failure takes precedence. A second regression first failed because the child never expired, then passed after implementing the linked token. It also checks propagation of user cancellation into a subsequent child without local expiry cancelling the parent.

## Verification scope

Final verification: `cargo test --locked -p clash-verge --lib --features clippy,portable,tropa-probe --offline --quiet` passed all 409 tests after the final source changes (exit 0). Only library tests were run for this backend increment. No Tropa EXE, UAC prompt, scanner executable, downloaded upstream code, TUN adapter or network configuration was started or changed. Native scanner timing, cleanup and real endpoint quality remain unverified. The scheduler regression uses simulated elapsed durations; linked cancellation uses short local timers without network access.

## Remaining milestone 4 work

This is a first scheduling increment, not completion of milestone 4. Persisted scanner-discovered but unvalidated rows, scanner adapter extraction, redistribution of unused slices and explicit separation of cleanup duration remain. Per-transport counters/states and UI results were implemented in the next increment below. Aggregate progress remains available for compatibility. The scanner's completed report is read only after successful exit; killed scans are not assumed to contain a complete report. Cancellation/process ownership still requires the deferred native test. MASQUE enrollment/runtime and later milestones are unchanged.


## Second increment: protocol progress

Search progress now includes independent WG/AWG records: state, unique discovered/checked/verified/rejected/untested counts, elapsed milliseconds and a safe typed failure. Candidate identities include the transport parameters; repeated scans or revalidation do not inflate totals. Incomplete, cancelled and cleanup-failed attempts are not counted as network rejections. Previously completed observations survive an unfinished recheck. Aggregate counters are derived from the transport records. Policy-disallowed transports are omitted from the scheduled progress list.

Cached candidates participate in the same accounting. Each task's elapsed time is finalized on exit, including error and cancellation paths. A task that exits through a persistence error cannot report successful completion. A later successful task retains a prior typed scanner failure so the frontend can indicate an incomplete earlier step. State describes the most recent task of that transport and can return to running during expansion. The current elapsed value includes teardown; it is finalized when the task ends, rather than updating continuously while running.

The server panel shows a per-protocol summary during and after search, separate from the selectable server table. It distinguishes queued/not started, running, completed, cancelled, timeout and failure, and shows untested separately from rejected. Cached revalidation is labelled as cached profiles rather than incorrectly labelled AWG. Duplicate verified candidates also replace the current search's previous observation rather than consuming the 128-result limit repeatedly.

Verification after backend changes: 410 Rust library tests passed (exit 0). Frontend verification: 53 tests across 14 files passed, TypeScript and scoped ESLint passed, and Vite production build passed. The new serialization regression first failed because there was no per-transport result; the UI regression first failed because the protocol summary was absent. Both passed after implementation. No Tropa, elevation, scanner executable or tunnel was launched. Browser visual interaction remains unverified.

Scanner-discovered rows are not yet persisted/displayed as selectable candidates: the selectable catalog continues to contain runtime-verified profiles. This remains an explicit next task rather than representing the summary counts as a full persisted candidate table. Milestone 4 remains incomplete.


## Third increment: persisted scanner candidates and manual revalidation

The catalog now stores a separate `scanned` collection with endpoint/transport, scanner timestamp, node/node country, optional scanner RTT/loss and an optional terminal rejection timestamp. It does not construct a fake WarpObservation from scanner data. Existing catalogs without this field still read successfully. Registration changes clear both scanner and runtime evidence. The existing 1 MiB file-size limit remains; verified profiles are capped at 128 and scanner evidence at 256, retaining a manual pin during eviction. A failed oversized/invalid save does not overwrite the prior catalog.

A completed scanner batch is persisted before runtime candidate validation. Successful runtime checks promote a profile into verified entries and remove its scanner-only row. Ordinary transport rejection and policy rejection are persisted; cancellation, deadline interruption and cleanup failure are not saved as a completed network rejection. Verified entries retain a subsequent rejection independently of scanner-row eviction. A successful later validation clears that rejection. Earlier successful evidence is retained as historical evidence, while the current row shows rejected status. The final best-of-checked selection excludes a profile whose last completed check rejected it.

The server command returns both kinds of rows, deduplicated by full profile identity. Scanner-only rows carry null runtime measurements, null runtime timestamp and no invented exit country. The table shows verified, untested and rejected statuses; WarpScout values remain in their own columns. Default quality sorting puts runtime-verified results above scanner-only results; sorting by latency remains an explicit table choice. The details do not divide absent samples or manufacture epoch timestamps.

Manual choice is permitted for an allowed scanner-only or rejected address. It remains visibly unverified/rejected. The stored selection can point at a scanner candidate and survives restart. On Connect, discovery builds the revalidation queue from exactly that manual profile, applies current constraints, and requires a fresh successful probe before TUN activation. It cannot silently substitute another address. Automatic quick mode takes up to three suitable saved profiles, including scanner candidates after verified ones. Current policy rejects known-incompatible metadata; unknown metadata can enter revalidation but cannot satisfy geographic activation checks. None of this claims native TUN behavior has been newly tested.

Regression evidence: the storage test first failed on the unknown `scanned` field; the command-view test first returned zero rows; the sorting test first ranked an unchecked scanner result above a verified connection. The implemented tests cover file roundtrip, manual pending selection, promotion, rejection/recovery without losing the pin, a 70-row command result with null runtime metrics, and the unverified manual-choice UI. The command test uses synthetic addresses; it is not a live WarpScout scan.

Frontend verification after the final changes passed: 55 tests / 14 files, TypeScript, scoped ESLint and Vite production build (exit 0). Final Rust verification after the last source edit passed all 412 library tests (exit 0). No Tropa EXE, UAC, scanner executable, TUN or network change was run. Visual/browser interaction remains unverified.

Remaining milestone 4 work includes extraction of the scanner adapter, redistribution of unused task budgets and separate accounting of scan/probe/cleanup durations. Native process cancellation and transport fairness under actual network timeouts remain deferred. MASQUE and later roadmap milestones are still pending.


## Fourth increment: dynamic task budgets and scanner adapter

`task_budget` now reserves the initial slice of each still-unstarted transport before allocating time to the current task. Initial WG/AWG passes remain capped at 120 seconds in Extended / 30 seconds in Quick. Expansion tasks divide the actual remaining duration among the remaining expansions, rather than using fixed 60/20-second caps. Early completion therefore benefits later tasks, and the last expansion can use the remaining duration. Policy-disabled tasks are removed before allocation so they cannot retain a reservation. The existing absolute global deadline still caps admission/work; process cleanup is awaited even after expiry.

Two duration-controlled regressions first failed against fixed slices: 230 seconds remained stranded after early completion; with 150 seconds left, AWG consumed time that left WG only 30 seconds instead of its 120-second reservation. Both pass with the allocator used by discovery. The simulations test budget arithmetic, not real scanner/network timing. When the remaining global budget cannot support all initial reservations, earlier tasks can receive zero budget and be skipped rather than taking the later reservation.

The existing scanner CLI, report/profile parsers, hash verification, Windows Job Object, owned-child tracking and cleanup code moved into `core/warp_scout.rs`. Existing parser regressions moved with it, including the 70-address report and preservation of generated AWG parameters without credentials. Discovery now consumes the adapter and scheduling policy separately. No downloaded executable was run during this extraction.

Saved-profile revalidation now receives child cancellation linked to the overall discovery deadline, just like live scanned-candidate validation receives a task deadline. A local deadline is reported as validation timeout, while actual user cancellation remains cancellation. Cleanup is awaited before returning either result and cleanup errors are not reclassified as ordinary timeout.

Final verification after all source changes: 414 Rust library tests passed, exit 0, without compiler warnings. No frontend source or wire field changed in this increment, so frontend tests/build were not repeated. No Tropa/UAC/TUN/scanner execution occurred. Separate scan/probe/cleanup durations, complete live timing evidence and deferred native cancellation/routing checks remain open; milestone 4 is not declared complete.


## Fifth increment: separate phase timings

Per-transport progress now publishes a `timings` object with preparationMs, scanMs, validationMs and cleanupMs. A shared monotonic clock records mutually exclusive phases for sequential scanner/probe operations. Serialization includes the currently active interval without committing it twice. The RAII clock commits the last interval on both normal and error exits. A subsequent operation adds to the same transport totals without including the idle gap. The single scanner/core ownership invariant is unchanged; timing does not introduce concurrency or replace lifecycle ownership.

The scanner records preparation/registration/configuration, scan wait, owned-process/file cleanup and result parsing separately. Runtime validation includes core configuration validation, startup and HTTPS/UDP checks. Explicit child stop, tracked-child exit waits and secret-file removal count as cleanup. Best-effort secret-file scope cleanup also switches to cleanup before removing files. Standalone probe entry points use an isolated timing ledger; they do not mutate an unrelated search summary.

The protocol summary shows these durations in seconds while retaining total task elapsed time. It uses the larger of finalized task elapsed and current phase totals so an active scan does not appear to have zero total duration. These are operation durations, not RTT or game quality measurements. Coordinator catalog persistence and synchronization outside the adapter/probe are covered by task elapsed but not necessarily by phase totals, so phase totals need not equal wall-clock elapsed exactly. Millisecond rounding can also differ at phase boundaries. Timing remains diagnostic; it does not silently extend deadlines or interrupt cleanup.

The paused-clock regression first failed with all-zero durations, then passed with disjoint 2-second preparation, 5-second scan, 4-second validation and 3-second cleanup. It checks a live snapshot, error exit, an idle gap and accumulation by a later operation. The UI regression first failed because the split durations were absent, then passed. Final verification after all source edits: 415 Rust library tests, 55 frontend tests / 14 files, TypeScript, scoped ESLint and Vite production build passed (exit 0), without Rust compiler warnings.

No Tropa EXE, downloaded scanner, UAC, TUN, network settings, game or anticheat was launched. Native timing/cleanup and split-routing verification remain open. Other pending work includes comparable/fresh network-context evidence, diagnostics export and the later MASQUE registration/runtime milestones; no complete-client or milestone-4 native success claim is made.


## Registration-family failures within a search

The current four-mode scheduler is described in `masque-runtime-2026-10-28.md`; its 30/90-second initial slices and shared shortage allocation supersede the older two-mode budget constants above.

`RegistrationFailures` is scoped to one `run_profile_discovery` call, covering both cached revalidation and subsequent scanner tasks. A typed registration failure remembers the first safe cause for that family only: WG/AWG share one slot, H3/H2 another. Later profiles/tasks in that family are skipped without preparing registration again. Their progress retains a registration failure with the correct transport label, not an endpoint rejection. The rejected family is removed from pending tasks before budget allocation, so its future tasks cannot hold a reservation. A new explicit search begins with an empty failure ledger; neither identity store nor catalog pin is reset.

Scanner timeout/no-candidate, endpoint transport failure, user cancellation and cleanup errors do not disable a registration family. Cancellation, unconfirmed cleanup and unknown ownership remain terminal in the existing coordinator. A cached automatic registration failure also survives as the final error if no eligible task succeeds, rather than being replaced with generic no-candidates. Manual selection still returns its exact failure and cannot substitute another family/server.

Two regression tests first failed for the missing session ledger, then passed. They exercise actual task filtering and allocation: a failed AWG/WG identity leaves only H3/H2, which split a 120-second remainder as 60/60; failed MASQUE identity leaves WG/AWG and their expansions; scanner/cleanup/cancel causes do not remove the family, and the next search restores eligibility. This reduces repeated ambiguous POST preparation within one search but cannot recover a remote ID whose POST response was never received.

Final verification with the accompanying health increment: 443 Rust tests passed, locked/offline, and production portable-feature check passed without warnings. Frontend checks are recorded in the health report. No Tropa/scanner/core/TUN/UAC was started.

## Superseded failure during a later pass

The scheduler previously set a transport back to Running without clearing its prior per-pass failure. An IPv4 timeout followed by a successful IPv6 or AWG preset pass could therefore leave a Completed summary carrying the earlier failure. A cached retry could retain the same stale diagnostic.

Both cached revalidation and scanner task entry now use `SearchProgress::begin_task`: it changes only that transport's state and current failure. Candidate outcomes, accumulated elapsed/phase timings and the other transports remain intact. This field describes the current/latest pass, not an attempt history; a failed registration family is still skipped via its independent ledger and is not cleared by an unstarted task.

The regression covers a verified candidate, an untested candidate, elapsed time and a separate WG timeout while starting another AWG pass. The initial test failed for the missing transition; after implementation all 458 Rust library tests passed, locked/offline. A production portable-feature cargo check also passed. Native scanner/runtime execution was not performed. The preceding portable ZIP does not contain this later source change.
