# Tropa: review of changes after 12bc14e7

## Latest browser acceptance — ordinary package l

Firefox 159 was added through the actual GUI picker; the saved selected path is the internal Firefox Portable EXE. User-reported nonce-bearing trace pages show Firefox off → on (HTTP/3) → off, while Chrome 156 remains off/HTTP2. Independent system curl stayed direct; baseline/after routes, adapters, DNS and proxy exactly matched, registrations persisted, and temporary core configuration was removed. [Evidence/source and limits](production-connection.md#ordinary-package-l--firefox-selected--chrome-direct). Earlier pending-browser/picker statements below are now historical for this basic scenario. DoH, arbitrary UDP, IPv6, persistent sockets and other browser/process combinations remain unproved. HTTP/3 in Firefox does not validate MASQUE H3.

The separately built existing `--tropa-smoke --tropa-split-smoke` hook subsequently passed one initial cycle plus 20 consecutive production-path H2 TUN cycles. Independent acceptance rechecked selected/direct HTTPS and exit-hash return, exact baseline network restoration, registration preservation, runtime cleanup and process exit after every cycle. Batch cycle durations were 6.57–7.01 seconds. [Evidence and limits](production-connection.md#repeated-clean-core-h2-lifecycle--21-diagnostic-cycles). This diagnostic is not shipped in ordinary package l and does not represent twenty manual GUI cycles. The 60-minute session, sleep/network changes and faults remain open.

## Latest native acceptance — ordinary package l

User-operated ordinary-app testing now confirms one full-computer H2 connection, continued traffic after close-to-tray and clean tray Exit. A separate fixture with the same release binaries confirms selected/direct separation for new IPv4 HTTP/HTTPS TCP connections using byte-identical `curl.exe` files with the same name in different paths. Only the selected copy changed exit IP and reported WARP on; both unselected copies stayed direct. Explicit Disconnect restored all three to direct and left the app open. In both runs routes/adapters/DNS/proxy returned exactly to baseline, registrations were preserved, core/worker stopped and temporary secret runtime files were removed. The earlier user app remained untouched.

[Exact scope and evidence](production-connection.md). The fixture used staged preferences, not the GUI EXE picker. Browser/DoH/QUIC, UDP/IPv6, long-lived sockets, 20-cycle/60-minute resilience and clean Windows remain open. Prior no-ordinary-TUN statements below are superseded for these two narrowly identified runs; the cancelled diagnostic remains a cancelled diagnostic.

Portable runtime preparation also progressed: official x64 Fixed WebView2 156.0.4314.50 was downloaded and statically inspected; CAB and primary binaries have valid Microsoft signatures. It remains outside package l, unexecuted and uninstalled, with redistribution/configuration and clean-Windows acceptance still open. [Runtime candidate evidence](webview2-portable.md).

## Latest delivery — package l

The ordinary clean-core release compiled in 10m 24s and package `Tropa-owned-search-2026-10-28-l.zip` passed independent verification against external application/source/worker evidence: 16 files, `development-test`, `publicReleaseReady=false`. ZIP SHA-256 `aae789ccfdee09a4d10bed592f81c7da4d5c9b8ee001f1658e1c4e3d4da578f0`, 55,039,657 bytes. Its actual extracted worker passed source-ID-checked hello/shutdown before and after a Cyrillic/space-containing folder move, exit 0, unchanged files. [Full delivery evidence and limits](portable-release.md#latest-package--28-october-2026-build-l). Build/packaging is no longer pending; native ordinary GUI/TUN/tray and the complete goal remain unaccepted. The UAC cancellation below did not run an elevated diagnostic.

The twelve-requirement matrix is now updated in [the roadmap](../tropa-status-and-roadmap-2026-10-28.md#7-соответствие-исходной-цели). New core embedded-component comparisons are recorded in the component audit. EasyTier artifact/protobuf source declarations agree on `63519db2b5f2a6a1b9b7f20905f036dab54eb829`, but retrieval from the documented upstream returned HTTP 404; corresponding source and WASM reproducibility remain unresolved. Earlier build-in-progress and package-k-only statements below describe the state before this continuation.

## Read this first — current review, 28 October 2026

This section supersedes the status statements below. This pass reviewed the dirty tree against `12bc14e7`, the search/discovery changes, routing and close handling, package contracts, archive references and existing native evidence. No application source was changed and no network/UAC/GUI test was launched in this pass.

Fresh checks: four targeted frontend test files passed **29 tests**; `pnpm typecheck` exited 0; Python `test_tropa_*.py` passed **39 tests**. Knip exited 1 and still reports **125 unused files**, plus unused dependencies/exports. These are investigation leads, not a safe deletion list: externally invoked scripts can appear unused. `git diff --check` also reports six added blank lines at EOF. A subsequent post-pin Rust library run passed **480 tests, 2 ignored**, with exit 0; log: `.artifacts/validation/clean-core-library-tests-2026-10-28.txt`.

Continuation: the isolated clean-core coordinator diagnostic was refused without elevation. Its UAC launch was then cancelled by the user, so no elevated diagnostic process or network acceptance ran; no repeat prompt was created. The normal source-bound portable build `Tropa-owned-search-2026-10-28-l` was started separately. A live compilation is not a finished archive or runtime pass.

### Findings and priorities

1. **Current-source acceptance and delivery are incomplete.** The development pin/resource now uses clean source-built mihomo SHA `8ccf696897f4d56aa8f9b13893b6b3c6d24a8a505eb257d46eeac53e6701ed2e`. Package k still contains the former core. Existing clean-core H2 and AWG comparisons confirm HTTP/HTTPS `warp=on` and five successful UDP DNS samples for each core, with process exit, temporary-runtime removal and unchanged registrations. Their reports explicitly set `systemTunTested=false` and `rustCoordinatorTested=false`; they do not establish current application or split-routing acceptance. Complete the current coordinator/failure checks, ordinary build and exact-artifact runtime acceptance before offering a current verified package.
2. **Archive preservation is local, not fully versioned.** `archive/` is ignored; `.github/legacy-workflows/` is untracked. Tracked originals are recoverable from Git, but these copies will not accompany a checkout unless deliberately preserved. Do not add the whole archive: it contains private runtime material. `prebuild.mjs` still references removed `scripts/set_dns.sh` and `scripts/unset_dns.sh` in macOS-only tasks. This is a residual legacy-path inconsistency, not a demonstrated failure of the Windows Tropa build. Decide which legacy workflows/build paths remain supported before further removal.
3. **Routing target identity is implicit.** `RoutingPreferences::rules` identifies directories from current filesystem metadata or a non-EXE extension; the frontend labels them only by the extension. An existing directory named `Something.exe` can be accepted as a directory but labelled as an application, and if it later disappears the backend can interpret it as an exact EXE path. Persist an explicit file/folder discriminator with compatibility for existing saved paths. Folder rules cover executable location, not child-process ancestry or external services; the current explanatory text correctly states that limitation.
4. **Live selection is a reconnect, without rollback.** `select_tropa_server` persists the new choice before the frontend calls disconnect/connect. If the new exact profile fails, the former connection is not restored. This preserves exact manual intent, but the product needs explicit failure/retry behaviour and tests for repeated selection, disconnect failure and exit while switching. Do not claim the old connection is restored or silently choose another endpoint.
5. **The roadmap contains historical contradictions.** Later sections still describe the old CLI dependency, synchronous close callback and obsolete notices as open after subsequent fixes. Retain evidence history but use one current requirement matrix for decisions; otherwise completed implementation work will be repeated and acceptance gaps overlooked.

### Implemented versus still required

Implemented in source: owned structured Rust/Go search without production WarpScout CLI/report scraping; protocol/family scheduling and resumable expansion; fresh runtime validation of saved candidates; automatic/exact manual choice and geography policy; expanded server table with concrete parameters and distinct measurement methods; folder routing; configurable asynchronous close handling with tray default; mandatory app/source/worker binding and independent package verification. These implementation statements are not blanket runtime acceptance.

Next order: (1) current clean-core coordinator and exact-manual/failure acceptance, including early runtime rejection followed by successful resumed expansion; (2) actual ordinary EXE/TUN/tray tests; (3) selected/direct TCP, browser DoH/QUIC, controlled external UDP and separate IPv6 checks; (4) 20 connection cycles, 60-minute session, sleep/network-change/crash cleanup; (5) clean Windows/WebView2 and moved-folder acceptance, remaining license/source/signature obligations; (6) separate game measurements; (7) source-bound final ZIP, independent verification and scoped cleanup. Missing external UDP infrastructure and an IPv6-capable test environment must remain explicit evidence gaps.

Avoid adding new transport/UI features before these checks. The immediate improvements are explicit routing-target types, clear reconnect failure feedback and a consolidated status matrix; a rewrite of the working search or another packaging layer is not indicated by this review.

## Latest state — package k and clean core candidate

Ordinary package k compiled and passed independent app/source/worker verification; it includes the asynchronous close-action fix. Its extracted worker also passed hello/shutdown before and after a folder move with Cyrillic/spaces. [Package hashes and limits](portable-release.md#latest-package--28-october-2026-build-k). GUI/TUN/tray acceptance remains open.

A separate clean upstream mihomo candidate now has the pinned source commit, an exact source snapshot, `vcs.modified=false`, verified modules and equal SHA-256 across two local offline builds. All 119 embedded dependency tuples are covered by the notice inventory; four synthetic WG/AWG/H3/H2 configs passed `-t`. It is not part of package k and still needs real connection acceptance before replacing the existing pinned core. [Evidence and provenance limits](../third-party-components.md#clean-mihomo-source-candidate--28-october-2026). This progress does not close the default shipped core's provenance or the full license audit.

## Changes after package j — asynchronous window-close preferences

The current source no longer reads/parses YAML synchronously inside the window callback. Windows portable close is prevented immediately; a task reads preferences through `TropaPreferences::load` under the existing config-write serialization, releases that lock, then dispatches hide or the existing quit path. Exit state is checked both before the read and before dispatch. Non-portable close behavior is preserved. Missing/unreadable preferences retain the tray default.

The dispatcher now receives the chosen action explicitly. Its existing tray test is independent of user settings; an added persistence/dispatch regression checks saved Quit, saved MinimizeToTray and shutdown reentry using an isolated preferences file. Disabling the Quit branch caused the intended behavioral failure (hidden count 1 instead of 0); restoring it passed. Fresh full library result: **480 passed, 2 ignored**. Ordinary portable `cargo check` exited 0 with existing dead-code warnings. Logs: `.artifacts/validation/portable-close-{red,green,mutation,full,ordinary-check}-2026-10-28.txt`. The initial red compilation also confirmed the missing explicit-action API; the subsequent mutation demonstrates actual behavioral sensitivity.

Package `j` predates this change and is not a package of the current source. No ordinary EXE/tray/TUN acceptance is claimed for this fix. Two new Computer Use initialization attempts returned `trusted Node process exited unexpectedly; kernel reset, rerun your request` before returning a window. No GUI input occurred; the existing user Tropa process was not stopped. The next GUI/native acceptance still needs a working UI helper or user-operated observations.

## Latest delivery evidence — build j

The new ordinary release compiled successfully and `.artifacts/portable/Tropa-owned-search-2026-10-28-j.zip` passed the independent app/source/worker verifier (16 files, development-test, publicReleaseReady=false). Root notices now describe the actual owned worker and preserve upstream attributions. The packaged worker passed hello/shutdown before and after moving a Cyrillic/space-containing extracted directory; no registration or TUN was used. App and worker remain unsigned. Full hashes, build identity and evidence paths are recorded in [portable-release.md](portable-release.md#latest-package--28-october-2026-build-j). Earlier statements below that a current package or corrected root notices are missing are historical; GUI/TUN and full goal acceptance remain open.

## Current review summary — 28 October, 20:20 local

This section supersedes open/closed status statements in the historical review below. The current pass inspected the working-tree diff, scheduler/runner, public measurements and sorting, folder-rule generation, window-close handling, packaging inventory, notices and archive paths. It did not rerun the previously recorded test suites or perform GUI/TUN acceptance.

- Owned search, policy-aware early handoff, bounded saved-profile admission, resumable expansion, precise measurement DTOs and method-separated sorting are implemented. Mandatory app/source/worker binding and the dynamic worker inventory are also implemented. Earlier statements that these are missing are historical.
- Native proxy evidence confirms H2 and mixed search ending in AWG on this network. WG/H3 validation timeouts remain network-specific observations. These results do not establish current ordinary-EXE TUN routing, arbitrary UDP, IPv6 or browser isolation.
- `archive/` is ignored. Legacy page/script/doc copies are local preservation, not tracked renames. The `.github/legacy-workflows/` directory is currently untracked too; describing it as already tracked is premature. Previously tracked originals remain recoverable from Git, but never-tracked research files require separate preservation. Do not add the entire archive: it contains private runtime material.
- Supported consumers of `service-release.mjs`, `updatelog.mjs`, `Makefile.toml` and `knip.json` have their files restored. The remaining Knip failure needs scoped investigation; it is not evidence that all reported files can safely be removed.
- Folder/file identity is still inferred from the current filesystem and `.exe` extension. Persist an explicit selection type before relying on portable path migration or missing directories. Folder matching covers executable location, not parent/child process relationships or separately installed anti-cheat services.
- Window close still reads/parses `tropa.yaml` synchronously inside the callback, bypassing the normal config read boundary. Replace this with config-owned state when addressing lifecycle behavior. Verify tray default, explicit quit, cleanup failure and exit during switching in the ordinary application.
- Live server selection performs disconnect/reconnect. Failure of the new exact profile does not implement restoration of the former connection. Keep the failure visible; any rollback behavior needs an explicit product decision rather than silent substitution.
- Root `THIRD_PARTY_NOTICES.md` still describes shipping WarpScout 0.16.1 and its old compiled dependency graph. This must be reconciled with the owned worker before the next package. Existing notice coverage is not completed source/provenance/license clearance; mihomo modified-source identity and plugin terms remain open.
- Release build `Tropa-owned-search-2026-10-28-h` finished successfully during this review after 11m 07s. `build.json` records x64, Windows GUI, `requireAdministrator`, SHA-256 `9a6bb7e57525c65792bc150b6502d78cdaf2ab0873ab0e4ae0890c0a6f852ef8` and `executed=false`. This is compilation/source evidence, not runtime acceptance. After correcting root notices, capture a new source-bound build record and independently verify the resulting ZIP.

Recommended order: finish and verify the current package; reproduce early-runtime-failure → resumed-search → success and exact-manual failure; validate ordinary GUI/TUN/tray cleanup; close TCP/UDP/IPv6/browser and lifecycle acceptance; verify clean-Windows portable requirements and component provenance. Separate game measurements follow the network foundation. Additional unrelated features and blanket archive cleanup should wait.

Дата: 28 октября 2026 года. Проверено незакоммиченное рабочее дерево относительно `12bc14e7`. Анализ не является разрешением публичного выпуска. В рамках проверки исходники приложения не изменялись; добавлен только этот отчёт. Сетевое подключение TUN, игры, античиты и изменения настроек роутера не запускались.

## Обновление после исправлений

- **Проверен реальный worker при остановившемся читателе stdout.** После полного hello с совпавшим source-bound build ID отправлен поток loopback-only WG-заданий с синтетической identity; stdout намеренно не читался. Worker самостоятельно завершился с exit 1 за 0,042 с без kill, writer получил BrokenPipeError; до заполнения канала прочитаны 2 accepted и 26 error events, stderr содержит только `tropa_probe_failed`. Хвост последнего frame неполон, поэтому этот вывод нельзя считать пригодным каталогом результатов. Evidence: `.artifacts/native-connection/worker-slow-reader-2026-10-28-b/result.json`; воспроизводящий script — `.artifacts/validation/worker-slow-reader-2026-10-28.py`. Это проверка отказа Go-host при backpressure; Rust-owner cleanup, системный TUN и пользовательские регистрации не затрагивались. Первый эксперимент сохранил отдельный неуспешный результат: перегрузка началась до полного hello, и проверка handshake справедливо не прошла.
- **Корневые notices приведены к собственному worker.** WarpScout EXE больше не указан как поставляемый компонент; MIT-атрибуции WarpScout/Android сохранены, прямые Go-зависимости и отсутствие QUIC Initial generator описаны отдельно от поддержки MASQUE H3. Свежая сверка: все 21 Path/Version/Sum tuple worker присутствуют в notice inventory. Полнота лицензионных обязательств этим не доказана. Все 39 Python build/package tests прошли. После исправления notices запущена новая source-bound ordinary release-сборка `j`; первоначальная попытка `i` не запустила Cargo из-за отсутствия локального toolchain в PATH и не имеет завершённого build record.

После первоначального анализа внесены адресные исправления. Описанные ниже находки сохраняют контекст исходного обзора; их актуальный статус:

- **Worker inventory привязан к доверенной сборке.** Генератор комплекта добавляет собственный worker с SHA-256, build ID, source snapshot ID, версией протокола и ссылками на bundled notices. Verifier заново строит ожидаемую инвентаризацию из проверенного engine build record. Packaging-регрессия сначала обнаружила отсутствующую запись; после исправления все 39 Python-тестов прошли, включая отклонение подменённых build/source IDs при пересчитанном manifest. Старые низкоуровневые fixtures с явными pins не являются обычным CLI-путём поставки.
- **Ошибка записи каталога проверена с реальным worker.** В изолированном synthetic-account fixture вместо `warp-profiles.json` создан каталог. Production stream/runner возвращает исходную IO-ошибку, не заменяя её отменой; worker подтверждённо завершён, sidecar reservation снова доступна, регистрация побайтно прежняя. Pinned native engine suite: 27 passed. Это проверка границы worker → persistence → cleanup, не полный TUN или сценарий ранний mihomo-отказ → успешное расширение.
- **Повторная попытка GUI-доступа не состоялась.** Инициализация `@oai/sky` и отдельный `cua.getApp("Tropa")` завершились до получения окна: kernel stderr `windows sandbox failed: helper_unknown_error: setup refresh had errors`. Действий с GUI, UAC, треем или TUN не было. Пользовательский старый `Tropa` PID 67668 оставлен без вмешательства. Release-компиляция `h` запущена отдельно; перед упаковкой нужно исправить старое описание WarpScout EXE в корневых notices и заново записать актуальные source evidence.
- **Подготовка ресурсов исправлена.** Ошибка `ENOENT` при чтении `.artifacts/warpscout-source/LICENSE` воспроизведена. Теперь `prepare-tropa.mjs` берёт полные MIT notices из отслеживаемого `engine/THIRD_PARTY_NOTICES.md`, без зависимости от ignored archive. Подготовка ресурсов завершилась успешно. Остальные ссылки на перенесённые build scripts, hooks и workflows ещё открыты.
- **Начальная очередь сохранённых профилей исправлена.** Регрессия с 40 медленными профилями на семейство адресов сначала подтвердила вытеснение стандартного endpoint. Теперь в начальном проходе допускается один сохранённый профиль на транспорт/семейство, затем стандартные потоки; остальные сохранённые варианты чередуются в расширении. Регрессия проходит для IPv4 и IPv4+IPv6. Это проверка планировщика, не измерение скорости внешней сети.
- **Условие раннего завершения исправлено.** HTTPS-кандидат разрешает ранний выход только при допустимом транспорте и соответствии географической policy. Проверены исключённый узел, несовпадение страны узла и выхода, неизвестная страна, запрещённый транспорт и неуспешный HTTPS. Возобновление расширенного поиска после отказа финальной mihomo-проверки всё ещё открыто; отдельный сценарий initial → expansion также не подтверждён этой регрессией.
- **Корни дисков и trailing separators исправлены.** Воспроизведено несовпадение `C:\` с EXE внутри диска. Генератор нормализует завершающие разделители директории; регрессия проверяет корень, вложенный EXE и соседний каталог. Подсказка интерфейса теперь объясняет расположение EXE вместо обещания охватить все дочерние процессы. Явный тип folder/file в настройках пока не добавлен.
- **Обход app build evidence закрыт.** Упаковщик и независимый CLI verifier теперь требуют app build record/source snapshot и engine build. Проверяется EXE из самого ZIP, соответствие source/frontend, PE/release features, worker pin и embedded localBuild внешней записи. Регрессия сначала воспроизвела принятие архива без app evidence. Сквозная fixture теперь использует настоящий snapshot/record и синтетический PE; подмена приложения с пересчитанными manifest/localBuild отклоняется. 17 packaging tests и все 39 Python tests прошли. Доверие к локальной записи не заменяет подпись; отдельная worker-строка в component inventory пока открыта.
- **Активные tooling references восстановлены.** `service-release.mjs`, `updatelog.mjs`, `Makefile.toml` и `knip.json` возвращены из проверенных архивных копий: у них остались активные потребители. Их tracked bytes снова совпадают с базой. Модули импортированы и проверены на локальных Cargo.toml/Changelog без загрузок и публикации; Node syntax checks потребителей прошли. Старые release/autobuild/Telegram workflows перенесены в отслеживаемый `.github/legacy-workflows`, за пределы исполняемого каталога Actions. README больше не ссылается на удалённые локальные переводы/screenshots. Hook-команды не запускались и cargo-make не устанавливался.
- **Knip gate остаётся красным.** После восстановления конфигурации реальный `pnpm knip:check` завершился с exit 1: 125 unused files и дополнительные замечания. Вывод сохранён локально в `.artifacts/validation/tooling-2026-10-28/knip.txt`. Устранение оборванных ссылок не закрывает эту отдельную уборку; suppressions для получения зелёного результата не добавлялись.
- **Текущая native proxy-цепочка подтверждена.** После новой компиляции diagnostic EXE и UAC выполнены H2 first search (5,603 с), свежая cached revalidation (2,661 с) и mixed WG/AWG/H3/H2 search (34,713 с, итоговый AWG). HTTPS WARP и UDP DNS прошли, каталог появился до завершения, регистрации не изменились, worker/mihomo и временный runtime после выхода отсутствуют. [Подробные результаты и пределы доказательства](production-connection.md#owned-search-native-acceptance--2026-10-28). Системный TUN, fault scenarios и текущий обычный ZIP этим не закрыты.
- **Новый ordinary ZIP собран и статически проверен.** `Tropa-owned-search-2026-10-28-g.zip`: 16 файлов, mandatory app/source/worker evidence, обычные release features без diagnostic/clippy, x64 GUI/admin manifest, без WarpScout EXE и личных данных. [Хэши и пределы проверки](portable-release.md#текущий-ordinary-zip--28-октября-2026). Native GUI/TUN этого EXE ещё не подтверждены; подписи нет. Не устранён устаревший текст README комплекта о моменте переключения сервера.
- **Индивидуальные WG/H3 прогоны завершены.** Оба дошли до validation timeout в пределах примерно 179 с. В сохранённых WG-наблюдениях есть handshake/HTTP=true (36 из 60), но HTTPS=true нет; H3 не сохранил успешных handshake/HTTP/HTTPS среди 14 измерений. Это результаты текущей сети, не универсальный запрет протоколов. Оба прогона сохранили регистрацию и подтвердили очистку. Произвольный внешний UDP этим не проверен.
- **Возобновление после ранней проверки добавлено в исходники.** После завершения initial phase runner возвращает оставшийся `SearchSchedule`, а не уничтожает очередь. Сохраняются endpoints, точные параметры, spent budgets и исходный discovery deadline. Первая mihomo-проверка ограничена 20 с и сроком поиска; при отказе worker продолжает expansion после освобождения runtime ownership. Уже проверенные в этой операции профили повторно не валидируются. Если discovery budget закончился во время первой проверки, оставшиеся кандидаты получают финальный validation reserve без нового worker. Ручной отказ, отмена и cleanup errors по-прежнему не разрешают подмену или конфликтующий запуск. Свежие результаты: Rust **478 passed, 2 ignored**, engine с native worker **27 passed**, ordinary portable `cargo check` — exit 0 с dead-code warnings. Native-тест подтверждает продолжение без повторного initial pass и очистку при отмене; полный fault-сценарий ранний runtime-отказ → новый рабочий кандидат ещё не воспроизведён. ZIP `g` собран до этих изменений и их не содержит.

**Измерения доведены до публичного DTO и таблицы.** В исходниках передаются точные дробные SearchMeasurement, nullable stages и параметры WG/AWG/MASQUE. Таблица показывает метод/цель, выборку, разброс и этапы предварительного поиска без повышения статуса до verified. Новые mihomo-наблюдения сохраняют типизированный UDP DNS method; старые записи без него остаются неизвестными. Сортировка учитывает статус и возраст/контекст, разделяет UDP DNS и ICMP и не ранжирует legacy/неподдерживаемый метод по его числам. Регрессии сначала воспроизвели неверный порядок, отсутствие публичной точности и runtime method. После исправления: Rust **479 passed, 2 ignored**, native engine **27 passed**, обычный portable check — exit 0 с существующими dead-code warnings; TypeScript и scoped ESLint — exit 0, пять frontend-файлов — **31 passed**. README будущего комплекта уточняет переподключение, папки и настройку крестика. ZIP `g` предшествует и этому изменению; новая GUI/TUN-проверка остаётся открытой.

Промежуточные проверки предыдущих исправлений: Rust library suite — **476 passed, 2 ignored**; TypeScript — exit 0; routing Vitest — **7 passed**; Python `test_tropa_*.py` — **36 passed**. Они не включали TUN, браузеры, игры, античиты или изменение системных сетевых настроек. Результаты web build и ZIP ниже относятся к исходному обзору; позднейший комплект `g` описан выше и не содержит последующих изменений исходников.

Дополнительно engine suite с закреплённым native worker и `--include-ignored --test-threads=1` — **26 passed, 0 ignored**. Этот набор пересекается с Rust library suite и проверяет границы worker, а не полную TUN-маршрутизацию.

## Что изменилось

- Удалён `warp_scout.rs`, старые CLI presets/arguments и scanner account export helpers. В проверенных production-путях `src-tauri/src` и `scripts` больше нет запуска `warpscout.exe` или прежних export helpers; тесты и атрибуции рассматриваются отдельно.
- `build-tropa.py`, `package-tropa.py` и CLI verifier требуют `--engine-build`. Запись сборки приложения получает worker pin; упаковщик сравнивает его при предоставленном app build record. Добавлены регрессии отсутствующего worker и несовпадения pin.
- Сохранённые точные профили добавляются в начало worker-очереди. `advanced` передаётся в планировщик, ограничивает широкий junk/SNI-подбор. Для обычного подключения введён ранний выход после начального прохода и HTTPS-кандидата.
- Поиск передаёт страну выходного IP в строки таблицы отдельно от страны узла; статус предварительного результата остаётся `untested`.
- Добавлены папки в раздельную маршрутизацию, настройка действия крестика, кнопка выхода и начальное меню трея.
- Выбор строки при активном подключении теперь сохраняет профиль, отключает текущую сессию и запускает новое подключение. Раньше выбор применялся при следующем подключении.
- Убраны старые страницы Clash Verge и предварительная загрузка home page. Таблица получила кликабельные строки, подсветку задержек и сворачиваемые результаты по протоколам.

## Проверенные результаты

| Проверка | Результат в этом анализе |
| --- | --- |
| `cargo test --locked -p clash-verge --lib --features clippy,portable,tropa-probe --offline --quiet` | 473 passed, 2 ignored |
| Тот же crate, фильтр `core::warp_engine`, `--include-ignored --test-threads=1`, pin из `source-2026-10-28-scheduled-a` | 24 passed, 0 ignored; пересекается с library suite |
| `cargo check --locked --offline -p clash-verge --features portable --quiet` | Exit 0; есть dead-code warnings |
| `pnpm.cmd typecheck` | Exit 0 |
| Vitest: services/tropa, utils/tropa-server, components/tropa/{server-picker,tropa-routing,registration-settings} | 5 files, 28 passed |
| `pnpm.cmd run web:build` | Exit 0; production frontend собран |
| Python unittest discover, `test_tropa_*.py` | 36 passed |
| `verify-tropa-package.py` для `Tropa-owned-search-2026-10-28-c.zip` с доверенным engine build | 16 files, development-test, publicReleaseReady=false |

Уменьшение общего числа Rust-тестов относительно прошлого отчёта связано с удалением старых scanner-тестов; сравнивать только счётчик как меру регрессии нельзя. Успешные native engine tests проверяют собственный worker и его границы, а не полную маршрутизацию Windows.

## Замечания по приоритетам

### 1. P1 — перенос оставил активные ссылки на отсутствующие файлы

`scripts/prepare-tropa.mjs:24` продолжает копировать `.artifacts/warpscout-source/LICENSE`; путь отсутствует после переноса исследования в `archive/research`. Подготовка ресурсов завершится ошибкой чтения исходного файла. Сам скрипт при анализе не запускался, поскольку он перезаписывает ресурсы.

Дополнительно:

- `scripts/prebuild.mjs:13` импортирует отсутствующий `./service-release.mjs`.
- `scripts/updater.mjs:3` и `updater-fixed-webview2.mjs` импортируют отсутствующий `updatelog.mjs`.
- Git hooks по-прежнему вызывают `cargo make`, но `Makefile.toml` перенесён.
- Активные upstream workflows вызывают перенесённые `extract_update_logs.sh` и `telegram.mjs`.
- README ссылается на перенесённые переводы и screenshots; `knip:check` остался в CI, а его явная конфигурация удалена.

Нужно определить поддерживаемые команды Tropa, исправить их ссылки и зависимости, а obsolete upstream workflows явно вывести из эксплуатации. Не подменять это возвращением production-зависимости от архивной исследовательской папки. Используемую лицензию следует брать из сохраняемого исходного файла проекта.

### 2. P1 — длинная очередь кэша может вытеснить стандартные проверки

`warp_engine/scheduler.rs:181–209` вставляет каждый сохранённый профиль отдельным initial job перед всеми стандартными потоками. Количество таких jobs не ограничено и не чередуется по протоколам. Они расходуют тот же `per_transport` budget, что и базовые адреса.

При многочисленных медленных сохранённых профилях бюджет транспорта может закончиться до его стандартного endpoint. При большом кэше нескольких транспортов можно израсходовать общий discovery deadline на сохранённые варианты. Это вывод по admission/budget logic, а не результат внешнего сетевого воспроизведения.

Нужно ограничить начальную порцию кэша, чередовать её по транспортам/семействам и резервировать базовый проход. Нужна регрессия с большим кэшем и длительностью jobs, а не только одним мгновенным saved H2 profile.

### 3. P2 — ранний выход считает кандидата до применения policy

`warp_engine/runner.rs:297–298` устанавливает `has_https_candidate` по любому HTTPS=true. География проверяется позже, в `stream.rs:155`, где кандидат может быть отброшен. Поэтому результат исключённого узла/страны может разрешить ранний выход из обычного поиска, хотя допустимого HTTPS-кандидата нет.

Нужно учитывать policy-допустимость при условии раннего выхода и предусмотреть поведение при последующей неудаче свежей mihomo-проверки. Регрессия: успешный исключённый кандидат в initial phase и допустимый кандидат только в expansion.

### 4. P2 — source binding упаковки приложения остаётся необязательным

В `package-tropa.py:28–42` app build record/source snapshot по-прежнему optional. Без них пропускается проверка соответствия worker pin тому, с которым собран `Tropa.exe`. Независимый ZIP verifier проверяет worker, компонентные хэши и manifest, но не доказывает происхождение произвольного app binary. Existing fixture даже упаковывает не-PE bytes без app record; это допустимо для fixture, но не доказывает строгость обычного production pipeline.

Для обычной поставки нужны обязательные app build evidence и проверка совместимости app/worker; экспериментальный режим, если нужен, должен быть явно отдельным. Worker также стоит внести в итоговую компонентную инвентаризацию с build/source ID. Текущий shipped inventory перечисляет в components только mihomo.

Проверено отдельно: все версии модулей из `engine/dependencies.json` присутствуют в старой Go notice inventory. Утверждать, что тексты лицензий всех новых Go-библиотек отсутствуют, оснований нет. Обновить нужно фактическое соответствие компонентов, происхождение и полноту обязательств, а не механически дублировать notices.

### 5. P2 — папка не равна группе дочерних процессов

`config/tropa.rs:167` создаёт PROCESS-PATH-REGEX для исполняемых файлов внутри выбранной директории. Процесс из другой папки не попадёт под правило, даже если его запустила игра. Отдельно установленная anti-cheat service также не охватывается автоматически.

Подсказка `routing-picker.tsx:110` должна объяснять именно расположение EXE, а не обещать покрытие всех дочерних процессов. Тип выбора лучше хранить явно, вместо вывода из `.exe` и текущего `path.is_dir()`.

Есть отдельный граничный случай: выбор корня диска. `C:\` уже оканчивается separator, а генератор добавляет ещё `[\\/]`; обычный путь `C:\Game\game.exe` не совпадёт с ожидающим второй separator regex. Нужна нормализация trailing separator и проверка root/subfolder/sibling.

### Дополнительные замечания

- Переключение строки при connected — полное переподключение, а не бесшовное переключение. Текущий текст «мгновенного переключения» этого не объясняет. При отказе нового точного профиля возврат к старому не реализован; автоматическая скрытая подмена также не нужна. Нужно явно показывать этап и конечное disconnected/failed состояние, проверить повторные клики и выход во время переключения.
- Для нового `CloseAction::Quit` нет отдельной явной проверки поведения. Существующий close test покрывает default tray hide; нужно проверить сохранение настройки, повторный запуск, выход при операции и отказ очистки. Чтение YAML синхронно прямо из window callback стоит заменить чтением состояния из владельца config либо чистым решением по переданному значению.
- Public server DTO всё ещё не передаёт точные search f64, measurement method/target, samples/stages и параметры профиля. Новая проверка ranking не заменяет эту часть Task 2. Основная таблица по-прежнему использует округлённые старые поля.
- Удаление старых страниц не устранило все тяжёлые зависимости: в текущем frontend bundle остались Monaco и его workers. Оптимизацию делать после определения используемых экранов и измерения выигрыша; не удалять зависимости вслепую.
- Старые roadmap/validation docs описывают уже отсутствующий WarpScout и отсутствие ZIP. Их текущие статусы нужно обновить, сохранив исторические результаты как историю.

## Что произошло с архивом

Перенесённые legacy pages, docs, scripts, Makefile, knip и renovate найдены в `archive/frontend-legacy` и `archive/legacy-clash-verge`. После нормализации CRLF/LF их содержимое совпадает с `12bc14e7`; screenshots совпадают побайтно. Удалённый scanner module не найден как архивная копия, но сохранён в истории Git.

В `.gitignore:23` добавлено `archive/`. Это локальный архив, а не tracked rename: следующий коммит зафиксирует удаления, но не включит перенесённые копии. Исторические tracked-файлы можно восстановить из Git; локальные исследовательские файлы, которые никогда не были tracked, такой гарантии не имеют. Архив исследований также содержит локальные регистрации, поэтому добавлять весь archive принудительно нельзя.

Рекомендуемое разделение: личные данные/сборки оставить локально; необходимые чистые исходники и provenance хранить в отслеживаемой структуре либо в явно сохраняемом источнике. На ignored archive не должны зависеть поддерживаемые build/runtime команды.

## Фактическое состояние ZIP

`Tropa-owned-search-2026-10-28-c.zip` существует и прошёл независимую проверку. В нём есть owned worker и нет `warpscout.exe`, указан source-bound worker ID. Это реальное продвижение относительно предыдущего обзора.

Но у snapshot из `.artifacts/validation/Tropa-owned-search-2026-10-28-c/source.zip` сейчас 29 путей отсутствуют или отличаются по bytes. В том числе изменены routing, close action, tray, live switching и удалены legacy pages. Такой архив не доказывает поставку текущего рабочего дерева. В более поздних локальных каталогах сборок обнаружены source/cargo outputs без итогового `build.json`; они сами по себе не являются завершённой source-bound сборкой.

## Остаток ближайшего плана

| Task из search-cutover-completion | Состояние после анализа |
| --- | --- |
| 1. Saved ordering, modes, early handoff | Fairness, policy-aware early exit и продолжение реализованы; полный native отказ → продолжение → успех ещё открыт |
| 2. Measurement semantics and UI | Точные DTO, методы, параметры, этапы и разделённая сортировка реализованы; проверка интерфейса обычного актуального EXE ещё нужна |
| 3. Mandatory worker build/package | Resource preparation и обязательный app/source/worker binding исправлены; отдельная worker-строка inventory ещё нужна |
| 4. Remove CLI and export paths | Основной source cutover сделан; production CLI/helper references не найдены в проверенных путях |
| 5. Native full-pipeline acceptance | Worker boundary tests подтверждены; текущая полная Rust → worker → mihomo → TUN цепочка и её fault scenarios не доказаны |
| 6. Current portable package | Есть проверенный ZIP предыдущего снимка; для последних изменений нужен новый комплект и native smoke именно из него |

## Порядок дальнейшей работы

1. Исправить поддерживаемые build/prepare/hooks ссылки после архивации; определить, что хранится в Git, а что только локально.
2. Исправить резервирование базовых проверок и policy-aware early exit; проверить большой кэш и неудачу final validation.
3. Уточнить поведение live switching, folder routing и close action; дополнить минимальными поведенческими регрессиями этих новых границ.
4. Довести публичные measurement fields и app/worker provenance; обновить актуальные статусы документации.
5. Собрать новый обычный source-bound ZIP из текущего дерева и пройти полный native поиск/подключение/отмена/tray/выход.
6. Закрыть оставшиеся требования основной цели: TCP/UDP/IPv6/браузеры, 20 циклов и 60 минут, сон/смена сети, чистая Windows/WebView2, лицензии/исходники/подписи.
7. После сетевой основы реализовать отдельные игровые измерения и проверяемые группы процессов; затем финальный архив, матрица всех 12 требований и адресная уборка.

Внешний UDP responder, доступная IPv6-сеть, чистая Windows и signing-процесс остаются отдельными недостающими возможностями для соответствующих доказательств. Отсутствие стенда не запрещает независимую разработку и не делает соответствующий тест пройденным.
