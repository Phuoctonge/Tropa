# Portable release: текущая проверка поставки

## Current UI-01 test package — 29 October 2026

`Tropa-ui-01-2026-10-29.zip` supersedes package l for the interface. It contains compact server rows, fixed comparison controls, disclosure details, separate active/saved states and the corrected saved-choice badge. The user confirmed six rows in the browser preview at approximately 1360 x 750 and supplied a wider screenshot with nine complete rows; the exact new ordinary EXE subsequently passed user-operated native readability/badge review. Light theme, text scaling and complete keyboard navigation remain unchecked.

App SHA-256: `2f4339b096d745809c585ed35ba00a5ebc2408ebc1ade05bbda0c02761f7f0d9`; x64 Windows GUI, `requireAdministrator`, Authenticode `NotSigned`. ZIP: 55,040,007 bytes, SHA-256 `2a5a978c2c24d68038a1b1747a0f6272db28df4a84e4d440b142bb9e6a88918c`. External source/build/engine verification passed with 16 files, `development-test`, `publicReleaseReady=false`; no private data or diagnostic executable is packaged. Build evidence: `.artifacts/validation/Tropa-ui-01-review-2026-10-28-c/`. Native UI evidence: `.artifacts/native-connection/ui-01-review-2026-10-29-a/`. No tunnel was connected in the new UI fixture; network acceptance still refers to package l. IPv6 production correction was deferred by the user and is absent from this ZIP.

Package l separately passed the hour-long H2 selected-IPv4-HTTPS session and exact post-disconnect cleanup; see the current production-connection summary. That closes the hour-session gate for this measured case only.

