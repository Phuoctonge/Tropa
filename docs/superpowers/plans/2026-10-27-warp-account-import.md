# WARP Account Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task in the existing checkout. Use subagent-driven-development only if the user selects delegation.

**Goal:** Import a user's own WarpScout WireGuard registration into portable Tropa, preserve it across restarts, and manage it without exposing credentials to the interface or changing the network.

**Architecture:** Backend config owns the validated account and atomic file storage. Commands serialize the account resource through the existing config-write lock and return status DTOs only. The existing Tropa page adds import/replace/remove actions through services and the shared mutation funnel; connection remains unavailable.

**Tech Stack:** Rust, serde_json, existing base64 dependency, tokio, existing atomic writer, Tauri commands/dialog plugin, React/TypeScript, SWR query boundary.

**Spec:** [WARP registration design](../specs/2026-10-27-warp-registration-design.md).

**Execution status (2026-10-27):** Model, store, commands and UI implemented; 364 Rust tests and 45 frontend tests pass, browser regressions and native compilation pass. Independent review finding about normalized size fixed and verified. Native application smoke and Windows ACL checks remain pending the updater-isolation prerequisite. Evidence: [validation report](../../validation/warp-account-import.md).

## Global Constraints

- Portable delivery is a ready-to-run extracted folder, not a single-file executable. DLLs, sidecar executables, resources and local runtimes may be bundled. Users must not install dependency packages separately. Verify the release layout on clean Windows; do not reinterpret a missing runtime as permission to launch an installer.

- Storage: `app_home_dir()/secrets/warp-account.json`; Windows portable resolves this to `exe_directory/data/secrets/warp-account.json`.
- Ordinary plaintext file; no mandatory DPAPI, password or encryption dependency. Copying the file copies the registration.
- One imported WireGuard registration. Ignore nested MASQUE/outer registrations with a visible explanation; do not merge credentials across transports.
- Import validates format and local persistence, not Cloudflare acceptance or connectivity.
- No registration API request, scanner/core execution, TUN, UAC prompt or network change in this task.
- No credentials in Git, output DTOs, log messages, tracing fields, ordinary backup or diagnostics.
- Keep previous account authoritative if validation or atomic replacement fails. Do not reset a malformed existing file on read.
- No dependency or lockfile changes. Base64, JSON, async file I/O and the writer already exist.
- Preserve the user's current working changes. No commit until an actual Git author identity is configured; do not invent one.
- The user-approved derivative project requirements define this local work; no upstream issue or public contribution is being created.

## File map

| File | Responsibility |
|---|---|
| New `src-tauri/src/config/warp_account.rs` | Import model, validation, normalized version-1 store, status DTO, filesystem tests |
| Existing `src-tauri/src/config/mod.rs` | Register config module |
| New `src-tauri/src/cmd/warp_account.rs` | Read/import/remove commands, serialization and safe errors |
| Existing `src-tauri/src/cmd/mod.rs`, `src-tauri/src/lib.rs` | Module/handler registration |
| Existing `src/types/tropa.ts`, `src/services/tropa.ts` | Status contract and backend/native dialog boundaries |
| Existing `src/pages/tropa.tsx` | Account query, mutation funnel, inline errors and busy ownership |
| Existing `src/components/tropa/tropa-view.tsx`, `tropa.css` | Setup/status and replacement/removal confirmation views |
| Existing `src/tropa-preview.tsx` | Synthetic preview states only |
| Existing `.gitignore` | Exclude local runtime data and account exports |
| New `docs/validation/warp-account-import.md` | Actual implementation evidence and limitations |

## Task 1: Validated import model

**Files:** create `src-tauri/src/config/warp_account.rs`; modify `src-tauri/src/config/mod.rs`.

**Interfaces:**

```rust
// Secret fields remain private to config; do not derive Debug.
pub(crate) struct WarpAccount {
    id: String,
    token: String,
    private_key: String,
    peer_public_key: String,
    ipv4: std::net::Ipv4Addr,
    ipv6: Option<std::net::Ipv6Addr>,
}
impl WarpAccount {
    pub(crate) fn from_warpscout_json(bytes: &[u8]) -> anyhow::Result<Self>;
    pub(crate) fn status(&self) -> WarpAccountStatus;
}
#[derive(serde::Serialize, PartialEq, Eq, Debug)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum WarpAccountStatus {
    Missing,
    Imported { #[serde(rename = "hasIpv6")] has_ipv6: bool },
}
```

