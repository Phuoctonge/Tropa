# MASQUE shared runtime foundation — 2026-10-28

This implements the code/configuration portion of milestone 6. It is not evidence that H3/H2 connects on the user's network. Tropa, mihomo, WarpScout, UAC and TUN were not launched. No Cloudflare enrollment request or network/service setting change was performed.

## Common configuration and identity

`core/warp_runtime.rs` now owns the shared probe and persistent-runtime builders. The existing WG/AWG builders were moved from `warp_probe.rs`, retaining AWG I1/junk/header fields, remote DNS behavior and selected-process rules. Existing WG/AWG regressions remain active. All profiles are validated before configuration creation.

`RuntimeIdentity` distinguishes a borrowed WG account from a borrowed MASQUE account. `RuntimeAccount` owns the selected family's loaded identity. Loading/ensuring uses only the profile's family; an unreadable other-family file cannot force substitution or prevent loading a valid identity. Missing or corrupt required identity is an error. Existing completed state is reused, while a missing MASQUE identity enters the separately tested pending/enrollment workflow. Invalid profile parameters are rejected before registration. Secret-bearing accounts still have no Debug implementation and are not returned through the GUI status contract.

The MASQUE proxy uses the enrolled device private key as base64 SEC1 DER, the server peer public key as base64 SPKI DER, assigned v4/optional v6, explicit SNI, URI `https://cloudflareaccess.com`, UDP and MTU 1280. The device's own public enrollment key is deliberately not used as the server peer. H2 adds `network: h2`; H3 uses the ordinary CONNECT-IP default and never `h3-l4proxy`. `skip-cert-verify` remains explicitly false even for a custom SNI.

Both probe and persistent connection paths now use this common builder and selected-family loader. Probe registration preparation participates in cancellation before any validator/core child is started. The persistent coordinator loads the already prepared identity; it does not create another device while enabling TUN. After cached validation, the catalog binds the actual loaded family's identity before recording success or ordinary rejection. Scanner batches likewise bind their family's loaded account. This avoids saving a fresh result under the pre-enrollment fingerprint.

The persistent configuration continues to start with TUN disabled, uses the existing full-path selected-process rules and DIRECT fallback, and retains the existing authenticated loopback verification for selected routing. The outer endpoint alone is excluded from TUN as a /32; no whole Cloudflare subnet is excluded. Explicit IP-literal bootstrap DNS is `https://1.1.1.1/dns-query#DIRECT`, avoiding implicit upstream defaults and hostname bootstrap. Normal all-computer DNS still uses #WARP and selected-application mode still uses #DIRECT. These are generated configuration contracts, not native DNS/TUN proof.

## Pinned source evidence

Two additional Go files were retrieved as source only through the already pinned v1.19.33 tree. Their Git blob SHA-1 was recomputed over the Git blob header and bytes before saving:

- `config/config.go`: `cc8c1016703c21f502cfb4151f5f0131bbb3be81` — confirms `route-exclude-address` and `default-nameserver` fields and IP-literal DNS requirements.
- `transport/masque/masque.go`: `a95f4dd0f64f23dfbe8747ba356f7790109025c2` — confirms the URI/SNI constants and peer-key comparison in `PrepareTlsConfig`.

They are under `.artifacts/research/upstream-review-2026-10-28/MetaCubeX__mihomo/pinned-v1.19.33/`. No downloaded Go code was compiled or executed. The previously inspected `adapter/outbound/masque.go` consumes SEC1 private/SPKI peer keys and selects H2 by the network field. Its TLS helper bypasses hostname/chain verification in favor of the enrolled ECDSA peer pin; the `insecure` option removes that pin callback. Tropa therefore must keep its corresponding skip option false. Actual rejection of a wrong peer key remains a native gate, not something these Rust configuration assertions prove.

## Probe runtime ownership fix

Inspection found an inherited lifecycle problem: the probe removed its secret runtime before checking that child shutdown succeeded, and its local scope guard also deleted the file on an unconfirmed shutdown path. Extending that behavior to MASQUE would have violated the runtime ownership requirement.

The Windows child supervisor now retains an arbitrary shared resource owner, rather than only the execution guard. Both the configuration validator and the transport child retain a composite owner containing the runtime-file cleanup guard followed by core execution ownership. Its field/drop order removes the old file before releasing the execution slot. Cancellation/return of the caller cannot remove that file while the tracked child remains alive; the OS exit monitor releases the final owner. Normal explicit deletion follows successful stop and confirmed child-monitor completion. The existing persistent-core shutdown ordering remains unchanged.

