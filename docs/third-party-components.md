# Компоненты portable-комплекта Tropa

## Current development state — 28 October 2026

The clean source-built mihomo described below is pinned in development, staged in resources and included in ordinary package l. The new ZIP passed independent app/source/worker verification and extracted-worker path/move checks. Subsequent user-operated ordinary-app runs confirmed one full-computer H2 TUN/tray/Exit cycle and exact-path selected/direct IPv4 HTTP/HTTPS TCP separation, with network restoration and runtime cleanup; the broader network/lifecycle matrix remains open. Package k contains the former core. [Exact package l evidence](validation/portable-release.md#latest-package--28-october-2026-build-l). Standalone proxy comparisons for the clean H2 and AWG profiles returned HTTP/HTTPS `warp=on` and UDP DNS 5/5; neither used system TUN or the Rust coordinator. See `.artifacts/native-connection/clean-core-{h2,awg}-2026-10-28-a/result.json`.

Current production search uses the owned `tropa-probe.exe` worker, not WarpScout CLI. The pinned worker build is `source:54d61ff7ffb7c7aaeb2794448386c7c26385f8ca83c7e48007242201ad946c73`, SHA-256 `3e79dfc254f18b2202912611b204bd24e7b5279b5ffd2a7c1e7d30c981c23851`. Its trusted build record, source snapshot and Go notices remain mandatory packaging inputs. Historical inventories and binary offsets below describe the earlier examined binaries unless explicitly revalidated; do not apply their offsets or dependency counts to a replacement by assumption.

The new core's embedded artifacts were revalidated without loading code. Its Wintun 0.14.1 amd64 DLL exactly matches the pinned official ZIP bytes (427,552 bytes, SHA-256 `e5da8447dc2c320edc0fc52fa01885c103de8c118481f683643cacc3220dafce`, offset 52,211,712). Its EasyTier WASM exactly matches the pinned module artifact (6,850,909 bytes, SHA-256 `8b82f37d62fba2ffe8e256386096fddf448d4261d51c495d5bd97cd11bbb24a0`, offset 52,639,264). Evidence/reproducer: `.artifacts/validation/clean-core-embedded-components-2026-10-28.{json,py}`. The public research inputs now reside under `archive/research/go-license-audit-2026-10-28/`. Exact byte identity does not close the prebuilt DLL redistribution/API conditions or the WASM corresponding-source obligations.

Further WASM provenance review found matching artifact/protobuf declarations of EasyTier source commit `63519db2b5f2a6a1b9b7f20905f036dab54eb829` in the pinned Go module. Its generator requires a clean external EasyTier checkout and `script/build-wasi-core.sh`; the Go module alone is not that Rust source tree. Retrieving this exact commit archive from the documented `EasyTier/EasyTier` upstream returned HTTP 404. This does not establish that the commit is unavailable everywhere: repository mapping and corresponding-source retrieval remain unresolved, and no WASM rebuild is claimed. Evidence: `.artifacts/validation/easytier-source-63519db2-2026-10-28/result.json`.

A follow-up public GitHub API check returned zero indexed commit-search matches for that full source hash and HTTP 422 for its upstream commit endpoint. In contrast, the Go proxy's exact module-origin revision `3d0c9c3ca5e2c0b680383dedf5898f6c7eef9253` resolves successfully and describes the lowercase-import-path change (#2560), with parent `86d942ec8c81a88f7ee0ce5ae4c780c8da78417f`. This narrows the unresolved provenance to the embedded Rust artifact's declared revision rather than the existence of the Go module. Search indexing and these endpoints cannot prove global source unavailability. Evidence: `.artifacts/validation/easytier-source-search-2026-10-28/`; corresponding source and rebuild remain open.

The exact Microsoft WebView2 **Fixed Version** EULA was also retrieved from the official download dialog's API, including its `fixed=true` parameter. Its object-code distribution, downstream terms, notices and SmartScreen conditions are recorded in [the portable runtime review](validation/webview2-portable.md#exact-fixed-version-terms-retrieved). General Edge/Evergreen terms are not the authority for that candidate. Retrieval is complete; implementing and verifying the distribution conditions remains pending.

## Clean mihomo source candidate — 28 October 2026

### Optional no-EasyTier candidate — not adopted

The same clean source revision also supports the upstream `no_easytier` build tag. An isolated offline candidate was built twice with `with_gvisor,no_easytier`, Go 1.26.3, CGO disabled, Windows amd64-v2, readonly modules and fixed version/time flags. All tracked input hashes were compared with the existing source snapshot before/after; `vcs.modified=false` and the expected revision were verified. The two binary hashes match: `1c1d2b31e88e491ba659b8d4212dd14f5d2d31d937f4781177979658ef718e8b`, 50,501,632 bytes, 10,576,384 bytes smaller than the adopted core.

Independent checks found exactly two removed dependency paths (`github.com/easytier/easytier/easytier-go`, `github.com/metacubex/wazero`), no newly introduced dependency paths, absence of the embedded EasyTier WASM, and unchanged official Wintun DLL bytes. Synthetic WG, AWG-I1, MASQUE H3 and MASQUE H2 configurations passed parsing; an EasyTier configuration explicitly failed with the upstream disabled-by-`no_easytier` message. No external protocol connection or system TUN was exercised by these candidate checks.

Evidence: `.artifacts/mihomo-build/clean-v1.19.33-no-easytier-a/{result,independent-check}.json`, repeat binaries/build logs and `synthetic-config-checks/result.json`. Reproducers are `.artifacts/validation/{build,check}-no-easytier-candidate-2026-10-28.py` and the existing synthetic config script's `--output` option. The initial evidence assertion incorrectly assumed that Go sorted the tag string; it was corrected to compare tag sets, then the already completed binaries and source inputs were reverified without rerunning the build.

Tropa does not expose EasyTier, so this is a possible way to exclude an unused component and its unresolved embedded-source obligation from a future package. Package l, resource pins and the live user's core remain unchanged. Adoption still requires native protocol/coordinator/TUN regression checks, a deliberate pin/notices update and a newly bound app/package; the current package's EasyTier provenance gate remains open.

The upstream `v1.19.33` tag was cloned at `3f27ef39f8c96bbfbc8b0b9bb10f5c9ef60149d8`. A separate Windows amd64-v2 candidate was built offline with Go 1.26.3, CGO disabled, `with_gvisor`, `-mod=readonly`, `-buildvcs=true`, `-trimpath`, fixed Version/BuildTime ldflags and an empty build ID. `go mod verify` passed. The source was clean before and after compilation; embedded metadata confirms the pinned commit and `vcs.modified=false`.

Candidate SHA-256: `8ccf696897f4d56aa8f9b13893b6b3c6d24a8a505eb257d46eeac53e6701ed2e`, 61,078,016 bytes. Repeating the same offline build locally to another output filename produced the same hash. This establishes local repeatability with this toolchain/cache, not cross-host reproducibility or equivalence to the existing official EXE. Its 119 actual Go dependency tuples are covered by the existing notice inventory; corresponding-source and other license obligations still require review.

Evidence: `.artifacts/mihomo-build/clean-v1.19.33-b/{build-evidence.json,source.zip,module-verify.log,repeat-result.json}`. The source archive contains hashes of actual tracked input bytes; archive SHA-256 is `43fa54b72f351c9f511c32e3e472129ffdf1e83f68d51b06c6a06f62e32f13cf`. The dependency-download operation added 58 go.sum lines for the full module graph; that generated file was preserved separately under attempt a, then restored to HEAD before the readonly build. No source edits were needed to compile the candidate.

The candidate returned its expected version and accepted four synthetic config-only fixtures: WG, AWG with I1 and no junk, MASQUE H3 and MASQUE H2. Evidence: `synthetic-config-checks/result.json` under the same directory. Those configuration checks used no real identity, external protocol connection or system TUN. Subsequent standalone H2/AWG connection evidence and development adoption are recorded above; current application/TUN acceptance remains open.

The official upstream workflow copies a host CA bundle into `component/ca/ca-certificates.crt` before building. That can produce a modified source marker but does not prove the exact modified inputs of the existing EXE. Its provenance gate remains open until a verified replacement is adopted or its exact inputs are established.

## Owned search target provenance

The Rust target generator in `src-tauri/src/core/warp_targets.rs` adapts endpoint pool and port data from WarpScout revision `9e2ce9570625757ba24b390ad680dccd5e7f2707` (MIT): `pools.go` SHA-256 `c8557247d38b205d0c50115fa2cbec29c8cc45cca16f4baf07d2a1b1df605038`, `warp.go` SHA-256 `c305450ddf66f55d44b9b601ff5b3329a24d21a181f09f2e48d442a5a008083d`, and `masque.go` SHA-256 `ac1134639c126a7fe438a7596b1ed703449149faa8b994e3207548a5b6f77ba3`. The implementation uses bounded deterministic sampling and canonical-address priority rather than upstream's random expansion. Existing WarpScout attribution remains applicable after the executable is removed. This source record does not establish completion of the production cutover or distribution audit.

## Historical portable inventory — before the owned-worker cutover

The table in this section describes the former WarpScout-based package, not the current development inventory. Use the current-state section above, the actual app/worker build records and independent ZIP verification for a new delivery.

Фактический источник версий, SHA-256 и source/release URL для двух поставляемых sidecar EXE: [scripts/tropa-components.json](../scripts/tropa-components.json). Его используют подготовка ресурсов, упаковщик и независимая проверка ZIP. Runtime-проверки Rust дополнительно закрепляют хэши принимаемых исполняемых файлов. Изменение этого inventory не является автоматическим разрешением обновить runtime pins.

| Компонент | Что входит в текущий комплект | Лицензия / статус |
| --- | --- | --- |
| Tropa / Clash Verge Rev | Модифицированный Tauri EXE, встроенный frontend | GPL-3.0; ещё нужен соответствующий snapshot именно наших изменений и проверка транзитивных зависимостей |
| mihomo 1.19.33 | `resources/verge-mihomo.exe` | GPL-3.0; pinned binary hash проверяется, соответствие воспроизводимой сборке исходников пока не доказано |
| WarpScout 0.16.1 | `resources/warpscout.exe` | Корневая MIT; полный Go dependency audit не завершён |
| AmneziaWG / WireGuard / usque / CONNECT-IP / quic-go / Charm | Код внутри Go executables; отдельные приложения не поставляются | Нужно сверить точные версии обоих Go-графов, тексты лицензий и требования notices |
| Wintun 0.14.1 | amd64 DLL побайтно найдена внутри mihomo; отдельного файла DLL в ZIP нет | Официальная Prebuilt Binaries License включена; применение условий к embedded loader ещё проверяется |
| mini_quic_generator | Указан WarpScout как источник ported QUIC Initial builder | В исследованной лицензии есть дополнительные условия; нужна проверка конкретного заимствованного фрагмента и его provenance. Отказ от выбора QUIC в UI не удаляет код из существующего EXE |
| WebView2 Runtime | Не входит в ZIP, требуется установленный runtime | Чистая Windows и возможный Fixed Version комплект ещё не проверены |
| GeoIP databases и CVR service/install/uninstall helpers | Не включаются allowlist-упаковщиком | Наличие этих файлов в `src-tauri/resources` не разрешает включать их в portable ZIP |

Начальные notices: [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md). Они кладутся в комплект вместе с текстами GPL и MIT WarpScout. Это неполный аудит транзитивных лицензий, а не разрешение на публичную раздачу.

Проверенные локальные источники для следующего этапа аудита: закреплённые `go.mod` WarpScout и mihomo, Cargo.lock, pnpm-lock.yaml; лицензии из анализа upstream находятся в `.artifacts/research`. Полные graph/notice manifests ещё не сформированы. В Credits встречается base-relay как fallback регистрации: сам серверный reverse proxy не копируется в комплект; возможность использования endpoint отдельно от заимствования серверного кода не подменяет проверку его условий.

## Предварительные Rust license metadata

Получены offline Cargo metadata с `--filter-platform x86_64-pc-windows-msvc --features portable`. Обход от clash-verge по normal/build edges дал 635 packages; это metadata-кандидаты, не точный список кода в итоговом EXE и не аудит текстов лицензий. Dev-only edges исключены. Полный запрос без target filter требовал отсутствующий Android package и не завершился; Windows-filtered запрос завершился без загрузки и без изменения lockfile. Cargo сообщил о неиспользованном macOS tray-icon patch; обновление зависимостей не выполнялось.

У восьми packages нет license metadata: семь локальных crates Tropa/CVR и tauri-plugin-mihomo. Для локальных crates нужно отразить применимую корневую лицензию в итоговом inventory. У tauri-plugin-mihomo 0.6.0 на commit `7a03d16f4d39b1bb885b7babe66b9e153e34b615` не найдены LICENSE/COPYING/NOTICE в каталоге manifest и двух родителях. Это незакрытая проверка provenance/условий, а не заключение об отсутствии лицензии или нарушении.

Локальные материалы: `.artifacts/validation/tropa-rust-license-metadata-2026-10-28.json` (содержит локальные manifest paths, не для публикации) и `tropa-rust-license-candidates-2026-10-28.json` (сокращённые name/version/license/source/repository). Тексты и notices ещё не собраны; Go/frontend graphs отдельно не заменяются этой Rust-выгрузкой.

## Предварительный frontend inventory

Чтение package.json из локальной pnpm-установки (без выполнения модулей) дало 273 кандидата по dependencies/optionalDependencies/peerDependencies, начиная с production dependencies проекта. Это over-approximation: отсутствуют точное tree-shaking сопоставление с Vite chunks и проверка всех исходников против lockfile. Один optional/peer-кандидат `@mui/material-pigment-css` не разрешился локально; это не объявляется runtime-дефектом без проверки того, нужен ли он бандлу.

У `tauri-plugin-mihomo-api` 0.6.0 нет поля license. В пяти пакетах не найден LICENSE/COPYING/NOTICE на уровне package.json: `@vscode/l10n` 0.0.18, `ahooks` 3.10.0, `client-only` 0.0.1, `server-only` 0.0.1 (у них MIT metadata), `tauri-plugin-mihomo-api` 0.6.0 (metadata отсутствует). Вложенные файлы/репозитории ещё нужно проверить; это не заключение об отсутствии лицензии. Локальный отчёт с версиями, SHA manifest и найденных лицензионных файлов: `.artifacts/validation/tropa-frontend-license-candidates-2026-10-28.json`.

Шрифт `src/assets/fonts/Twemoji.Mozilla.ttf` реально используется Windows CSS/theme и попадает в Vite assets. Его OpenType name table не содержит строк IDs 0/8/13/14 (copyright/manufacturer/license/license URL); рядом с файлом нет notices. Происхождение конкретного font binary и применимые тексты нужно проверить отдельно. Лицензия шрифта не выводится из названия Mozilla/Twemoji или лицензии React.


## Уточнение происхождения и notices, 28 октября

Предыдущая пометка об неизвестном происхождении font binary уточнена фактическими сравнениями. Первоначальный `Twemoji.Mozilla.ttf` в commit `50ba2e3ad4eabdf701e2c87cf511809d78605c28` побайтно совпадает с официальным release asset Mozilla twemoji-colr v0.7.0 (1 474 284 байт, SHA-256 `6d90152ee0d29e82fe2a87793af5aa4b7ad13e6538360889e141e81ed299ee8e`). Тег v0.7.0 указывает на `c0460fdb2654dee584e75c4913bfa9fd2f873338`; LICENSE.md этого тега содержит Apache-2.0 для кода и CC BY 4.0 для artwork.

Текущий файл (1 454 532 байта, SHA-256 `61db6397b8b32b72c585d538ef698a84cf78cf2558a14046b3fa0ec4cc362540`) побайтно совпадает с файлом из последней замены в CVR commit `b10c1d5006081d79c7517dce9fec83627f796d4a`. Все сохранившиеся font tables отличаются от оригинального asset: это модифицированная версия, а не просто удалённый copyright table. Процесс rebuild/конвертации не восстановлен, визуальная идентичность оригиналу не заявляется. Добавлены attribution с указанием изменения, upstream notice и полные Apache-2.0 / CC BY 4.0 тексты в `scripts/licenses`; упаковщик включает их в `licenses`, а verifier проверяет обязательность и pinned hashes. Inventory содержит исходный font hash и provenance, но не извлекает/аутентифицирует шрифт из произвольного Tropa.exe.

Для плагина `tauri-plugin-mihomo` прочитано полное recursive tree закреплённого commit `7a03d16f4d39b1bb885b7babe66b9e153e34b615` (truncated=false) и README. Явных LICENSE/COPYING/NOTICE файлов и указания лицензии в README не найдено. Rust/npm packages относятся к тому же commit. Проверка остаётся открытой; лицензию CVR нельзя автоматически приписать отдельному repository. Обращений к авторам не отправляли.

Из байтов закреплённых Go EXE извлечены inline build-info строки без вызова Windows loader или кода executable. WarpScout сообщает Go 1.26.3, root v0.16.1, revision `9e2ce9570625757ba24b390ad680dccd5e7f2707`, vcs.modified=false, GOAMD64=v1 и 40 dependency entries. Mihomo сообщает Go 1.26.10, revision `3f27ef39f8c96bbfbc8b0b9bb10f5c9ef60149d8` (совпадает с pinned source tag v1.19.33), vcs.modified=true, GOAMD64=v2 и 119 dependency entries; replacement protobuf-go сохранён отдельно от заменяемого модуля. Это self-reported metadata, связанная с проверенным SHA EXE, а не доказательство аутентичности исходников/полноты notices.

Текущий комплект требует x86-64-v2 CPU из-за mihomo; это указано в README. Метка modified=true не раскрывает полный diff. Pinned build workflow перед сборкой копирует обновлённый CA bundle в исходное дерево; это возможное объяснение изменённого дерева, но не доказательство точного набора изменений именно поставляемого binary. Проверку соответствия исходников и воспроизводимости не закрываем.

Артефакты чтения находятся в `.artifacts/research/license-provenance-2026-10-28`: complete plugin tree, font license/tag/release responses, font comparison и raw Go build-info. Структурированные модули обеих сборок сохранены в `.artifacts/validation/tropa-go-build-dependencies-2026-10-28.json`. Тексты лицензий всех этих Go-зависимостей, Rust/frontend транзитивные notices, Wintun и mini_quic_generator ещё не проверены полностью. Новый inventory не даёт разрешения на публичный выпуск.


## Go modules и встроенные артефакты: уточнение аудита

Обработаны все 157 уникальных пар module/version из embedded metadata двух EXE (40 и 119 entries, две пары общие). Для protobuf используется фактический replacement, исходная запись не выдаётся за поставляемый код. Из Go proxy получены ZIP ровно этих версий; Go `h1` пересчитан по отсортированным именам и SHA-256 файлов и совпал с каждым ожидаемым checksum. На первом проходе ограничения размера не позволили проверить два больших ZIP; после проверки конкретных размеров и увеличения ограничений до 128 MiB ZIP/128 MiB total uncompressed/64 MiB member оба проверены. В том числе был прочитан как байты checkpointgofer.bin в gvisor; ничего из архивов не запускали.

Из 156 модулей скопированы 192 файла с именами LICENSE/COPYING/NOTICE/COPYRIGHT (включая вложенные notices). У `github.com/mattn/go-localereader` v0.0.1 отдельного текста нет, но pinned README сообщает MIT и автора Yasuhiro Matsumoto; эта декларация сохранена отдельно. Добавлен BSD license Go из тегов go1.26.3/go1.26.10, тексты совпадают. Полный сборник `scripts/licenses/Go-modules.txt` содержит 677 186 байт, машиночитаемые источники/версии/h1/notice hashes — в `scripts/tropa-go-module-inventory.json`. Проверено включение всех 192 исходных byte sequences без редактирования. Сборник избыточен относительно фактически слинкованных файлов: это не точный file-level SBOM и не завершённая правовая проверка.

### Wintun

H1-проверенный sing-tun v0.4.28 содержит `internal/wintun/amd64/wintun.dll`, README указывает 0.14.1. Официальный архив с wintun.net имеет SHA-256 `07c256185d6ee3652e09fa55c0b673e2624b565e02c4b9091c79ca7d2f24ef51`; его amd64 DLL (427 552 байта, SHA-256 `e5da8447dc2c320edc0fc52fa01885c103de8c118481f683643cacc3220dafce`) побайтно найдена в pinned mihomo на file offset 52 405 184. Сопоставление сделано чтением байтов, без Windows loader, распаковки/запуска DLL из EXE или драйверных действий.

Этот DLL лицензируется отдельной Prebuilt Binaries License, а Go wrapper содержит MIT/SPDX notice. Официальный текст сохранён без изменения байтов в `scripts/licenses/Wintun-0.14.1.txt` и включается в ZIP. Условия 3(a)–3(d) затрагивают модификации и распространение с использованием Permitted API; loader sing-tun загружает DLL из памяти через свой memmod. Само byte-match не подтверждает соблюдения этих условий. Это вопрос совместимости поставки, который остаётся открытым; наличие текста лицензии не заменяет проверку требований.

### EasyTier WASM и copyleft obligations

`github.com/easytier/easytier/easytier-go` включает 6 850 909-байт `internal/artifact/easytier_core.wasm`, SHA-256 `8b82f37d62fba2ffe8e256386096fddf448d4261d51c495d5bd97cd11bbb24a0`. Та же последовательность байтов найдена в mihomo на offset 52 832 736. Модуль имеет LGPL-3.0 LICENSE, README описывает встроенный Rust WASM. Его Rust source/dependency graph и воспроизводимость артефакта Go module graph не доказывает.

В Go-сборнике присутствуют GPL, LGPL и MPL тексты, включая LICENSE.gpl и README с GPLv2 у go-lzo. Код go-lzo в исследованных .go не уточняет `or later`; лицензию автоматически не повышаем до GPLv3 и совместимость не объявляем. Для LGPL/MPL нужны собственные проверки соответствующего исходного кода и применимых условий. Source-header notices и встроенные артефакты требуют отдельной проверки сверх поиска по именам файлов.

### Evidence и поставка

Архивы читаются только как данные; источник исследования — `.artifacts/research/go-license-audit-2026-10-28/`. `inventory.json` содержит все 157 результатов без ошибок; `wintun-embedded-comparison.json` и `easytier-wasm-comparison.json` фиксируют сопоставления. Текущий ZIP allowlist расширен до 14 файлов; все пять дополнительных notices проверяются по trusted SHA inventory. Пройдено 16 Python tests (9 packaging, 4 PE, 3 source) и Node syntax check. Реальный новый архив не создавали, historical ZIP не меняли. PublicReleaseReady остаётся false; полные Rust/frontend/native/source/signing gates не закрыты.

### Уточнение вложенных frontend notices

Без исполнения JS повторно прочитаны собственные деревья установленных @vscode/l10n 0.0.18, ahooks 3.10.0, client-only 0.0.1 и server-only 0.0.1. Найденный ahooks/dist/ahooks.js.LICENSE.txt содержит только attribution screenfull v5.2.0 и js-cookie v3.0.5; это не собственный полный MIT-текст ahooks и не закрытие пробела. В трёх остальных деревьях файлов LICENSE/COPYING/NOTICE не найдено. Поля MIT в package.json сохраняются как декларации, а не заменяют отсутствующие тексты. Evidence с версиями, относительными именами и хэшами: .artifacts/validation/frontend-nested-notice-review-2026-10-28.json. Это уточнение inventory, не разрешение публичного распространения.

### Коллекция frontend notices по проверенным локальным хэшам

273 установленных package.json сопоставлены с ранее записанными manifest SHA-256. Для 271 license/notice файла из inventory проверены SHA-256; тексты собраны без изменения bytes с заголовками package/version/path. Независимый повторный разбор сборника подтвердил все заголовки, размеры, 271 payload hash и отсутствие хвоста. Сборник `.artifacts/validation/frontend-license-collection-2026-10-28/Frontend-candidates.txt`: 376 867 байт, SHA-256 `0fadc264e911ffd22a6f95bda4510c7b2aa57d70493322db6b535d8b598df6ea`. Рядом inventory.json с hashes и оставшимися пробелами. Это 273 production/optional/peer кандидата, а не точный Vite SBOM; installed hashes пока не сверены с registry tarball/lock integrity. Сборник пока не включён в поставку.

Для ahooks 3.10.0 найден собственный MIT LICENSE в commit `11f6ad571bd365c95ecb9409ca3050cbbfc9b34a`: SHA-256 `b25a3f524da1b0d44e49edfb02695a690c72449e37aab9f85631ec63f1307376`, 1 063 байта. gitHead совпал в установленном manifest и метаданных npm registry. Исходный текст и origin.json сохранены в `.artifacts/research/frontend-license-audit-2026-10-28/`. Это обоснованный источник недостающего текста; точное соответствие всего кода npm артефакта commit ещё не доказано. Также сохранены version metadata @vscode/l10n, client-only, server-only: у этих трёх gitHead не опубликован, поэтому точную исходную ревизию по ним не выводим. Никакой загруженный код не выполнялся.

### npm integrity и отсутствующие тексты: четыре пакета

Для @vscode/l10n 0.0.18, ahooks 3.10.0, client-only 0.0.1 и server-only 0.0.1 скачаны version-specific npm tarballs. SHA-512 каждого совпал одновременно с version metadata registry и pnpm-lock.yaml. Все 521 файла (6 + 509 + 3 + 3) побайтно совпали с установленными пакетами, без выполнения/распаковки в рабочее дерево. Сохранены архивы, registry metadata и file-level verified.json в `.artifacts/research/frontend-license-audit-2026-10-28/`. Registry signatures не проверялись; integrity подтверждает соответствие локальному lockfile, не независимую подпись.

У Microsoft/vscode-l10n найден тег `l10n/v0.0.18` → commit `fc7e3d79ddb91a2cc24a9730aad912026a43dbf2`. JSON l10n/package.json этой ревизии полностью совпал с установленным manifest. Собственный MIT LICENSE (Microsoft Corporation), 1 141 байт, SHA-256 `c2cfccb812fe482101a8f04597dfc5a9991a6b2748266c47ac91b6a5aae15383`, сохранён с origin metadata. Это источник недостающего notice; JS build из исходников не воспроизводился. Точное происхождение client-only/server-only и отсутствие лицензии plugin остаются открыты. Эти исследовательские файлы не добавлены в ранее созданный ZIP.

### Rust notices: фактический release compilation graph

Из cargo.jsonl сборки Tropa-tuning-release-2026-10-28-b извлечены 615 package IDs: 602 registry, 5 git и 8 local/path. Это использованные Cargo artifacts, включая host/build/proc-macro, а не точный linked SBOM. Для всех 602 registry .crate SHA-256 совпал с Cargo.lock; Cargo.toml и 1 037 найденных LICENSE/LICENCE/COPYING/COPYRIGHT/NOTICE файлов побайтно совпали с installed source. Коллекция 5 183 265 байт, SHA-256 `ecb9d928c7594252c86e7a2b2a6ad181f5c9381695a0aee32ff19c0748113983`; независимый повторный разбор подтвердил каждый payload hash, заголовок и отсутствие остатка. Evidence: `.artifacts/validation/rust-release-notice-audit-2026-10-28/`.

У 33 registry пакетов отдельного notice не найдено. Для 32 с repository проверен .cargo_vcs_info.json внутри crate, затем прочитаны 19 точных GitHub revision trees. Получены ещё 24 license/notice файла на уровне корня/родительских каталогов; данные и URL/hash metadata сохранены в `.artifacts/research/rust-missing-notices-2026-10-28/`. Их применимость к каждому подкаталогу и встроенным компонентам пока не подтверждена. У selectors, terminfo и dark-light таких файлов в проверенной ревизии нет. У terminfo полный WTFPL v2 текст присутствует в комментарии src/lib.rs; поиск только по именам его пропускал. README dark-light явно объявляет Apache-2.0 или MIT, но ссылки на файлы в этой ревизии не ведут к присутствующим файлам. Deelevate не публикует repository в manifest; источник не угадываем.

Git/local packages, source-header notices, registry source files сверх manifest/notices, соответствующие исходники и embedded-data условия ещё требуют проверки. Коллекция пока не включена в ZIP; автоматическое добавление корневых upstream текстов не объявляется лицензионным разрешением. Загруженный код не выполнялся.
