# Portable WebView2: implementation constraints

## Observed baseline

Package `Tropa-owned-search-2026-10-28-l.zip` requires an existing WebView2 Runtime. It is an extracted-folder development package, not proof of autonomous startup on clean Windows. A current system's installed Evergreen Runtime cannot establish that requirement.

The pinned Tauri 2.12.1 already handles `bundle.windows.webviewInstallMode = { type: "fixedRuntime", path: ... }` during application startup, independently of whether installer bundling is active. In `tauri-2.12.1/src/app.rs`, before constructing the runtime, it resolves that relative path against the current executable directory and sets `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`. This supports relocation without a custom launcher or copying a system Evergreen installation.

## Recommended delivery

Ship a pinned official x64 Fixed Version Runtime in a directory next to `Tropa.exe`. Configure the existing Tauri runtime selection and extend the current package inventory/verifier to cover that exact directory, version, hashes and applicable license texts. The app, runtime and worker updates remain one reviewed package. Keep the current source-bound application/core/worker checks; a runtime manifest cannot replace them.

Do not use an Evergreen bootstrapper, runtime installer or a user's installed browser directory as the autonomous portable solution. Do not declare a minimum Windows version or CPU compatibility from the Rust target alone: the selected WebView2 release and the x86-64-v2 mihomo core both constrain the actual package.

An official x64 Fixed Version **156.0.4314.50** candidate was subsequently selected from the captured Microsoft download page and downloaded for static inspection. Its CAB is 310,130,282 bytes, SHA-256 `fcf0cd55f7c6e8efced05114372de8b8ffc9032c0296d16e060d260637dfe5f2`; Windows Authenticode returned `Valid`, signer Microsoft Corporation. Extraction produced 257 files totalling 706,172,999 bytes. Both `msedgewebview2.exe` and `msedge.dll` report version 156.0.4314.50 and valid Microsoft signatures. All extracted file hashes were recorded. The runtime has not been executed, installed, adopted into Tropa or included in package l.

The extracted archive includes `show_third_party_software_licenses.bat`, which opens its embedded credits viewer; it was read as text, not executed. Preserve the runtime's notices/credits. The actual Fixed Version terms were subsequently retrieved as described below; obtaining their text does not establish compliance with every distribution condition.

No Tauri configuration, ACL, registry value or installed runtime was changed. This is a downloaded and inspected candidate plus remaining delivery design, not implemented runtime support, clean-Windows acceptance or release approval.

## Exact Fixed Version terms retrieved

The saved official download page loads the Microsoft CDN entry `xqNrN82F.js`, which maps `BlockWebView2` to `BQyHLXAt.js`. Reading that module as text identifies the actual dialog request: `/microsoft-edge/api/eula/webview2?locale=en-us&fixed=true`. On the original `developer.microsoft.com` origin this returned HTTP 200 and `fixedHtml` headed **MICROSOFT EDGE WEBVIEW2 RUNTIME (FIXED VERSION)**. The no-`fixed` request returns Evergreen terms instead; neither those nor general Edge terms were substituted for the Fixed Version text.

The Fixed Version response is 25,752 bytes, SHA-256 `a75e803e5567be164c62b6a3dc756188df6d0f91c2d913f4c4ae75db3ad61c34`. Source URL, response URL, resource hashes, original JSON, extracted HTML and a plain-text reading copy are saved under `.artifacts/validation/webview2-terms-2026-10-28/`; reproducer: `.artifacts/validation/review-webview2-terms-2026-10-28.py`. No dialog was accepted and no downloaded script was executed.

The practical delivery requirements in that text are concrete:

- Section 2(a) permits copying and distributing object code with applications. Section 2(b) requires significant primary application functionality, appropriately protective downstream terms, acquisition directly from Microsoft and the stated distribution indemnity. Merely including a license filename does not prove those requirements have been met.
- Sections 2(c) and 3 prohibit the specified trademark implications, subjecting Microsoft code to source-disclosure/modification obligations, removing notices and a standalone runtime offering. Keep the unchanged Microsoft runtime and its applicable terms distinct from Tropa's GPL source and licenses; review the actual combined distribution before release.
- Sections 4(b) and 8 require an end-user SmartScreen notice unless that feature is disabled. No SmartScreen setting was changed. The release must disclose that the included Microsoft Defender SmartScreen collects and sends end-user information to Microsoft, with the specified privacy-statement and Edge privacy-whitepaper links. Section 4(a) separately addresses data collection and appropriate notices.
- Third-party component notices and the H.264/AVC notice remain applicable as stated in sections 1(c) and 7. Preserve the runtime's supplied notice/credits mechanisms.

Remaining work is to implement the actual package/runtime inventory, include the authoritative terms and required user notice, establish the downstream agreement path and verify the portable startup. This review records the requirements; it does not declare them satisfied or change security features to avoid them.

## Microsoft distribution constraints

The official distribution guidance states:

- Fixed Version binaries exceed 250 MB in unpacked form. Final ZIP size must be measured; it is not the current 55 MB package plus a known compressed increment.
- Fixed Version does not use a runtime registry key and is not updated automatically. Runtime security updates become the distributor's responsibility.
- Fixed Version cannot run from a network location or UNC path. A local portable directory is the supported target for this approach.
- For unpackaged Win32 applications on Windows 10, Fixed Version 120+ requires read/execute access for `ALL APPLICATION PACKAGES` and `ALL RESTRICTED APPLICATION PACKAGES` on the runtime directory. This particular requirement does not apply to Windows 11 according to the guidance. A ZIP does not by itself prove the extracted permissions are sufficient.
- Actual redistribution terms and notices must be reviewed for the chosen official runtime. Having a technical way to select it is not license clearance.

Restrict any required permission handling to the known runtime directory and the documented principals. Do not change unrelated ACLs, disable the renderer sandbox or add a hidden installer. Handle unsupported paths and initialization failure with an actionable startup error; a web-only error screen cannot be assumed available when WebView2 itself is missing.

## Required acceptance

Use a clean Windows/VM without Evergreen WebView2. Extract the final package to local paths with Cyrillic and spaces, launch it, verify the packaged runtime version actually used, connect/disconnect and exit, then move the entire directory and repeat. Cover Windows 10/11 separately if both remain supported, including the runtime directory permissions on Windows 10. Test declined UAC, missing/corrupt runtime files and update/rollback of the folder without overwriting private registrations. Test unsupported UNC behaviour separately from the supported local-path scenario.

Only this clean-system evidence can close autonomous portable startup. The existing extracted-worker hello/shutdown checks remain useful but do not exercise WebView2 or the GUI.

## Evidence

- [Microsoft: Distribute your app and the WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution), especially Fixed Version distribution and known issues.
- [Official WebView2 download page](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).
- Saved public pages: `.artifacts/validation/webview2-portable-2026-10-28/{distribution.html,download-page.html}`.
- Candidate URL, source-page hash and successful HEAD metadata: `candidate.json` in that directory. Downloaded CAB hash/signature: `download-result.json`; exact extracted-file hashes: `runtime-files.json`. The separate `products.json` enterprise API response did not list WebView2 and was not used as the candidate authority.
- Actual pinned local implementation: `.tools/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-2.12.1/src/app.rs`, inspected before planning a custom loader.