A regression first failed because `track` could not own the runtime resource lease, then passed: a system `ping` child directed only at loopback retains a synthetic temporary runtime after caller ownership is dropped; after the child is killed and OS-confirmed exit is awaited, the file disappears. This exercises real OS process monitoring without a VPN, downloaded executable, TUN, UAC, game or anticheat. It does not prove every native mihomo cleanup path. In particular, Windows-specific monitoring and scanner-owned temporary-file cleanup must still be exercised in their deferred native matrix; scanner account/report resources have not been converted to this composite lease in this increment.

## Verification and remaining gates

The new runtime tests first failed on the missing shared builder/identity loader. A test assertion initially had a Rust temporary-reference lifetime error; that test error was corrected before the behavior checks passed. The outer-endpoint regression then failed with a missing exclude list and passed after configuration changes. Tests check both H3/H2 fields, preserved enrollment keys/SNI, correct peer versus own public key, mandatory peer verification option, common routing, no API ID/token in runtime, incompatible-family rejection, required-family-only loading, corrupt other-family isolation, exact endpoint exclusion and explicit bootstrap DNS.

Final commands after all source changes:

- `cargo check --locked -p clash-verge --lib --features portable --offline --quiet`: exit 0, no warnings. This is a production-feature Rust compile check, not a release executable or portable ZIP build. The diagnostic profile wrapper is now gated by `tropa-probe`, matching its only consumer.
- `cargo test --locked -p clash-verge --lib --features clippy,portable,tropa-probe --offline --quiet`: all **431 tests passed**, exit 0, no compiler warnings.

Scoped changed Rust files were formatted. Test-owned temporary runtime identity files and lease markers were removed. Frontend checks were not repeated because this increment changes no frontend/wire contract.

Milestones 5–7 are still incomplete. The scanner adapter and scheduling still support discovery of WG/AWG only; the current initial policy guard also rejects MASQUE-only discovery. Thus the new runtime path must not be presented as a complete user-selectable four-mode connection feature. Next work is the MASQUE scanner account export/parser and independent H3/H2 scheduling, including moving that discovery capability check to the correct stage for cached/manual profiles. Native enrollment and mihomo format acceptance, HTTPS/UDP through H3/H2, actual selected/unselected processes, wrong-peer failure, cleanup/cancellation, and H2 performance under loss remain unverified. No new archive is being delivered as ready.


## Scanner parser and resource ownership increment

The adapter now has a transport-aware export parser for all four modes. MASQUE H3 accepts the pinned export's default network selector; H2 requires explicit `network: h2`. The parser checks the selected endpoint against the exported server/port and the selected transport's allowlist, validates SNI through `WarpProfile::new`, and rejects insecure peer verification, H3 L4-proxy, foreign CONNECT-IP URI and unexpected nested WG options. Exported private/public keys and assigned addresses are never copied into a catalog profile; runtime configuration continues to use the Tropa-owned identity. WG/AWG still pass through the existing strict header/junk parser. No MASQUE CLI task is enabled by this increment.

Report endpoints now parse as IPv4 socket addresses before transport-specific profile validation. Previously the WG endpoint parser rejected valid MASQUE addresses. A hand-written H3 row with no endpoint ping and a 12.3 ms tunnel ping now produces a 13 ms display value and retains loss/node data; a WG address in that H3 report is rejected. The regression was observed failing with the old WG-only allowlist, then passed with the transport-specific validation. The two new parser tests initially could not compile until the missing transport-aware API was implemented.

The scanner's account/export/report files now share `ScannerResources` with execution ownership. The OS child supervisor retains this composite resource after caller cancellation/drop. Normal reading/deletion is ordered after confirmed child stop and exit-monitor completion. The resource drop removes the three files before releasing its execution lease. A local system-loopback-ping test verifies that all three synthetic files and the lease survive caller ownership drop and disappear after OS-confirmed exit; no WarpScout process was started. This closes the scanner resource-lifetime gap identified in the earlier runtime increment; native scanner/Job Object cancellation remains unverified.

Fresh verification: 434 Rust library tests passed with `--locked --offline --features clippy,portable,tropa-probe`; production library check passed with `--locked --offline --features portable`, both without warnings. The frontend was unchanged. No Tropa, mihomo, WarpScout, UAC or TUN was launched.

Additional pinned source evidence: `main.go`, Git blob `2cf193cea889ab9b872a0615d2aba011b64b25f3`, was downloaded as text and its Git blob SHA-1 verified before saving. `loadScanAccount` invokes the WG-root-requiring `loadAccount` even for MASQUE, then `applyAccount`; it does not enroll missing MASQUE state itself. Existing `register.go` and `masque.go` show that the nested MASQUE transport consumes its own keys/IPs, while WG root fields remain mandatory to the shared loader. A correct adapter envelope and family-isolated error handling still need implementation; arbitrary root sentinels have not been introduced.

