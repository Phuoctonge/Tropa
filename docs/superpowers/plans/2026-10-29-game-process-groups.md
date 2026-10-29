# Game Process Groups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Execute inline; do not dispatch subagents without explicit authorization.

**Goal:** Add explicit, inspectable game component groups to Tropa split tunneling, with honest handling of shared anti-cheat services.

**Architecture:** Resolve a bundled profile and local installation into exact executable paths, then compile these with manual selections through the existing mihomo routing pipeline. Freeze the effective group policy at connection startup. Read-only process/service observations explain coverage; they never add routes automatically.

**Tech Stack:** Existing Rust/Tauri, serde, Windows bindings, mihomo, React/TypeScript and Vitest. No new network driver or runtime dependency by default.

**Spec:** `docs/superpowers/specs/2026-10-29-game-process-groups-design.md`

**Status:** Superseded; do not execute. The user rejected the game EXE catalog and whole-computer fallback as the solution. Retained for research history only. Read `.artifacts/research/application-session-feasibility-2026-10-29/findings.md` for the replacement feasibility investigation. Root AGENTS.md requires an approved issue before nontrivial source changes; UI-01 is unrelated.

## Global Constraints

- No production IPv6 changes, custom driver, scanner changes, automatic route inheritance, anti-cheat removal, game-server optimization, remote catalog updater, universal game detection or no-ban guarantees.
- Changes require disconnect/reconnect. No live route changes when a new process appears, no automatic WARP endpoint switching during a match, and no forced game/service termination.
- Keep manual `RoutingPreferences.applications` unchanged.
- Deduplicate case-insensitively and enforce the existing total limit of 64 paths.
- Process presence does not prove network ownership.
- Whole-computer mode is an explicit alternative, never an automatic fallback.
- Existing root, src and src-tauri AGENTS.md ownership rules apply. Tests below guard concrete policy expansion errors; do not add unrelated test scaffolding or refactor unrelated files.
- All source, comments and commits are English. Product strings follow existing Russian UI conventions. Make a conventional scoped commit only after reviewing the task diff; preserve unrelated dirty files.

## Delivery sequence and file ownership

1. Catalog and saved group data: new `src-tauri/src/config/game_groups.rs`, new `src-tauri/resources/game-groups.json`; registration in `src-tauri/src/config/mod.rs`, optional field in `src-tauri/src/config/tropa.rs`.
2. Local identity resolution: new `src-tauri/src/core/game_groups/mod.rs`, `resolve.rs`, `windows.rs`; registration in `src-tauri/src/core/mod.rs`. Extend existing Windows dependency features only as required for read-only process/SCM APIs.
3. Compilation and connection snapshot: new `src-tauri/src/core/game_groups/policy.rs`; existing `warp_connection.rs`, `cmd/tropa.rs`, `lib.rs`; use existing `warp_runtime.rs` rules without changing transport/DNS behaviour.
4. UI: `src/types/tropa.ts`, `src/services/tropa.ts`, owning page/hook/shared intent boundary, `routing-picker.tsx`, new `game-group-picker.tsx`, scoped styles and focused existing tests.
5. Read-only inspection: new `src-tauri/src/core/game_groups/inspect.rs`, existing command/service boundaries and game-group details UI.
6. Pilot and release: `docs/validation/game-process-groups.md`, one user-visible `Changelog.md` entry, existing source-bound portable build/package scripts.

The following interfaces are proposed additions, not existing APIs. Define them before implementing consumers. No source code in this plan has been compiled.

## Task 1: Define catalog and persistent choices

**Files:** Create `src-tauri/src/config/game_groups.rs` and `src-tauri/resources/game-groups.json`; modify `config/mod.rs` and `config/tropa.rs`. Tests belong beside the new pure configuration code.

**Interfaces:**

```rust
pub struct SavedGameGroup {
    pub profile_id: String,
    pub revision: u32,
    pub install_root: PathBuf,
    pub overrides: BTreeMap<String, PathBuf>,
    pub accepted_shared: BTreeSet<String>,
}
pub enum ComponentSource {
    InstallRelative { path: PathBuf },
    NamedService { name: String },
}
pub enum ComponentRole { Game, Helper, Launcher, AntiCheatService }
pub struct ComponentDefinition {
    pub id: String,
    pub role: ComponentRole,
    pub source: ComponentSource,
    pub required: bool,
    pub shared: bool,
}
pub struct GameGroupDefinition {
    pub id: String,
    pub revision: u32,
    pub title: String,
    pub store_app_id: Option<String>,
    pub experimental: bool,
    pub sources: Vec<String>,
    pub components: Vec<ComponentDefinition>,
}
pub fn load_bundled_catalog() -> anyhow::Result<Vec<GameGroupDefinition>>;
```