- [x] Write the following minimal regression in the new module's test section. `example_registration` is a test-only helper; values are synthetic and do not represent an actual registration.

```rust
fn example_registration() -> serde_json::Value {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    serde_json::json!({
        "id": "synthetic-device",
        "token": "synthetic-token",
        "private_key": STANDARD.encode([7_u8; 32]),
        "peer_public_key": STANDARD.encode([9_u8; 32]),
        "ipv4": "172.16.0.2",
        "ipv6": "2001:db8::2",
        "masque": { "token": "ignored-synthetic-token" }
    })
}

#[test]
fn invalid_keys_are_rejected_without_echoing_the_input() -> anyhow::Result<()> {
    let valid = serde_json::to_vec(&example_registration())?;
    let account = WarpAccount::from_warpscout_json(&valid)?;
    assert_eq!(account.status(), WarpAccountStatus::Imported { has_ipv6: true });
    assert_eq!(serde_json::to_value(account.status())?,
        serde_json::json!({ "status": "imported", "hasIpv6": true }));
    let mut invalid = example_registration();
    invalid["private_key"] = "invalid-secret-marker".into();
    let result = WarpAccount::from_warpscout_json(&serde_json::to_vec(&invalid)?);
    let error = match result {
        Ok(_) => panic!("invalid key accepted"),
        Err(error) => error,
    };
    assert!(!format!("{error:#}").contains("invalid-secret-marker"));
    Ok(())
}
```

- [x] Run `cargo test --locked -p clash-verge --lib --features clippy,portable config::warp_account`. Confirm that the new behavior is missing before implementation. A compilation failure due to undefined new symbols is expected on the initial test-first run; subsequently verify that deliberately invalid key data is rejected by the actual parser.
- [x] Implement a private deserialization DTO for the accepted WarpScout fields. Use typed IP fields, string fields for ID/token/keys and optional IPv6. Preserve unknown-field compatibility by not applying deny_unknown_fields; construct `WarpAccount` from known fields only.
- [x] Decode each key using `base64::engine::general_purpose::STANDARD`, require 32 decoded bytes and reject all-zero bytes. Require nonempty trimmed ID/token while preserving the original values. Map deserialization and field failures to fixed messages such as `Invalid WARP account format` or `Invalid WARP account key`; never return the parser error's original input value.
- [x] Add input cases to the same regression for missing ID/token, incorrect field types, wrong IP family, wrong key length and null optional IPv6. Use literal invalid inputs and assert failure/valid imported status, rather than testing source text or private field layout.
- [x] Serialize `account.status()` and assert that only `status` and `hasIpv6` occur. The secret model must not implement frontend serialization; a separate private stored DTO owns disk serialization.
- [x] Re-run the targeted tests; require success before proceeding.

## Task 2: Atomic local account store

**Files:** `src-tauri/src/config/warp_account.rs`; `.gitignore`.

**Interfaces:**

```rust
pub(crate) struct WarpAccountStore { root: std::path::PathBuf }
impl WarpAccountStore {
    // root is app_home_dir in production and a private temporary directory in tests.
    pub(crate) fn new(root: std::path::PathBuf) -> Self;
    pub(crate) async fn status(&self) -> anyhow::Result<WarpAccountStatus>;
    pub(crate) async fn import_file(&self, source: &std::path::Path)
        -> anyhow::Result<WarpAccountStatus>;
    pub(crate) async fn remove(&self) -> anyhow::Result<WarpAccountStatus>;
}
```

- [x] Add a real-filesystem regression using `std::env::temp_dir()/tropa-warp-test-<nanoid>` and the `example_registration` helper above. The directory is created with `create_dir`; files are removed explicitly and the directory with `remove_dir`. No recursive removal of a computed external path.

