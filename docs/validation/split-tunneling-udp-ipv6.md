# UDP / IPv6 split tunneling: подготовка и ограничения

## Game, launcher and anti-cheat routing requirement

Added from the user's concern on 2026-10-28. Selecting an EXE changes network routing rules; it does not change that process's Windows privileges, inject code, attach a debugger, or modify its anti-cheat. Current selected mode matches exact executable paths and sends unmatched traffic DIRECT. A game's launcher, authentication helper, anti-cheat service or driver-associated traffic can therefore follow a different path from the selected game. This possibility is unverified for actual games and is not evidence of a ban condition.

Before recommending a game-specific split profile, identify the actual connection owners and paths for the game, launcher and anti-cheat service. Check the game's official VPN/proxy and anti-cheat policies separately from technical observations. Do not infer service ownership from executable names or automatically route a system antivirus. If the user meant an antivirus rather than an anti-cheat, its update/telemetry traffic requires separate evidence of relevance.

First verify service/process attribution using owned synthetic programs, including service-owned sockets, children, reconnects and UDP/IPv6. For an explicitly authorized game test, compare DIRECT, all-traffic WARP and the selected application group; observe authentication and session continuity as well as actual TCP/UDP/IPv6 paths. Include game/launcher/service startup before and after tunnel activation, and avoid changing the endpoint mid-session. Record unidentifiable driver/System traffic as unverified rather than inventing an executable rule. Do not intercept or reverse engineer anti-cheat internals.

All-traffic mode is an existing alternative that reduces dependence on per-process attribution; it is not a guarantee of equal egress for every address family, compatibility, or freedom from account sanctions. Ordinary Cloudflare WARP likewise establishes no universal anti-cheat approval. Recommend neither mode as ban-safe without authoritative game-specific evidence. Launching games or anti-cheats still requires separate user authorization; this requirement does not authorize those launches. No game or anti-cheat has been tested for this addition.

## Статус на 28 октября 2026

Нативные проверки Windows TUN отложены по прямому указанию пользователя: запуск Tropa требует подтверждения UAC, сейчас подтверждать его некому. Tropa, mihomo и WarpScout для этих проверок не запускались. Текущее подтверждение разделения приложений остаётся прежним: два EXE-пути, IPv4 HTTPS/TCP, см. [предыдущий протокол](split-tunneling.md). UDP, внутренний IPv6, браузеры/QUIC и античиты этим не подтверждены.

Подготовлены:

- `src-tauri/src/bin/tropa-routing-fixture.rs`: отдельный TCP/UDP тестовый клиент, feature `tropa-probe`, без вызовов app_lib, UAC, TUN или изменения сети. Одинаковое имя файла можно использовать в двух разных папках.
- `scripts/tropa-routing-echo.py`: собственный opt-in responder для контролируемого удалённого узла. Его сервер в этой сессии не запускался. Он принимает ограниченный nonce-протокол и возвращает адрес источника, не записывая IP в логи.
- `scripts/test-tropa-routing.ps1`: ручные фазы baseline / connected / after. Скрипт не запускает Tropa, не запрашивает UAC, не меняет настройки подключения, маршруты, DNS, proxy и адаптеры. Он запускает только явно указанный собственный fixture, копирует его в selected/direct и собирает измерения и хэши сетевого состояния.

Эти инструменты диагностические; обычный portable-упаковщик их не включает. Отдельная сборка fixture не является сборкой Tropa для пользователей. Feature `clippy` используется только для unit-проверок и не разрешается для публичного desktop-релиза.

## Протокол

Запрос: `TROPA1 <hex-nonce>:<sample-index>\n`.
Ответ: тот же marker/nonce и необязательный адрес источника через пробел, затем newline. Для внешнего routing-прогона адрес источника обязателен; обычный echo без него проверяет обмен, но не разделение маршрутов.

Fixture использует подключённый UDP socket (ответы только от заданного peer), проверяет nonce и индекс каждого пакета, пропускает старые/неверные ответы до дедлайна. Для TCP установлен общий дедлайн чтения, лимит ответа — 512 байт. CLI не допускает DNS-порт 53, неоднозначные/повторные аргументы, multicast и неограниченные числовые параметры.

JSON включает `transport`, `replyMatched`, `elapsedMs`, `samples`, `matched`, `lost`, `invalidReplies`, `medianMs`, `observedSourceSha256`. Исходный IP и nonce в результат не входят. Пять DNS-запросов или RTT этого echo-сервера не являются измерением качества конкретной игры.

## Локальная проверка инструмента

Команда:

```powershell
cargo test --locked -p clash-verge --bin tropa-routing-fixture --features clippy,portable,tropa-probe --offline --quiet
```

Результат: 4 теста прошли. Проверены отбрасывание чужого/устаревшего nonce, ограничения CLI, обычный UDP с неверными пакетами перед правильными и короткие TCP-соединения. В тестах использованы только loopback sockets, без TUN. Отдельно проверены функция формирования ответов Python для IPv4/IPv6 и отклонение четырёх неверных запросов; сервер не открывался. PowerShell-сценарий прошёл AST syntax parsing; сам сценарий не выполнялся.