Import `PathBuf`, `BTreeMap` and `BTreeSet` from std. Derive serde representations with camelCase fields, tagged component sources and stable enum spellings. Add `#[serde(default, rename = "routingGroups", skip_serializing_if = "Vec::is_empty")] pub routing_groups: Vec<SavedGameGroup>` to `TropaPreferences`; retain all old fields. Catalog envelope has `schemaVersion: 1` and `profiles`. Reject any unsupported schema, duplicate profile/component IDs, zero revisions, absolute/traversing install-relative paths and privileged wildcard targets.

- [ ] Add a minimal regression using existing preference YAML without `routingGroups`; assert an empty vector after load and unchanged manual selections after save/load.
- [ ] Add rejection tests for `../outside.exe`, duplicate IDs and unsupported schema. These guard route expansion from data, not hypothetical transport failures.
- [ ] Implement catalog parsing using `include_str!` and the existing serde dependencies. Ship an empty profile list until Task 6 supplies evidence-backed DayZ definitions; do not ship guessed executable names.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --features portable game_groups` and the existing configuration test filter. Expected: catalog rejection and legacy round-trip cases pass.
- [ ] Review only the listed files and record the scoped implementation change after issue approval.

## Task 2: Resolve installations and named services

**Files:** Create `core/game_groups/{mod,resolve,windows}.rs`; modify `core/mod.rs`; extend Windows feature declarations in `src-tauri/Cargo.toml` only for APIs actually called. Do not introduce a general OS management service.

**Consumes:** Catalog definitions and saved group choices from Task 1.

**Produces:**

```rust
pub struct ResolvedComponent {
    pub id: String,
    pub role: ComponentRole,
    pub required: bool,
    pub shared: bool,
    pub path: Option<PathBuf>,
    pub reason: Option<String>,
}
pub struct ResolvedGameGroup {
    pub profile_id: String,
    pub revision: u32,
    pub components: Vec<ResolvedComponent>,
}
pub async fn resolve_group(
    definition: &GameGroupDefinition,
    saved: &SavedGameGroup,
) -> anyhow::Result<ResolvedGameGroup>;
```

- [ ] Reproduce root escape and missing-required-path failures in resolver tests with temporary files; assert a sibling directory never becomes a component.
- [ ] Implement manual-root resolution first. Canonicalize root and candidate; reject a candidate outside the root, including junction escapes. Only explicit saved overrides may refer outside the installation root; show their origin as a user override.
- [ ] Read Steam library/appmanifest locations as data, with manual root correction always available. Reuse an existing parser only if one is already present; otherwise keep a small read-only parser limited to the required VDF string/object grammar, escaped strings and named keys. Test a non-default library, escaped path and malformed document; return a correction prompt on malformed data rather than guessing a directory.
- [ ] Implement read-only named-service lookup with `OpenSCManagerW`, `OpenServiceW`, `QueryServiceConfigW` and `QueryServiceStatusEx`. Query the configured image and, if running, `QueryFullProcessImageNameW` with limited access. Do not start/stop/configure services or request process memory rights.
- [ ] Keep ambiguous unquoted command lines, config/image disagreement, shared-host services, driver services and access-denied queries unresolved. Report the component ID and reason; never synthesize an EXE path by splitting on spaces.
- [ ] Run focused resolver tests and a read-only local lookup of one explicitly named installed service if available. A missing service is a supported result, not permission to install one. Expected: no Windows service/process changes.

## Task 3: Compile effective policy and freeze connection state

**Files:** Create `core/game_groups/policy.rs`; modify `core/warp_connection.rs`, `cmd/tropa.rs`, `lib.rs`; preserve the existing lifecycle lock ordering. Configuration tests remain beside their respective policy functions.

**Consumes:** Manual policy, catalog definitions, saved group choices, resolved groups.

**Produces:**

```rust
pub struct ActiveGameGroup {
    pub profile_id: String,
    pub revision: u32,
    pub component_ids: Vec<String>,
}
pub struct EffectiveGameRouting {
    pub routing: RoutingPreferences,
    pub groups: Vec<ActiveGameGroup>,
}
pub fn compile_game_routing(
    manual: &RoutingPreferences,
    saved: &[SavedGameGroup],
    resolved: &[ResolvedGameGroup],
) -> anyhow::Result<EffectiveGameRouting>;
```

`compile_game_routing` rejects group revision mismatch, absent required paths, required shared components without consent and more than 64 deduplicated total paths. Optional unaccepted shared components produce no path. All mode returns the manual policy and no active groups without resolving missing game files. The coordinator implements that bypass before calling the resolver.

- [ ] Add a regression with a required shared service: compilation must fail until its component ID is accepted for the selected revision. Add an optional shared service case asserting it is omitted without consent.

Use this minimal policy regression inside `policy.rs` (import the Task 1/2 types and `RoutingMode` from their owning modules):

```rust
#[test]
fn required_shared_service_needs_explicit_acceptance() {
    let manual = RoutingPreferences {
        mode: RoutingMode::Selected,
        applications: Vec::new(),
    };
    let mut saved = SavedGameGroup {
        profile_id: "fixture".into(),
        revision: 1,
        install_root: PathBuf::from(r"C:\Fixture"),
        overrides: BTreeMap::new(),
        accepted_shared: BTreeSet::new(),
    };
    let resolved = ResolvedGameGroup {
        profile_id: "fixture".into(),
        revision: 1,
        components: vec![ResolvedComponent {
            id: "service".into(),
            role: ComponentRole::AntiCheatService,
            required: true,
            shared: true,
            path: Some(PathBuf::from(r"C:\Fixture\service.exe")),
            reason: None,
        }],
    };
    assert!(compile_game_routing(
        &manual, std::slice::from_ref(&saved), std::slice::from_ref(&resolved)
    ).is_err());
    saved.accepted_shared.insert("service".into());
    let policy = compile_game_routing(&manual, &[saved], &[resolved]).unwrap();
    assert_eq!(policy.routing.applications, vec![PathBuf::from(r"C:\Fixture\service.exe")]);
    assert!(manual.applications.is_empty());
}
```

This is a pure compiler test, not a fake anti-cheat and not a filesystem or routing test. The test filename is not added to the shipped catalog.

- [ ] Add cases for revision mismatch, case-insensitive duplicate paths, total path overflow, empty manual list with a valid group, and all mode with stale saved groups.
- [ ] Implement compilation by constructing an effective `RoutingPreferences` and reuse `normalized`, `validate_connection` and `rules`. Never modify manual preferences with expanded paths. Preserve `MATCH,DIRECT` and existing DNS policy.
- [ ] Insert resolution/compilation at connection startup before core spawn, under the existing operation owner. Publish the effective routing plus active group revisions after successful startup. Retain authoritative state if later cleanup fails.
- [ ] Add `preview_tropa_game_groups` (read-only resolution result) and `set_tropa_game_groups` (persist saved choices) commands. Mirror `set_tropa_routing` idle/cleanup checks and configuration-write ownership for mutation; frontend paths and claimed coverage are untrusted inputs and must be revalidated by Rust.
- [ ] Verify a saved/catalog edit cannot alter an already active snapshot. Verify a failed group save leaves preferences unchanged. Run targeted backend tests with the project's portable feature configuration.

## Task 4: Add group review to split tunneling

**Files:** Modify `src/types/tropa.ts`, `src/services/tropa.ts`, `src/components/tropa/routing-picker.tsx`, `src/components/tropa/tropa.css` and the current page/hook owning routing intents; create `src/components/tropa/game-group-picker.tsx`. Extend `src/components/tropa/tropa-routing.test.ts` and existing service tests only for new contracts.

**Interfaces:** TypeScript mirrors the Rust camelCase serializations. Add `routingGroups?: SavedGameGroup[]` to preferences. Services own `previewTropaGameGroups()` and `setTropaGameGroups(groups: SavedGameGroup[])`; components never call `invoke` directly. Mutations use the existing shared intent funnel. Backend resolution is authoritative.

- [ ] Add rendering regression for an unresolved required component, a shared-service consent row and connected-state controls disabled until disconnect. Do not assert CSS implementation details.
- [ ] Add a profile chooser and expandable component review: role, exact path, required/optional, shared scope, resolution reason. Retain manual EXE/folder controls and ordinary connect without a game.
- [ ] Use explicit Russian copy: `Общая служба: через WARP пойдёт весь трафик этого EXE, в том числе для других игр.` Show `Путь найден`, `Правило применено`, `Процесс обнаружен`, and `Трафик проверен` only for the matching evidence states. Never label mere presence as successful routing.
- [ ] Display active group revisions from the connection snapshot; saved previews cannot overwrite them. Update shared-service consent only through an explicit user action and the normal persistence command.
- [ ] Run `pnpm exec vitest run src/components/tropa/tropa-routing.test.ts src/services/tropa.test.ts`, `pnpm typecheck`, changed-file ESLint and `pnpm web:build` once source changes are final.
- [ ] Inspect the actual UI in light/dark themes, narrow window and keyboard-only operation; confirm missing components and shared-service scope remain readable. Preserve the approved compact server panel.

## Task 5: Provide read-only component inspection

**Files:** Create `core/game_groups/inspect.rs`; modify the existing backend command registration, TypeScript service/types and group details UI. Reuse the Windows adapter from Task 2.

**Consumes:** The resolved preview or frozen active group snapshot. **Produces:** `GameGroupInspection` with `observedAt`, `components` and per-component process instances (`pid`, `createdAt`, `path`, `parentPid`) plus explicit unresolved reasons. These are observations, not routing decisions.

- [ ] Add a pure identity comparison test: PID reuse with a different creation time cannot retain an old component association.
- [ ] Implement a user-requested process snapshot using documented enumeration, full image path and creation-time queries. Resolve only component-relevant metadata; access denied remains unknown. Do not gather command lines, game memory, account data or unrelated detailed histories.
- [ ] Associate by canonical full path and creation time; parent PID is supplementary. Query named-service state separately. Do not add a child to a group merely because its parent is a launcher.
- [ ] Add an inspection button and timestamp; clearly distinguish `Процесс обнаружен` from `Трафик проверен`. Expire inspection display when the connection snapshot changes. Keep full local paths out of routine exported diagnostics unless the user deliberately exports them.
- [ ] Run focused inspection tests and perform a read-only observation of an ordinary permitted application. Confirm no service changes and no rules changed after inspection.

Continuous monitoring is intentionally outside this deliverable. If later evidence requires it, design its initial snapshot/event reconciliation, PID reuse and cancellation before implementation; do not substitute a polling loop that silently expands routes.

## Task 6: Establish the DayZ/BattlEye pilot and ship bounded claims

**Files:** Populate the catalog only with observed evidence; add `docs/validation/game-process-groups.md`; update one user-visible line in `Changelog.md` when functionality ships. Use the existing `tropa-routing-fixture` and package pipeline instead of building duplicate harnesses.

- [ ] Obtain explicit authorization for the real game session and identify the installed DayZ/Steam location. Use store app ID `221100` as identity evidence; discover and confirm actual executable/service paths rather than assuming filenames from a web listing. If unavailable, finish infrastructure with no tested game claims and keep pilot acceptance open.
- [ ] Collect a user-operated direct baseline. Observe launch/login, BattlEye initialization/update, server join, ordinary gameplay, reconnect and exit. Record exact game/core/catalog versions and which components actually own network requests. Do not launch competing anti-cheats for unrelated games.
- [ ] Compare whole-computer WARP and the explicit group using a fixed verified WARP profile, then disconnect. Avoid endpoint changes during gameplay. Verify an unselected browser remains direct in group mode and returns to baseline after disconnect.
- [ ] Check TCP and game UDP separately, with fresh sockets and persistent flows where observable. Use destination-side evidence or a permitted controlled responder for independent UDP assertions. Without one, describe the precise client-side evidence and leave unverified dimensions open; do not infer full UDP correctness from Firefox HTTP/3.
- [ ] Test missing/moved files, required-service lookup denial, game update changing a component, saved shared consent invalidated by revision changes and two groups referring to the same shared EXE. Do not run two protected games together merely to simulate this case; use pure policy tests for the shared-path case.
- [ ] Record anti-cheat warnings, kicks and connectivity failures separately. Stop testing on a protection warning; never disable/bypass the anti-cheat or repeatedly retry a denied launch. A successful session is not a ban guarantee.
- [ ] Mark the catalog entry tested only for observed versions/steps and retain explicit unresolved component/traffic gaps. A service with no observed external traffic is not labelled traffic-verified.
- [ ] Build through the existing source-bound Windows portable pipeline, verify manifest/hash/package contents, and check that the ZIP includes neither personal registration nor local game paths. Reopen the packaged EXE and confirm the saved/manual/group distinction and tray/exit cleanup. Preserve existing user sessions.

## Completion criteria

The infrastructure milestone is complete when a saved group compiles into explicit paths, shared-service consent is enforced, old preferences still work, active snapshots remain immutable, inspection is read-only, and unknown coverage is visible. A real-game compatibility milestone is separate: its required DayZ/BattlEye identity and network evidence must be collected before that profile is described as tested. Unavailable game/UDP/IPv6 checks remain explicitly open.

## Review result

This draft covers catalog ownership, resolution, shared-service limitations, connection lifecycle, UI, observation and real validation. It deliberately excludes automated game launching, continuous monitoring and a remote catalog updater. New APIs above are proposed and must be implemented before their consumers. No implementation or game compatibility is claimed by this planning document.