```rust
#[tokio::test]
async fn rejected_import_preserves_the_previous_account() -> anyhow::Result<()> {
let root = std::env::temp_dir().join(format!("tropa-warp-test-{}", nanoid::nanoid!()));
tokio::fs::create_dir(&root).await?;
let source = root.join("source.json");
let invalid = root.join("invalid.json");
let store = WarpAccountStore::new(root.clone());
assert_eq!(store.status().await?, WarpAccountStatus::Missing);
tokio::fs::write(&source, serde_json::to_vec(&example_registration())?).await?;
store.import_file(&source).await?;
let saved_path = root.join("secrets/warp-account.json");
let before = tokio::fs::read(&saved_path).await?;
tokio::fs::write(&invalid, b"not-json-invalid-secret-marker").await?;
assert!(store.import_file(&invalid).await.is_err());
assert_eq!(tokio::fs::read(&saved_path).await?, before);
assert_eq!(WarpAccountStore::new(root.clone()).status().await?,
    WarpAccountStatus::Imported { has_ipv6: true });
tokio::fs::write(&invalid, vec![b' '; 256 * 1024 + 1]).await?;
assert!(store.import_file(&invalid).await.is_err());
assert_eq!(tokio::fs::read(&saved_path).await?, before);
let mut future: serde_json::Value = serde_json::from_slice(&before)?;
future["version"] = 2.into();
tokio::fs::write(&saved_path, serde_json::to_vec(&future)?).await?;
assert!(store.status().await.is_err());
tokio::fs::write(&saved_path, b"broken-json").await?;
assert!(store.status().await.is_err());
assert_eq!(store.remove().await?, WarpAccountStatus::Missing);
assert!(tokio::fs::try_exists(&source).await?);
assert_eq!(store.remove().await?, WarpAccountStatus::Missing);
tokio::fs::remove_dir(root.join("secrets")).await?;
tokio::fs::write(root.join("secrets"), b"blocked-directory").await?;
assert!(store.import_file(&source).await.is_err());
for file in [&source, &invalid, &root.join("secrets")] {
    tokio::fs::remove_file(file).await?;
}
tokio::fs::remove_dir(root).await?;
Ok(())
}
```

- [x] Run the focused test and confirm failure before implementing the store.
- [x] Import through a bounded read: `File::open`, `AsyncReadExt::take(256 * 1024 + 1)`, `read_to_end`; reject more than 256 KiB before JSON parsing. Fully validate before creating/replacing the stored account.
- [x] Persist a normalized JSON envelope with `version: 1` and the validated registration fields. Create `root/secrets` only when saving. Use the existing `utils::help::save_yaml_str(&path, &serialized_json)` as the raw atomic string writer; it does not require YAML content and preserves the old destination until replacement. Its temporary file is created beside the destination, inside secrets. Do not call `save_yaml`, which serializes the payload again.
- [x] Read a missing file as `Missing`; return sanitized errors for unreadable/malformed/unsupported-version files. Do not call import or register while reading. `remove` deletes only `root/secrets/warp-account.json`, treats NotFound as success and leaves the source import untouched.
- [ ] Extend the filesystem test with oversized input, corrupted stored JSON, and a blocked destination represented by a regular file at `root/secrets`. Assert error and no unexpected replacement. Use a native Windows check for denied folder access rather than assuming Unix permission bits simulate Windows ACLs.
- [x] Add these ignore entries; verify using `git check-ignore --no-index` against synthetic paths. No actual key file is needed.

```gitignore
/data/
**/secrets/
warpscout-account*.json
```

- [x] Inspect `core/backup.rs::create_backup` and the sysinfo diagnostic exporter against the new store. Keep secrets out of `profiles/`, `clash.yaml` and `verge.yaml`. Current allowlists exclude the new directory, so do not add unrelated backup refactors. Record that generated runtime configs need a separate export review when the tunnel is integrated.
- [x] Run the focused tests and `git diff --check`.

## Task 3: Command and service boundaries

**Files:** create `src-tauri/src/cmd/warp_account.rs`; modify config/cmd module lists, `src-tauri/src/lib.rs`, `src/types/tropa.ts`, `src/services/tropa.ts`.

**Interfaces:** commands return `CmdResult<WarpAccountStatus>`. Reads and writes acquire `Config::lock_config_write` before constructing the store. No lifecycle lock is needed while there is no Tropa session consumer; introducing that consumer requires the active-registration guard described in the spec.

```rust
#[tauri::command]
pub async fn get_warp_account_status() -> CmdResult<WarpAccountStatus>;
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn import_warp_account(path: std::path::PathBuf) -> CmdResult<WarpAccountStatus>;
#[tauri::command]
pub async fn remove_warp_account() -> CmdResult<WarpAccountStatus>;
```

- [x] Implement each command using the existing Tropa command pattern: acquire the config-write lock, resolve `dirs::app_home_dir()`, call the store and return the status. Use stable codes `WARP_ACCOUNT_READ_FAILED`, `WARP_ACCOUNT_IMPORT_FAILED`, `WARP_ACCOUNT_REMOVE_FAILED`; return only sanitized fixed detail. Do not attach underlying serde or file contents as an error cause.
- [x] Register the module and the three fully qualified command paths in `tauri::generate_handler!`. Do not change generated event contracts: this stage uses query/mutation responses, not a new event stream.
- [x] Add the exact frontend contract and wrappers:

