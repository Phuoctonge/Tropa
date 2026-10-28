# Preliminary Tropa interface

Date: 2026-10-27. Implements the interface approved in chat and recorded in [the interface design](../superpowers/specs/2026-10-27-tropa-interface-design.md).

The application root now opens a minimal Tropa screen: connection switch, selected game, Automatic server row, settings access. Game, server, settings and WARP setup explanations replace the same window's content. System/light/dark themes use existing backend-owned theme configuration. Windows portable defaults to 420 by 720 logical pixels with a 360-pixel minimum width. Other builds retain their previous dimensions. The native window and HTML title are Tropa.

One local game executable can be selected using the native file picker and removed. Its path is persisted atomically in `data/tropa.yaml` by backend config, with serialized reads/writes. The file is not executed. A missing preferences file gives an empty selection; malformed data reports an error rather than silently resetting it. Saving validates an absolute path to an existing `.exe` file before replacing the previous selection.

Technical upstream routes remain available through Settings, with their Home dashboard at `/advanced` and a separate return link to Tropa. Review identified a mismatch between the route and sidebar target; both now use the shared navigation path. The return icon is provided for both monochrome and colorful sidebar modes.

## Verification evidence

- The persistence regression first failed because a `.txt` selection replaced the saved game. After validation was implemented, the test passed and confirmed that the original choice survives rejection and malformed YAML is reported.
- The compact-window regression first failed because the original minimum width rejected a 420-pixel window. It passed after the portable sizing adjustment.
- Full portable app-library suite: 362 passed, 0 failed. Log: `.artifacts/tropa-interface-backend-tests.log`.
- Frontend typecheck and lint passed; 45 tests in 11 files passed. The final native build also runs TypeScript checking and the Vite production build.
- Headless Chrome checked home/game/server/settings/setup navigation, disabled connection, theme changes and no horizontal overflow at 320 by 600. A follow-up check reproduced incorrect Back navigation from Home to WARP setup; navigation now follows a local screen history stack, and both Home and Settings entry paths return correctly. No browser runtime errors were reported in the final checks. Screenshots were visually inspected in light, dark and compact layouts. Script and screenshots are in `.artifacts/tropa-ui-check.mjs` and `.artifacts/tropa-*.png`.
- Independent read-only source review covered preferences, routing, truthful UI states and portable dimensions. Its routing and collapsed-icon findings were corrected.

The embedded browser automation runtime failed during sandbox setup; the visual and interaction checks used the bundled Playwright package with installed headless Chrome. This was a separate preview page, not a launch of the native networking application.

## Product limits

This is a preliminary interface. The WARP registration importer, candidate discovery and connection coordinator are still unimplemented. The connection switch is explicitly unavailable and the server list is empty; no simulated connection, location or latency is shown. Choosing a game does not install routing rules. The preview does not persist choices and cannot open the native file picker; the application adapter uses the backend for those operations.

The native application, native picker, saved theme across a real restart, TUN cleanup and game connectivity have not been exercised interactively. Upstream updater/package identity isolation and portable ZIP packaging are still separate work. No native Tropa run changed the service or network configuration during this interface task.

A separate local preview can be served with `npx.cmd --yes pnpm@12.10.0 exec vite --host 127.0.0.1 --port 5173 --strictPort` and opened at `http://127.0.0.1:5173/tropa-preview.html`. This entry is not included in the normal Vite production HTML build.

Final native portable interface build passed with exit 0; compilation 1m 20s. Artifact: `target/debug/clash-verge.exe`, 87222784 bytes, SHA256 `99283adaea7325e83497636d52f5184616fa8f9c9f43e81debde382e34218dca`. Log: `.artifacts/tropa-interface-build.log`. The build includes the final screen-history and technical-navigation fixes. It remains a debug build without an installer or portable ZIP.