Локальные тесты не доказывают Windows process lookup, обход блокировок, работу маршрутов, внешний IPv6 или передачу через WARP.

## Будущий ручной нативный прогон

Нужен доверенный контролируемый внешний сервер с доступным TCP и UDP на одном не-DNS порту. Пример запуска собственного responder на этом сервере (не на машине пользователя):

```text
python tropa-routing-echo.py --host <server-bind-ip> --port 32123 --transport udp
python tropa-routing-echo.py --host <server-bind-ip> --port 32123 --transport tcp
```

Эти команды не выполнялись. Доступ к responder ограничить тестовыми клиентами; он предназначен для стенда, не для постоянного публичного сервиса.

1. Собрать fixture, проверить хэш. В обычном режиме без portable его manifest не должен требовать администратора; перед ручным запуском это отдельно проверить по фактическому PE. Для native-прогона не подменять этим требованием проверку manifest самой Tropa.
2. Выполнить фазу baseline с отключённой Tropa. Скрипт создаст два одинаковых EXE в `.artifacts/validation/routing/<RunId>/selected` и `direct`. Убедиться, что ответы содержат хэши наблюдаемого адреса, а не только `replyMatched=true`.
3. В настройках Tropa выбрать только полный путь selected EXE. Direct с тем же именем не выбирать. Подключить Tropa вручную с подтверждением UAC, после чего выполнить connected тем же fixture/target/RunId.
4. Отключить Tropa вручную и выполнить after. Сравнить source-хэши всех фаз: DIRECT должен остаться на исходном выходе, выбранный процесс должен менять путь только в connected и возвращаться после отключения. Дополнительно проверить правила/outbound по диагностике ядра, а не только IP.
5. Проверить TCP short, UDP burst 100 пакетов, UDP fresh sockets 20, UDP persistent не меньше 5 минут (`-IncludeLongUdp` запускает 301 sample с интервалом 1000 мс). Сравнить сетевые fingerprints baseline/after; изменение внутри фазы исследовать отдельно. Повторить для доступного IPv6 target отдельным RunId.
6. При отсутствии настоящей внешней IPv6-доступности записать NotTested/Unsupported. Loopback IPv6 и IPv4 endpoint Cloudflare не подтверждают внутренний IPv6.
7. Отдельно проверить Firefox Portable / Chrome, системный DNS / browser DoH / QUIC, старт приложения после подключения, недоступный процесс и повторное использование UDP-порта. Снятие ограничений античита не является частью этого инструмента.

Пример ручной команды (PowerShell 7.2+, запускать только когда стенд готов):

```powershell
./scripts/test-tropa-routing.ps1 -Phase baseline -Fixture <own-built-fixture.exe> -Target <controlled-ip:32123> -RunId ipv4-first
```

Для connected/after заменить только Phase. Для IPv6 target использовать `[address]:32123`. Скрипт не перезаписывает существующие отчёты. Он намеренно оставляет `nativeRoutingVerified=false` и `requiresReview=true`: автоматический успех доставки пакетов не заменяет разбор всех фаз и принадлежности маршрута.

## Привязка фаз к одному стенду

При статическом разборе обнаружено, что прежний harness записывал только targetFamily и не требовал baseline перед connected/after. Один RunId мог содержать измерения разных адресов/портов или разный набор долгих UDP-проверок. Это делало последующее сравнение неоднозначным.

Теперь schemaVersion=2 сохраняет SHA-256 канонического IPEndPoint (адрес и порт, без raw target). До копирования/запуска fixture отдельный pure preflight проверяет наличие baseline для connected и baseline+connected для after, совпадение phase/schema, target hash, fixture hash и longUdpTested. Для смены стенда/режима и старых отчётов нужен новый RunId. Внутри одной фазы до/после сетевое состояние по-прежнему собирается без изменения системы.

Команда `./scripts/test-tropa-routing-phase.ps1` выполнила 9 проверок на синтетических JSON: корректная последовательность, пропущенные фазы, несовпадение target/fixture/long-UDP и старый формат. Она dot-sources только pure guard, не запускает основной native harness, fixture, Tropa или сеть. Основной скрипт отдельно прошёл PowerShell AST parsing. Native routingVerified остаётся false: эта привязка предотвращает некорректный стенд, но не доказывает маршрутизацию или очистку.

## Native IPv4 TCP smoke, 2026-10-28

A locally built diagnostic Tropa EXE (`portable,tropa-probe`, SHA-256 `16af09007f2037f835a5a322d643d45465536cc9b4d337302a283b0c90794c51`) ran with elevation after the user authorized UAC testing. The PE manifest was inspected before launch: x64, Windows GUI, requireAdministrator. This diagnostic binary is separate from the ordinary portable release.

