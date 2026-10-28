# Session-owned connection health — 2026-10-28

This is source implementation and local HTTP regression evidence for milestone 8. No Tropa, mihomo, WarpScout, UAC, TUN, network/service configuration, game or anticheat was launched or changed. Native sleep/network/routing and cleanup gates remain open.

## Behavior

The connection snapshot now includes `health: healthy|degraded|unreachable|unknown`, independent of `ConnectionPhase`. A verified newly connected session starts healthy. A session-owned background task checks the WARP data path on a 15-second interval, with a 5-second total request bound, no automatic retries and no redirects. One or two consecutive failed checks mean degraded; three or more mean unreachable. A successful check recovers healthy without reconnecting or changing the selected profile. A snapshot older than 45 seconds becomes unknown; a long pause cannot retain a stale healthy label. Missed timer ticks are skipped rather than queued for a burst after resume.

The monitor always uses a generated, authenticated loopback mixed listener pinned to the WARP outbound, for selected routing and all-computer routing alike. The client disables ambient/system proxies before installing that explicit loopback proxy; it has no DIRECT fallback. Production checks target HTTPS `https://1.1.1.1/cdn-cgi/trace`, retain normal TLS verification, require a bounded valid response with warp=on/plus, and do not publish proxy passwords or trace content. The listener password remains only in the owned runtime and client. The existing initial all-computer verification still checks the system TUN path; selected initial verification still uses the pinned listener.

This checks that the selected WARP outbound can pass HTTPS data. It does **not** prove routing of every application, UDP quality, DNS restoration, or the absence of a TUN routing fault while the outbound still works. Native selected/unselected TCP/UDP/IPv6 validation remains separate.

## Ownership and cancellation

`SessionHealth` has a generation and active flag. A completed check applies only to its own active generation; stopped/previous sessions cannot mutate a new session. `HealthMonitor` aborts its Tokio task on drop. Cancel/disconnect/exit invalidate health and drop the owned monitor; core-process-loss snapshot handling does the same. Starting a monitor is serialized with its ownership slot and checks the operation cancellation token and exit flag before publication. Cancel signals the operation before stopping health, closing the cancel-before-monitor-publication gap. Core ownership and teardown requirements remain unchanged; the monitor does not start another core or switch endpoint/protocol.

The GUI keeps the active switch available for disconnect, while replacing unconditional Connected/routing claims with unstable/no-data/checking labels for degraded/unreachable/unknown. The existing 700 ms background snapshot polling delivers health updates; no new event channel was introduced. The frontend health field is optional for compatibility with existing preview fixtures, while the backend always emits it.

## Local evidence and remaining gates

A state regression first failed for missing health APIs. It covers consecutive failures, recovery, stale observations after a long pause, stopped-session callbacks and an old generation completing after a new one starts. A local real HTTP-proxy fixture verifies the absolute proxy request and Basic authentication, drives warp=off to unreachable and warp=on back to healthy, then verifies task abort and unknown state after stop. The existing pre-publication cancellation regression was extended to reject monitor startup after cancellation. No external address is contacted by these tests: the reserved `.invalid` HTTP target is served only by the loopback test proxy.

Three UI regressions first failed with the old unconditional Connected text, then passed for degraded/unreachable/unknown while retaining Disconnect. Final verification: 443 Rust library tests passed with `--locked --offline --features clippy,portable,tropa-probe`; production `--features portable` library check passed without warnings; 62 frontend tests in 15 files, TypeScript, scoped ESLint and Vite build passed.

Milestone 8 is incomplete: real internet loss, sleep/wake, interface/network change, 60-minute sessions, 20 connect/disconnect cycles, native crash/Job Object teardown, route/DNS restoration and selected/unselected process behavior remain unverified. No public-ready archive or release claim is made. Next source work includes independent protocol search controls and network-context freshness before continuing packaging/release inventory; native gates remain deferred by the user.

## Ошибка чтения состояния и cached healthy

Обнаружена UI-регрессия: query сохраняет последнюю ConnectionView при IPC read error; заголовок и routing hint продолжали подтверждать connected/healthy. Страница теперь передаёт connectionUnavailable, а view показывает «Состояние недоступно» и причину отсутствия свежего состояния. Предыдущий снимок не удаляется и продолжает удерживать routing lock; существующая блокировка переключателя при read error сохранена. Успешное следующее чтение автоматически снимает этот override.

SSR regression воспроизвела прежний healthy heading и после изменения прошла. Проверены 72 frontend tests, TypeScript, scoped ESLint и Vite production build. Это проверка отображения cached/query-error состояния, не реальный обрыв IPC или проверка сна/TUN. Предыдущий portable ZIP изменения не содержит.
