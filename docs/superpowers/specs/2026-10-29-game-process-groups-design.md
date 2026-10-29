# Game process groups and anti-cheat routing

Status: superseded; do not implement. The user rejected maintaining a game EXE catalog and rejected whole-computer routing as the answer to automatic per-application routing. Retain this document as research history only. Current feasibility evidence is in `.artifacts/research/application-session-feasibility-2026-10-29/findings.md`.

Original proposal: not implementation approval. Research refreshed on 2026-10-29. No game, anti-cheat, network driver or VPN session was launched for this research.

## Problem and goal

Selecting an executable currently creates a path rule, not a group containing its launcher, helpers and anti-cheat. Users cannot tell whether all relevant game components are covered. Add explicit, explainable game process groups to application split tunneling. Keep ordinary connection discovery and game-server optimization independent.

The implementation requires an approved issue under root AGENTS.md rule 1. This document provides the concrete scope to approve. UI-01 approval does not authorize this subsystem.

## Evidence and limits

Fresh source responses and extracts are in `.artifacts/research/game-process-routing-2026-10-29/`; earlier competitor materials are in `.artifacts/research/game-process-routing-2026-10-28/`.

- [EAC FAQ](https://www.easy.ac/support/articles): its Windows service is shared across games. Public FAQPage structured data explicitly documents this. Service lifecycle is different from a game's child-process tree.
- [BattlEye FAQ](https://www.battleye.com/support/faq/): the launcher and service require working Internet access; service files can be outside the game directory. It distinguishes kicks or blocked files from bans. It does not prescribe a universal same-public-IP rule for game and anti-cheat.
- [mihomo rules](https://wiki.metacubex.one/en/config/rules/): full process paths and path regular expressions are supported. A path rule identifies the executable, not which game a shared executable is serving.
- [Windows service enumeration](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-enumservicesstatusexw) distinguishes own-process and shared-process services. [Service configuration](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-queryserviceconfigw) exposes configured executable command lines. [Process image query](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew) accepts limited query access.
- [WFP ALE](https://learn.microsoft.com/en-us/windows/win32/fwp/application-layer-enforcement--ale-) supports application identity at Windows filtering layers. This is a possible future enforcement architecture, not proof that a custom driver is needed now or automatically compatible with anti-cheats.
- [GearUP platform settings](https://www.gearupbooster.com/support/platform-boost-setting.html) and [path selection](https://www.gearupbooster.com/support/how-to-change-the-game-path.html), saved on 2026-10-28, document supplementary platform acceleration and editable launch paths. Refresh requests timed out on 2026-10-29. Its internal process catalog is not public in these materials.
- [LagoFast AIMR](https://www.lagofast.com/en/support/285/) describes destination-aware routing, not an executable or anti-cheat matching schema. Marketing assertions do not establish our compatibility.
- ExitLag knowledgebase and FAQ returned HTTP 403 again. Web searches did not expose a reliable implementation description. No claims are made about its private database or anti-cheat routing policy.

No reviewed source establishes that different game/service IPs alone cause an anti-cheat ban. No source certifies Tropa. A shared outbound does not guarantee identical IPv4/IPv6 or destination-specific exit addresses. An anti-cheat service being present does not prove that it generates its own external traffic.

## Existing implementation

- `src-tauri/src/config/tropa.rs`: `RoutingPreferences` stores `mode` and up to 64 application paths. `normalized`, `validate_connection` and `rules` validate paths and emit exact or directory `PROCESS-PATH-REGEX` rules followed by `MATCH,DIRECT` in selected mode.
- `src-tauri/src/core/warp_runtime.rs`: installs these rules and `find-process-mode: always`; selected and whole-computer DNS policies differ.
- `src-tauri/src/core/warp_connection.rs`: reads and validates routing before starting the owned core, then retains the active routing snapshot.
- `src-tauri/src/cmd/tropa.rs`: `set_tropa_routing` rejects changes while a connection or cleanup owns the operation.
- `src/components/tropa/routing-picker.tsx`: explicit EXE/folder controls; external services must be added separately.

Real Firefox/Chrome separation and an hour of paired IPv4 HTTPS requests have passed in earlier builds. They do not validate game groups, arbitrary game UDP, anti-cheat services or a future executable. IPv6 production correction remains deferred at the user's request.

## Approaches considered

1. **Recommended: explicit profiles compiled into existing path rules.** Reuses mihomo and current lifecycle. Known paths are installed before a game starts. Shared services remain an explicit limitation. Low impact on portable packaging.
2. **Automatic inheritance from every child process.** Appears convenient but misses services started by Windows, existing processes and shared launchers. Updating rules after a start notification races the first connection. Use process observations as evidence, not automatic inheritance.
3. **New WFP redirect/filter implementation.** Potentially stronger application attribution, but introduces a substantial network subsystem, driver/signing/compatibility work and does not itself determine correct game membership. Defer unless measured attribution failures require it.

## Routing policy

Do not globally exclude anti-cheats. Do not automatically include every related EXE. Each game profile explicitly lists game networking executables, required helpers, optional launcher components and individually identified user-mode services.

An approved and tested profile may default a confirmed network-using service to WARP. An experimental profile cannot silently add a shared anti-cheat service. The review screen names the exact service/path and explains that the entire executable's traffic is affected while the group is active. User acceptance is saved for that profile revision and component; new or changed shared components require review again.

No automatic rules for `System`, `svchost.exe`, every Steam/Battle.net process or unknown descendants. Shared-host services and kernel-originated traffic with no separately attributable executable are unsupported in group mode and shown as unresolved. Multiple games using the same service cannot be separated by that service's path; do not claim otherwise. Whole-computer mode is an explicit alternative, never an automatic fallback.

## Architecture and flow

### Bundled catalog

Start with a small versioned JSON catalog included in the application's source-bound build. Separate profile revision from schema version. Each entry has an ID, title, store ID, installation evidence, component IDs/roles, exact relative paths or explicit Windows service names, required/optional flags, shared-service flags and provenance. Initial DayZ/BattlEye entry is experimental until local discovery and real-session validation establish its contents. Do not guess filenames or mark it supported from the Steam listing alone.

No online catalog updater, remote scripts, arbitrary manifest regexes or general executable command execution in version 1. Profile maintenance ships with reviewed releases. Local installation roots and component overrides survive profile updates; shared-component consent is revision-specific.

### Installation and service resolver

Use a chosen installation directory and, when available, Steam `libraryfolders.vdf` plus the relevant `appmanifest_*.acf` install location. Support multiple libraries and manual correction. Parse these as data without running Steam. Resolve profile-relative paths within the canonical installation root; reject traversal and missing required files. A file existing under a plausible name is installation evidence, not proof it owns game traffic.

Query only named services from the profile with read-only SCM rights. Obtain configured path and running process metadata separately. An unquoted ambiguous service command line is unresolved; do not split it on the first space or run it. A running process image can verify its path, but disagreement with service configuration is unresolved. Kernel driver services and shared-host service processes cannot become executable rules automatically.

### Saved choices versus effective routing

Keep manual `RoutingPreferences.applications` unchanged. Store group IDs, catalog revisions, roots, explicit component overrides and shared-component choices in a separate optional `TropaPreferences.routingGroups` field. Old preferences deserialize with an empty group list.

When disconnected, preview the resolved components. At connect time, under existing operation ownership, resolve again, verify catalog revisions/consent and compile manual paths plus approved group paths into an effective `RoutingPreferences`. Deduplicate case-insensitively and enforce the existing total limit of 64 paths. Missing required components or stale shared-service consent stop startup with a specific actionable error. Optional unresolved components remain visible and never produce a claim of complete coverage.

Feed only the effective policy to existing `validate_connection`, rule generation and runtime configuration. Publish effective paths plus group IDs/revisions as the active snapshot. Do not persist expanded group paths back into the manual list. In all-computer mode preserve saved groups but do not resolve them or let a missing game prevent connection.

Changes require disconnect/reconnect. No live route changes when a new process appears, no automatic WARP endpoint switching during a match, and no forced game/service termination.

### Observation and coverage

Version 1 uses user-requested read-only snapshots: PID, creation time, full image path, parent PID and named-service metadata. Query with limited access; access denied means unknown. Do not read game memory, inspect protected modules, inject DLLs, hook game code or change anti-cheat configuration. Do not attempt to bypass protected-process restrictions.

Treat these separately:

| State | Meaning |
| --- | --- |
| Resolved | A component path was located. |
| Configured | The active runtime snapshot contains its rule. |
| Observed | Its process/service was seen. |
| Traffic verified | A dated test observed its relevant flow and expected route. |
| Unresolved | Identity, path or attribution could not be established. |

Process presence does not prove network ownership. For permitted validation, correlate available mihomo connection metadata with an actual network request and independent destination evidence. If metadata is missing or a process emits no observable network request, record that limitation. Do not infer success from a generic tunnel probe.

Continuous start/stop monitoring is a separate follow-up only if snapshot validation shows it is needed. It must reconcile a full initial snapshot, identify PID reuse by creation time, own cancellation and never silently add routes.

## Interface

Within Split tunneling add a game-group list beside the existing manual controls. Show profile name, experimental/tested scope, component count and expandable exact paths/roles. Shared-service inclusion is explicit and explains its scope. Show missing required components before connection. Active state comes from the frozen runtime snapshot, not newly edited catalog data.

Keep ordinary connection independent of game selection. Avoid generic labels such as 'anti-cheat safe', 'protected' or 'everything included'. Describe verified test scope and date. A Windows service being included does not mean Tropa controls or installs that service.

## Validation and release boundary

Use focused tests for path boundaries, migration, shared-service consent, deduplication/limits, missing components and active snapshot isolation. Reuse existing TCP/UDP routing fixture where it provides real evidence; do not create a fake anti-cheat or synthetic service to claim real compatibility.

Pilot DayZ only in an explicitly authorized user-operated session. Compare direct, whole-computer WARP and grouped routing using the same endpoint where possible. Cover launch/login, BattlEye update/initialization, server join, gameplay, reconnect and clean exit. Keep unrelated browser traffic direct in selected mode. Observe actual TCP and UDP ownership separately. A permitted game server or controlled responder is required for independent UDP claims; unavailable tests remain open.

Do not disable anti-cheats, bypass blocks, or use valuable accounts as an unattended test mechanism. Any kick, policy restriction or incompatible software warning is recorded distinctly and stops automatic test progression. Successful sessions establish a bounded compatibility observation, not a guarantee against future sanctions.

Public readiness requires tested profile contents, portable preference preservation, network cleanup, no account data in the ZIP and source-bound package verification. Keep DayZ experimental if any required identity or traffic evidence remains unknown.

## Scope exclusions

No production IPv6 changes, custom driver, scanner changes, automatic route inheritance, anti-cheat removal, game-server optimization, remote catalog updater, universal game detection or no-ban guarantees.