A separate current diagnostic executable with package-l core/worker pins passed one initial and 20 consecutive H2 selected-IPv4-HTTPS production-path lifecycle cycles, including exact network restoration and process/runtime cleanup after each. [Evidence and limits](production-connection.md#repeated-clean-core-h2-lifecycle--21-diagnostic-cycles). It is not included in the ordinary ZIP; its populated private fixture must not be distributed. This does not establish 60-minute stability, sleep/network changes, other protocols or clean-Windows portability.

The user-operated Firefox-selected / Chrome-direct scenario subsequently passed with the package-l binaries and actual GUI picker. Reported trace pages show Firefox off → on (HTTP/3) → off while Chrome stays off, with an independent direct system control and clean network/runtime restoration. [Scope and source](production-connection.md#ordinary-package-l--firefox-selected--chrome-direct). This closes the basic browser scenario, not DoH, arbitrary UDP, IPv6 or persistent browser sockets.

Subsequent user-operated acceptance of package l confirmed one actual full-computer H2 connection, continued traffic after close-to-tray and clean tray Exit. HTTP/HTTPS changed off → on → on → off; core/TUN exited and routes/adapters/DNS/proxy returned exactly to baseline. Registrations were preserved and temporary secret runtime files removed. [Actual scope and evidence](production-connection.md#ordinary-package-l--successful-user-operated-system-tun-cycle). Later claims below that package l has no ordinary TUN/tray evidence are historical. Split routing, other protocols, arbitrary UDP/IPv6, resilience and clean Windows remain open.

A subsequent separate same-binary fixture also passed [exact-path split routing for new IPv4 HTTP/HTTPS TCP connections](production-connection.md#ordinary-package-l--exact-path-split-routing-passed), including identical EXE names/bytes in different directories and cleanup after explicit Disconnect. Browser/DoH/QUIC, UDP, IPv6, long-lived sockets, folder/child/service rules and the GUI picker remain outside that acceptance.

Autonomous WebView2 startup remains open. The pinned Tauri already supports an executable-relative Fixed Version runtime; the outstanding work is a pinned official runtime, redistribution review, package/configuration integration and clean-Windows acceptance. [Concrete delivery constraints and required tests](webview2-portable.md) include Windows 10 AppContainer directory permissions, local-path/UNC limits and runtime update ownership. No runtime or system permission was changed during this investigation.

## Latest package — 28 October 2026, build l

`Tropa-owned-search-2026-10-28-l.zip` is the latest ordinary local development package and supersedes k. It contains the clean source-built mihomo SHA-256 `8ccf696897f4d56aa8f9b13893b6b3c6d24a8a505eb257d46eeac53e6701ed2e` and the source-bound owned worker; no WarpScout executable, diagnostic executable or user registration is included. Release compilation completed in 10m 24s. App SHA-256: `65e56d678fd29b72c208c1668531c584b13162418e6c788af14d777fb4ea39c6`; x64 Windows GUI, `requireAdministrator`, Authenticode `NotSigned`.

ZIP: 55,039,657 bytes; SHA-256 `aae789ccfdee09a4d10bed592f81c7da4d5c9b8ee001f1658e1c4e3d4da578f0`. Independent verification using external app build/source/engine records passed with 16 files, `development-test`, `publicReleaseReady=false`. Build record and source archive: `.artifacts/validation/Tropa-owned-search-2026-10-28-l/{build.json,source.zip,cargo.jsonl}`. The source snapshot ID is `83e9bd55fece5a818b8c71655d597d7ef7137106ae9a02b6882dcdf2ea83083b`.

The worker actually extracted from this ZIP passed hello/shutdown before and after moving the directory with Cyrillic/spaces: 0.524s / 0.426s, exit 0, expected source build ID, empty stderr, unchanged package files. Evidence: `.artifacts/native-connection/portable-path-2026-10-28-l/result.json`. This is not ordinary app GUI, registration, TUN/tray or clean-Windows acceptance. The separate clean-core coordinator UAC launch was cancelled; it produced no elevated test. Existing standalone-core H2/AWG results do not substitute for these remaining gates. Licenses/corresponding sources, WebView2/clean Windows, signatures and full network/lifecycle acceptance remain open.

## Latest package — 28 October 2026, build k

`Tropa-owned-search-2026-10-28-k.zip` supersedes package j and includes the asynchronous close-action fix. Release compilation finished in 10m 39s; app SHA-256 `af09d4f9d1fe1aa9cfcca30b70815f530557bd77d97598bd302e1a5a4382c6ef`, x64 Windows GUI, `requireAdministrator`. ZIP size is 55,153,967 bytes; SHA-256 `70aee84861146a96565f7c8b1f57486b3ad2c9a1dfce84e8b1c1e5825ad3b1c0`. Independent verification with external app/source/engine records passed: 16 files, development-test, publicReleaseReady=false. App Authenticode status is NotSigned.

The actual worker from this ZIP passed source-ID-checked hello/shutdown under Cyrillic/space-containing paths before and after moving the extracted folder: 0.477s and 0.426s, exit 0, empty stderr and unchanged package files. Evidence: `.artifacts/native-connection/portable-path-2026-10-28-k/result.json`; app evidence: `.artifacts/validation/Tropa-owned-search-2026-10-28-k/{build.json,source.zip,cargo.jsonl}`. This did not register a device, connect a tunnel or exercise the ordinary GUI/tray. The clean mihomo candidate documented in third-party-components is separate and is not included in this ZIP.

## Latest package — 28 October 2026, build j

`Tropa-owned-search-2026-10-28-j.zip` is the latest ordinary source-bound development package, superseding earlier package statuses below. The archive is 55,149,225 bytes, SHA-256 `09fe6b42d04958585105650555b5d4bb57db7ba26ff2efa3dae5070e1ea758df`. Independent `verify-tropa-package.py` completed with 16 files, `development-test`, `publicReleaseReady=false`, using the external app build record/source snapshot and pinned engine build `source-2026-10-28-scheduled-a`.

The release compilation completed in 10m 32s. App SHA-256 is `45448ec57c172f35286bffc7dd6d66ee91384670fe13dd32b9f23de1b5486c2a`; PE inspection records x64, Windows GUI and `requireAdministrator`. Source evidence is `.artifacts/validation/Tropa-owned-search-2026-10-28-j/{build.json,source.zip,cargo.jsonl}`. This package includes the resumable search, precise measurements, dynamic worker inventory and corrected root notices. App and worker Authenticode status is `NotSigned`.

After verification, the ZIP was extracted under a directory containing Cyrillic and spaces. Its actual worker completed source-ID-checked hello/shutdown in 0.554s. After moving the entire extracted directory, it completed the same check in 0.428s. Both returned exit 0 with empty stderr; all extracted files remained unchanged. No registration, probe request or TUN was used. Evidence: `.artifacts/native-connection/portable-path-2026-10-28-j/result.json`; reproducer: `.artifacts/validation/portable-worker-path-smoke.py`.

This establishes worker launch/exit from the transferred package, not ordinary app GUI/TUN acceptance or clean-Windows portability. WebView2, routing/lifecycle acceptance, signatures and remaining source/license gates remain open. Earlier no-execution statements below describe earlier stages.

Общий release gate остаётся открытым. Здесь проверены упаковщик и содержимое ZIP; запуск Tropa, UAC, TUN, скачанных VPN/scanner executables не выполнялся.

## Текущий ordinary ZIP — 28 октября 2026

Собран `.artifacts/portable/Tropa-owned-search-2026-10-28-g.zip` (55 144 065 байт,
SHA-256 `91095223a75c7efa91d47a17ac7e375c6b12ef996e04bcaa5ce51acd14d4225d`).
Упаковщик и отдельный CLI verifier завершились с exit 0: 16 файлов,
`development-test`, `publicReleaseReady=false`. WarpScout EXE, диагностический
EXE и личные данные не входят в allowlist и отсутствуют в комплекте.

App build evidence: `.artifacts/validation/Tropa-owned-search-2026-10-28-g/{build.json,source.zip}`;
snapshot ID `1a0a594ab2f16576f2214b58c3d024ade951d97bba7472fe56347b8584e532e2`.
EXE SHA-256 `a46ccc58325f4be89b28c43a36041d187d60ca8840e722f33a3a577717820cfb`;
features — `custom-protocol,default,portable`, без `tropa-probe`/`clippy`.
PE — x64 GUI, `requireAdministrator`; подпись — `NotSigned`.
Worker повторно проверен до компиляции, его pin совпал с app record и bytes ZIP.

Frontend production build прошёл. Native proxy-проверки текущего исходного
пути описаны в [production connection](production-connection.md), но они
использовали отдельный diagnostic EXE. Сам ordinary EXE из этого ZIP ещё не
прошёл TUN/GUI/tray smoke. Две попытки инициализации Computer Use завершились
ошибкой `trusted Node process exited unexpectedly; kernel reset` до управления
окном. Это ограничение UI-проверки, а не подтверждённый отказ приложения.

README внутри ZIP ещё описывает применение выбранного сервера при следующем
подключении; текущий интерфейс умеет переподключаться сразу. Этот текст нужно
согласовать с окончательным поведением live switching в следующей сборке.

После сборки ZIP `g` в исходники добавлено продолжение оставшейся очереди после
раннего отказа runtime validation. Эти изменения прошли Rust/native/check
проверки, но в `g` не входят. Архив сохраняет свой зафиксированный snapshot;
новую поставку необходимо собирать с новой записью сборки, а не менять EXE в
существующем ZIP.

## Проверяемый состав

В корневой папке архива ожидаются ровно шестнадцать файлов: `Tropa.exe`, `README.txt`, `build-info.json`, `component-inventory.json`, `THIRD_PARTY_NOTICES.md`, `licenses/GPL-3.0.txt`, `licenses/WarpScout.txt`, `resources/verge-mihomo.exe`, `resources/tropa-probe.exe`, `licenses/Twemoji-Mozilla.txt`, `licenses/Apache-2.0.txt`, `licenses/CC-BY-4.0.txt`, `licenses/Wintun-0.14.1.txt`, `licenses/Go-modules.txt`, `licenses/Frontend-candidates.txt`, `licenses/Rust-release-candidates.txt`. Это состав текущего тестового комплекта, не запрет на добавление необходимых DLL в будущем; такое изменение требует явного обновления inventory и проверок. WarpScout EXE в комплекте запрещён; его MIT attribution сохраняется.

`scripts/package-tropa.py` копирует только перечисленные источники, вычисляет хэши всех payload-файлов и после записи ZIP запускает проверку. `scripts/verify-tropa-package.py` проверяет архив отдельно, без выполнения содержимого: единый корень, точный состав, отсутствие дублей без учёта регистра, traversal/backslash/ADS/symlink, лимиты размера, покрытие манифестом, фактические хэши и trusted pins EXE и notices. Встроенный inventory должен совпадать с inventory инструмента проверки. Изменённый EXE не допускается только на основании изменённого build-info.

Упаковщик и отдельный CLI verifier требуют внешние `build.json`, `source.zip` и каталог engine build. Verifier сверяет текущие выбранные исходники и frontend с записью сборки, проверяет хэш и PE самого `Tropa.exe` из ZIP, release features и worker pin, затем сравнивает embedded `localBuild` с внешней записью. Проверяемый EXE временно копируется в отдельный каталог и удаляется после статической проверки; выполнения нет. Эта локальная привязка не является цифровой подписью, полной фиксацией corresponding source или доказательством безопасности программы. Поле publicReleaseReady остаётся false.

Команды из корня проекта:

```powershell
python -m unittest discover -s scripts -p test_tropa_bundle.py
python scripts/package-tropa.py --binary target/release/clash-verge.exe --name Tropa-local-test --build-record .artifacts/validation/Tropa-local-test/build.json --source-snapshot .artifacts/validation/Tropa-local-test/source.zip --engine-build .artifacts/engine-build/source-2026-10-28-scheduled-a
python scripts/verify-tropa-package.py .artifacts/portable/Tropa-local-test.zip --build-record .artifacts/validation/Tropa-local-test/build.json --source-snapshot .artifacts/validation/Tropa-local-test/source.zip --engine-build .artifacts/engine-build/source-2026-10-28-scheduled-a
node --check scripts/prepare-tropa.mjs
```

Упаковка не собирает и не запускает EXE. Существующий архив не перезаписывается. CLI упаковщика по умолчанию использует release-путь; это не команда публичного выпуска. Внешние records и snapshots должны соответствовать текущим исходникам и `dist`; историческая сборка после изменения входных файлов отклоняется. Старые архивы автоматически не перепаковываются.

## Фактические тесты

17 packaging unittest проходят. Включены отсутствие app evidence, подмена приложения/worker с пересчитанным манифестом, несовпадение app/worker pin, лишние личные данные и прежние проверки состава/notices/путей. Сквозная fixture использует минимальный синтетический PE, настоящий source snapshot и build record; только проверка сборки Go-worker заменена fixture-ответом. В исходной папке есть синтетическая регистрация; она не попадает в ZIP. Проверяется реальная логика упаковщика, PE inspector и отдельного верификатора; бинарники не выполняются. Эти проверки не доказывают работу реального приложения.

## До выпуска

### Контролируемая привязка локальной сборки

`scripts/build-tropa.py --name <unique-name> --engine-build <verified-engine-build>` компилирует только portable release bin через offline Cargo и сохраняет выбранные исходники **до** компиляции. Нужен заранее собранный `dist/index.html`; Vite нельзя запускать одновременно с Cargo. Команда не запускает EXE, UAC, TUN или sidecars. Текущие `CARGO_HOME`, `RUSTUP_HOME` и PATH должны указывать на подготовленные локальные toolchains.

После exit 0 проверяются Cargo compiler-artifact (bin, opt_level=3, test=false, точные features custom-protocol/default/portable), неизменность набора/байтов выбранных исходников и файлов dist, затем PE/manifest. Только после этих проверок создаётся `.artifacts/validation/<name>/build.json`. При ошибке остаются snapshot/log для диагностики, но успешного build.json нет. Папка evidence не перезаписывается.

Упаковщик требует пару `--build-record <build.json> --source-snapshot <source.zip>` вместе с `--engine-build`. Он повторно сверяет текущие source/dist/EXE, проверяет хэш скопированного EXE и включает summary в build-info.json без абсолютных путей. Обхода без записи сборки в обычном CLI больше нет. Низкоуровневые тесты состава могут передать явные доверенные hashes через Python API; CLI этот путь не предоставляет.

Это локальная запись контролируемого процесса, которую владелец файлов может изменить. Она не является подписью, независимой аттестацией, полным corresponding source или доказательством воспроизводимости. Внешние зависимости, toolchain и все environment flags не зафиксированы полностью; publicReleaseReady=false. Статический ZIP verifier сравнивает localBuild с предоставленной внешней записью, но доверие к самой записи остаётся внешним условием. Исторический release EXE не получает такую запись задним числом.

Остаются полноценный license/notice audit, соответствующий source snapshot рабочего дерева, release build без probe/clippy, статическая независимая проверка его PE и manifest, подпись конкретных файлов, WebView2/чистая Windows, перенос/обновление папки и разрешённые нативные проверки подключения/очистки. Упаковочный тест не закрывает эти пункты и не создаёт готовый дистрибутив приложения.

## История предыдущих поставок

Записи ниже относятся к прежним этапам и составам ZIP. Текущее рабочее дерево они не подтверждают.

Статический PE inspector и снимок выбранных рабочих исходников добавлены отдельным этапом: [протокол проверки](source-and-pe-2026-10-28.md). Нативный запуск они не выполняют; существующий debug EXE прошёл PE/manifest проверку.

Release-компиляция завершилась успешно; создан новый локальный release-profile тестовый ZIP `Tropa-release-local-2026-10-28.zip` с девятью файлами. Source inputs, features, PE и EXE внутри ZIP проверены; подписи нет. Полные хэши и границы evidence: [результат release-попытки](source-and-pe-2026-10-28.md#результат-release-попытки). Это ещё не публичный выпуск и не проверка native runtime.


## Обновление inventory: шрифт и статические Go metadata

В текущий обязательный состав добавлены три font notices; packaging tests проходят (8 тестов упаковки, 15 вместе с PE/source tests). Origin шрифта установлен через побайтное совпадение первоначального Git blob с Mozilla v0.7.0 и текущего файла с последней модификацией CVR. [Доказательства и ограничения](../third-party-components.md#уточнение-происхождения-и-notices-28-октября).

Текущий закреплённый mihomo сообщает GOAMD64=v2. README комплекта и корневой README теперь указывают это требование CPU. Embedded vcs.modified=true не выдаётся за точное соответствие немодифицированному исходному тегу; native запуск не выполнялся. Старый девятифайловый ZIP сохранён как исторический и не соответствует новому trusted inventory; автоматически его не перепаковывали. Текущий код упаковщика не доказывает, что старый EXE был собран из новых исходников.


## Обновление: notices Go и Wintun

В текущие обязательные 14 файлов входят Go-module collection и официальная лицензия встроенного Wintun 0.14.1. Тест сначала воспроизвёл успешную проверку архива без этих notices, затем после обновления inventory оба пропуска отклоняются. Пройдено 16 Python tests и Node syntax check. [Побайтные сопоставления DLL/WASM и незакрытые условия распространения](../third-party-components.md#go-modules-и-встроенные-артефакты-уточнение-аудита). Старый ZIP без этих notices не соответствует текущему trusted inventory; его не перепаковывали. Новый состав проверен сквозной fixture, а не нативным запуском приложения.

## Frontend notices в следующем составе

В trusted inventory добавлен обязательный Frontend-candidates.txt (379 991 байт, SHA-256 7bb78cbaf4767c0b586b8cca4dcabfac13ef4d7914d0529684c174a0edf06605). Сохранены 271 исходный payload и два source supplements без изменения bytes; список версий и hashes — scripts/tropa-frontend-notice-inventory.json. Regression сначала воспроизвела отсутствие файла в CONTENT_FILES, затем проверила отклонение отсутствующего и подменённого notice даже с пересчитанным manifest. Прошли все 23 Python tests, включая сквозную упаковочную fixture без выполнения EXE. Предыдущий 14-file ZIP Tropa-tuning-release-2026-10-28-b сохранён; он не соответствует новому точному inventory и не является текущей 15-file поставкой. Новый EXE/ZIP на этом шаге не собирались. Полнота лицензий и native gates не подтверждены.

## Rust notices в следующем составе

Добавлен обязательный Rust-release-candidates.txt: 5 184 529 байт, SHA-256 f20cff5d6098235f702a9ac41ac4442a999d3861bff9e46cf626475a00c8e6bd. Коллекция сохраняет все 1 037 исходных payloads и terminfo source-header notice. Machine inventory — scripts/tropa-rust-notice-inventory.json; local path IDs удалены. Регрессия сначала показала отсутствие файла в CONTENT_FILES, затем подтвердила отклонение пропуска/подмены с пересчитанным manifest. Прошли все 24 Python tests, включая сквозную fixture; source bytes сохранены отдельно. Текущий состав — 16 файлов; новый release EXE/ZIP на этом шаге не создавались, прежние архивы не менялись. License/source/native gates остаются открыты.