```ts
export type WarpAccountStatus =
  | { status: 'missing' }
  | { status: 'imported'; hasIpv6: boolean }

export const getWarpAccountStatus = () =>
  invoke<WarpAccountStatus>('get_warp_account_status')
export const importWarpAccount = (path: string) =>
  invoke<WarpAccountStatus>('import_warp_account', { path })
export const removeWarpAccount = () =>
  invoke<WarpAccountStatus>('remove_warp_account')
export const chooseWarpAccountFile = () => open({
  title: 'Выберите регистрацию WarpScout', multiple: false, directory: false,
  filters: [{ name: 'Регистрация WARP', extensions: ['json'] }],
})
```

- [x] Run backend targeted tests and frontend typecheck. Verify serialized command payloads contain only a file path, and returned values contain status metadata rather than ID/token/key strings. The frontend test boundary may mock Tauri transport; do not mock the actual config parser/store in its regression tests.

## Task 4: Import/replace/remove screens

**Files:** `src/pages/tropa.tsx`, `src/components/tropa/tropa-view.tsx`, `tropa.css`, `src/tropa-preview.tsx`.

- [x] Add a query with key `['tropa-warp-account']`, `queryFn: getWarpAccountStatus`, no silent fallback from read error to Missing. Show a read error with explicit replace/delete recovery actions.
- [x] Use the existing busy/error owner for account actions. Native picker cancellation performs no mutation. Import and removal use the same `mutate` identity `tropa-warp-account`, `errorNotice: false`, and revalidate `[['tropa-warp-account']]` only after success. Display operation-specific Russian errors without the raw returned payload.
- [x] Extend the pure view with `accountStatus?: WarpAccountStatus`, `accountError?: string`, `onImportAccount: () => void`, `onRemoveAccount: () => void`. A missing initial query is loading, not Imported. Reuse screen-history Back navigation.
- [x] Setup screen copy: missing → «Импортировать регистрацию»; imported → «Регистрация импортирована · WireGuard», «Заменить» and «Удалить»; read failure → explicit error and recovery actions. Inline confirmation before replacement/removal must preserve the old status on failure. Explain that only the main WireGuard registration is imported and copying the local plaintext file copies the registration.
- [x] After import change the main hint to «Регистрация добавлена; подключение ещё не настроено». Keep the connection switch disabled and the server list empty. No Connected/Connecting simulation is introduced.
- [x] Extend the separate browser preview with synthetic missing/imported/error states; keep it independent of native application startup. Exercise import success, replacement failure and removal through callback state, without using real credentials.
- [x] Add a browser regression before fixing any new interaction issue: assert the displayed status follows fulfilled mutations, canceled picker does not change it, repeated actions stay blocked, Back returns to the origin. Re-run the existing theme/compact/navigation checks. Native picker and filesystem persistence are checked separately after the safe startup prerequisite is met.

## Task 5: Verification and handoff

- [x] Run `npx.cmd --yes pnpm@12.10.0 typecheck`, `lint`, `test`. Run `cargo test --locked -p clash-verge --lib --features clippy,portable` with the existing project-local Rust environment. Compare against the current baseline of 45 frontend and 362 portable app-library tests; count newly justified regressions rather than treating counts as a target.
- [x] Run formatting checks on touched files and `git diff --check`. Check that dependency lockfiles are unchanged.
- [x] Build `npx.cmd --yes pnpm@12.10.0 exec tauri build --debug --no-bundle --features portable -- --locked`. This compiles without launching the app. Record hash, size and exit code.
- [x] Request one read-only review of model/store, command secrecy, frontend states and overwrite/error semantics. Resolve material findings and rerun affected checks.
- [ ] Before any native smoke run, complete the separate updater-isolation prerequisite from the spec and inspect current startup network defaults. Use a fresh isolated data directory and synthetic import; verify restart persistence and original-file preservation. Do not auto-register a Cloudflare device as part of an import test.
- [x] Write `docs/validation/warp-account-import.md` with actual evidence, remaining limitations and whether native smoke testing ran. The completed stage means «own registration can be imported and managed», not «WARP tunnel works».

## Execution boundary

Implementation and verification status are recorded above and in the validation report. Native smoke remains pending. Next network stages are specified in the design's rollout table. Automatic registration, endpoints, game rules, elevation and recovery require their own concrete plans after the preceding evidence is available. Do not expand this import task into a generic account system or a second core manager.