Evidence directory: `.artifacts/native-connection/tcp split,2026-10-28-c`. `data/tropa-smoke-result.json` records a successful automatic AWG connection, successful disconnect, and passing before/connected/after checks. The two fixtures were byte-identical copies of Windows curl in different directories; only the exact selected path was routed. Before and after, both reported `warp=off` and the same source-address hash. While connected, selected reported `warp=on` with a different source-address hash; direct remained `warp=off` with its baseline hash. The authenticated verification listener rejected an unauthenticated CONNECT request as required by the smoke check.

The Tropa process (PID 54816) exited. A subsequent process inventory found no Tropa, verge-mihomo or WarpScout processes. The temporary runtime YAML was absent. Independently collected after-network fingerprints matched before-network fingerprints for routes, DNS, adapters, proxy and relevant services. Raw source IPs and registration secrets were not included in this report.

This proves the observed IPv4 HTTPS/TCP split and cleanup for this AWG run. It does not establish WG/MASQUE behavior, external UDP/IPv6 routing, browser routing, long sessions, sleep recovery or anti-cheat compatibility.

## Follow-up protocol runs, 2026-10-28

The same diagnostic EXE was restarted sequentially with `selectionPolicy.allowedTransports` restricted to wireGuard, masqueH3 and masqueH2. Reports were preserved separately as `wg-smoke-result.json`, `h3-smoke-result.json` and `h2-smoke-result.json` in the native evidence directory above. No fallback transport was allowed.

- WG: discovery returned scanner/noCandidates. No connected-phase routing claim is possible.
- MASQUE H3: Tropa reported scanner/processExited. A separate bounded run of the pinned WarpScout using the same exported EC identity reproduced exit code 1; its diagnostic states that 14 MASQUE endpoints were checked and none passed data. This establishes a no-candidate outcome for this scan, not a proven universal network block.
- MASQUE H2: discovery progressed to mihomo validation, but Tropa returned validation/noCandidates. No connected-phase routing claim is possible; the difference between scanner and runtime validation still requires investigation.

All before/after curl checks passed, and the final independent network fingerprints matched the original snapshot for routes, DNS, adapters, proxy and relevant services. No Tropa, WarpScout or verge-mihomo process remained; the temporary runtime YAML was absent. Both saved account files retained their exact pre-WG SHA-256 hashes through all three restarts. The test-only transport restriction was removed after the runs.

The H3 reproduction exposed a diagnostic classification defect: Tropa recognized WarpScout's WG/AWG no-candidate wording but not `no MASQUE endpoint passed data`. The existing scanner classification regression was extended with the observed response and failed with ProcessExited versus expected NoCandidates. The classifier now recognizes both messages while retaining generic handling for unexpected diagnostics. All 466 Rust library tests then passed. The diagnostic EXE used for these native runs predates this classifier fix; no rebuilt EXE or new archive is claimed here.

## MASQUE H2 endpoint comparison, 2026-10-28

Read-only official CLI inspection found WARP disconnected, configured for MASQUE with HTTP/3 and HTTP/2 fallback. The historical service log records successful H2 TLS and connection upgrade at 2026-10-27T16:48:09Z to `162.159.198.2:443`, SNI `consumer-masque.cloudflareclient.com`. No official settings or connection state were changed.

A bounded isolated reproduction used the pinned mihomo binary, the same Tropa EC registration, a loopback mixed proxy and `tun.enable=false`. No elevation or system routing changes were required for this reproduction. The generated proxy matched Tropa's MASQUE H2 options, including enrolled peer verification. Source-address and account values were omitted from the recorded summary.

`h2-matrix-summary.json` in the native evidence directory records:

| Endpoint | HTTP trace | HTTPS trace | HTTPS limited to TLS 1.2 |
| --- | --- | --- | --- |
| 162.159.198.2:443 | curl exit 0, warp=on | curl exit 0, warp=on | curl exit 0, warp=on |
| 162.159.199.38:4443 | curl exit 0, warp=on | curl exit 28, timeout | curl exit 28, timeout |

A preceding reproduction of the two scanner-selected H2 candidates timed out on HTTPS for both. The matrix confirms that H2 is usable through our pinned core and our registration on the standard endpoint; it also demonstrates that an endpoint passing a simpler scanner check may still fail HTTPS. It does not prove full native TUN split routing over H2, UDP/IPv6, or current official-client connectivity.

The pinned WarpScout source samples H2 from two IPv4 /24 pools, while Tropa's initial pass requests sample=1 and only the best exported endpoint for a quick connection. This can omit the working standard address and leaves no alternate from that pass when the exported endpoint fails runtime HTTPS validation. The next search correction must include standard MASQUE endpoints deterministically and preserve runtime-validated alternatives within the transport's budget, while keeping broad search and all user restrictions. No such correction or rebuilt EXE is claimed by this investigation.

Both reproduction processes were terminated and waited for by their owning scripts. A subsequent process inventory found no Tropa, verge-mihomo or WarpScout process, and generated credential-bearing probe configurations had been removed. Local diagnostic logs remain in the isolated secrets directory for investigation.
