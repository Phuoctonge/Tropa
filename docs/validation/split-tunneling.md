# Application split tunneling — 2026-10-28

The previous game EXE picker saved a preference but left the runtime at `MATCH,WARP`. It did not implement split tunneling. The new panel explicitly selects all-computer routing or a list of exact application executable paths. Selected mode ends with `MATCH,DIRECT`; adding an executable enables that mode. Firefox Portable launchers resolve to the installed inner `firefox.exe`.

## Automated verification

- 396 backend tests passed with `clippy,portable,tropa-probe` features. Regression coverage includes routing persistence, legacy game behavior, exact case-insensitive paths, commas in paths, empty selected mode, Firefox Portable resolution, and removed executables in all-computer mode.
- 50 frontend tests passed, including the split-tunneling label and connected routing snapshot.
- Changed-file ESLint passed; TypeScript and the Vite production web build passed.
- Native Windows executable built offline with `portable,tropa-probe`, without `clippy`.
- Read-only review found no blocking issues. Cleanup-pending UI locking was corrected during review.

## Real Windows TUN verification

An isolated portable folder `.artifacts/native-connection/split test,2026-10-28` exercised both spaces and a comma in the process path. Two copies of Windows System32 `curl.exe` were verified against its SHA-256 before execution. Only the `data/smoke/selected/curl.exe` path was selected; the same executable in `data/smoke/direct` was unselected.

The first attempt failed the outbound HTTPS connection verification and disconnected cleanly. Its report is retained as `attempt1.json`. The cause of that initial failure is not established; a single successful retry does not establish connection reliability.

The second attempt completed successfully. Its `data/tropa-smoke-result.json` records:

| Stage | Selected process | Unselected process |
| --- | --- | --- |
| Before connection | `warp=off` | `warp=off` |
| Connected | `warp=on` | `warp=off` |
| After disconnect | `warp=off` | `warp=off` |

The unselected process retained the same hashed external address before, during and after connection. The selected process changed its hashed external address during connection and returned to its original address afterward. Raw external addresses were not stored in this report.

The loopback verification listener rejected unauthenticated CONNECT with HTTP 407. Successful connected diagnostics also checked that authentication was required. IPv4 route, DNS-server, adapter and Windows proxy snapshots matched before/after. No test Tropa, mihomo or WarpScout processes remained; the private runtime YAML was removed.

This test covers real IPv4 HTTPS over TCP from two process paths. Firefox/Chrome browser interaction, UDP/QUIC, IPv6, and clean-Windows startup were not verified. The implementation is not a kill switch; unidentified processes follow DIRECT in selected mode. Existing application connections may require restarting the application after changing policy.

## Portable artifact

`.artifacts/portable/Tropa-portable-split-tunneling-test.zip` contains seven allowlisted files, 60,455,086 bytes, with no account, settings or test fixtures. ZIP integrity and all manifest hashes passed. Its executable matches the binary used in the successful native test.

Executable SHA-256: `8e5f9a3d82553f34299c82819fd5d1259ab2775ac9647801abc5958bd5465e29`.

PE subsystem is Windows GUI (2); embedded manifest requires administrator privileges. This remains a development test build requiring an existing Windows WebView2 runtime.