`flags.go::rejectMasqueFilters` explicitly rejects node/country inclusion/exclusion CLI flags for MASQUE. The future CLI adapter must omit those flags for H3/H2 and preserve Tropa policy enforcement on fresh runtime metadata; forwarding the WG/AWG arguments would cause scanner failure. Four-mode scheduling/progress, separate account export and coordinator enablement remain open, along with the native gates listed above.


## Four-mode scanner/coordinator integration

This increment supersedes the WG/AWG-only discovery limitations above at source level. It does not close native acceptance gates.

`MasqueAccount::scout_account` emits a temporary nested MASQUE account using the enrolled private SEC1 key, server peer PEM public key, and assigned IPs. IDs and management tokens are omitted. Pinned v0.16.1 `loadAccount` only checks nonempty root private/peer strings before `applyAccount`; `runScanCmd` selects `newMasqueTunnel` for the requested MASQUE protocol. The compatibility envelope duplicates the genuine EC values in the required root fields instead of reading/registering a WG device or inventing sentinel credentials. Those EC values cannot be used as a valid 32-byte WireGuard identity. This is tightly scoped to the pinned scanner and must be re-reviewed on upgrade; native loading of the envelope has not been executed.

`ScoutPreset` exhaustively maps supported names to WG, AWG and H3/H2; unknown names fail rather than becoming AWG. It builds protocol arguments and a validated family seed for `RuntimeAccount::ensure`. Scanner registration now reads/ensures only the requested family, participates in the task deadline/cancellation, and checks cancellation/deadline again before spawning. MASQUE receives its own exported account. Node/country flags are forwarded only for WG/AWG because the pinned MASQUE scanner rejects them. Tropa's fresh runtime metadata policy remains mandatory for H3/H2 as well; missing/mismatching metadata cannot pass a constrained connection. Scanner rows still do not prove runtime connectivity.

Search progress now contains all four modes and policy filters remove disabled tasks before budget allocation. Initial slices are AWG standard, WG, H3, H2, each capped at 30 s in the 180 s quick search or 90 s in the 600 s extended search. If cache work has left less than the initial reservations, the remaining time is shared among unstarted initial tasks rather than giving them zero. The six expansion tasks retain WG/AWG variants; nominal extended slices are 40 s, quick slices 10 s, with actual leftover time redistributed. The initial/expansion task distinction remains separate from protocol variants. These budgets include enrollment/scan/runtime validation; confirmed cleanup can add its bounded teardown time.

A fresh quick search no longer exits after the first successful transport. It visits the initial task of each policy-enabled mode, compares runtime-confirmed observations through existing catalog ranking, and skips expansion if it already has a verified result. A valid cached profile still returns after successful revalidation for fast startup; this remains a best-of-checked result, not a global optimum. Manual pins retain exact-profile behavior and no silent replacement.

A typed registration error in automatic cached validation or a scanner task is recorded for that transport and can continue to another mode. Unknown ownership errors, runtime errors, cleanup failures and cancellation remain terminal. Neither registration file is erased or substituted. Repeated same-family preparation after a failed task, network-context freshness, and native cancellation/quality remain follow-up work.

The policy editor now exposes four modes (all selected for an unrestricted policy), and the active-search label distinguishes H3 and H2 instead of displaying AWG. Existing server table protocol/status fields handle the new results.

Local regression evidence: tests first failed for missing account/preset APIs, missing H3 scheduling and progress state, and the incorrect MASQUE UI label. After implementation, all 439 Rust library tests and 59 frontend tests in 15 files passed. TypeScript, scoped ESLint and Vite build passed. A test-only unused import surfaced in the production check and was gated to tests; the final portable-feature check passed without warnings. No Tropa/mihomo/WarpScout or tunnel was run. Current source integration is not a verified portable release.

Next source work: suppress repeated failed registration preparation for the same family in one search, improve independent search controls/recovery and network freshness, then continue resilience and packaging preparation while native gates remain deferred. Native tests must still cover the scanner account envelope, actual H3/H2 config loading and peer pin rejection, HTTPS/UDP and selected/unselected processes, H2 under loss, process/Job Object cleanup, UAC/WebView2 and clean Windows portability.


The subsequent registration-isolation/health increment supersedes the repeated-family-preparation follow-up above: the coordinator now skips a failed identity family for the rest of that search and excludes its tasks from budget reservation. Details and current 443/62 test results are in `fair-warp-search-2026-10-28.md` and `connection-health-2026-10-28.md`. Native four-mode acceptance remains open.
