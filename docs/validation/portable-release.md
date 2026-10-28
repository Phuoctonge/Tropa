# Portable release: текущая проверка поставки

Общий release gate остаётся открытым. Здесь проверены упаковщик и содержимое ZIP; запуск Tropa, UAC, TUN, скачанных VPN/scanner executables не выполнялся.

## Проверяемый состав

В корневой папке архива ожидаются ровно шестнадцать файлов: `Tropa.exe`, `README.txt`, `build-info.json`, `component-inventory.json`, `THIRD_PARTY_NOTICES.md`, `licenses/GPL-3.0.txt`, `licenses/WarpScout.txt`, `resources/verge-mihomo.exe`, `resources/warpscout.exe`, `licenses/Twemoji-Mozilla.txt`, `licenses/Apache-2.0.txt`, `licenses/CC-BY-4.0.txt`, `licenses/Wintun-0.14.1.txt`, `licenses/Go-modules.txt`, `licenses/Frontend-candidates.txt`, `licenses/Rust-release-candidates.txt`. Это состав текущего тестового комплекта, не запрет на добавление необходимых DLL в будущем; такое изменение требует явного обновления inventory и проверок.

`scripts/package-tropa.py` копирует только перечисленные источники, вычисляет хэши всех payload-файлов и после записи ZIP запускает проверку. `scripts/verify-tropa-package.py` проверяет архив отдельно, без распаковки и выполнения содержимого: единый корень, точный состав, отсутствие дублей без учёта регистра, traversal/backslash/ADS/symlink, лимиты размера, покрытие манифестом, фактические хэши и trusted pins двух sidecar EXE и пяти дополнительных notices (font, Go modules и Wintun). Встроенный inventory должен совпадать с inventory инструмента проверки. Изменённый sidecar EXE не допускается только на основании изменённого build-info.

Это проверка целостности и состава относительно локального доверенного inventory. Она не подтверждает подлинность Tropa.exe, цифровую подпись, release-profile/features, PE subsystem/UAC manifest, полноту исходников или безопасность программы. Поле publicReleaseReady остаётся false.

Команды из корня проекта:

```powershell
python -m unittest discover -s scripts -p test_tropa_bundle.py
python scripts/package-tropa.py --binary <path-to-test-build.exe> --name Tropa-local-test
python scripts/verify-tropa-package.py .artifacts/portable/Tropa-local-test.zip
node --check scripts/prepare-tropa.mjs
```

Упаковка не собирает и не запускает EXE. Существующий архив не перезаписывается. CLI упаковщика сейчас по умолчанию использует debug-путь; это не команда публичного release. Старые тестовые архивы без новых notices/inventory не проходят новую проверку и автоматически не перепаковываются.

## Фактические тесты

Девять packaging unittest проходят: лишняя регистрация в ZIP, замена ядра с пересчитанным манифестом, отсутствие notices, traversal, case-insensitive duplicate, чистый ZIP, сквозная упаковка, обязательные тексты font licenses и их подмена с пересчитанным манифестом. Сквозной сценарий использует неисполняемый синтетический файл вместо Tropa и настоящие закреплённые sidecar-файлы только как копируемые байты. В исходной fixture-папке есть синтетическая регистрация; она не попадает в ZIP. Файлы создаются во временной директории и удаляются после теста. Проверяется реальная логика упаковщика и отдельного верификатора; сетевые бинарники не выполняются. Node syntax check подготовительного скрипта также прошёл.

## До выпуска

### Контролируемая привязка локальной сборки

`scripts/build-tropa.py --name <unique-name>` компилирует только portable release bin через offline Cargo и сохраняет выбранные исходники **до** компиляции. Нужен заранее собранный `dist/index.html`; Vite нельзя запускать одновременно с Cargo. Команда не запускает EXE, UAC, TUN или sidecars. Текущие `CARGO_HOME`, `RUSTUP_HOME` и PATH должны указывать на подготовленные локальные toolchains.

После exit 0 проверяются Cargo compiler-artifact (bin, opt_level=3, test=false, точные features custom-protocol/default/portable), неизменность набора/байтов выбранных исходников и файлов dist, затем PE/manifest. Только после этих проверок создаётся `.artifacts/validation/<name>/build.json`. При ошибке остаются snapshot/log для диагностики, но успешного build.json нет. Папка evidence не перезаписывается.

Упаковщик принимает пару `--build-record <build.json> --source-snapshot <source.zip>` вместе с `--binary <exe>`. Он повторно сверяет текущие source/dist/EXE, проверяет хэш скопированного EXE и включает summary в build-info.json без абсолютных путей. Параметры необязательны для прежних development-test fixtures; отсутствие записи не означает подтверждённого происхождения сборки.

Это локальная запись контролируемого процесса, которую владелец файлов может изменить. Она не является подписью, независимой аттестацией, полным corresponding source или доказательством воспроизводимости. Внешние зависимости, toolchain и все environment flags не зафиксированы полностью; publicReleaseReady=false. Статический ZIP verifier проверяет целостность payload, но отдельно не аутентифицирует localBuild. Исторический release EXE не получает такую запись задним числом.

Остаются полноценный license/notice audit, соответствующий source snapshot рабочего дерева, release build без probe/clippy, статическая независимая проверка его PE и manifest, подпись конкретных файлов, WebView2/чистая Windows, перенос/обновление папки и разрешённые нативные проверки подключения/очистки. Упаковочный тест не закрывает эти пункты и не создаёт готовый дистрибутив приложения.

Статический PE inspector и снимок выбранных рабочих исходников добавлены отдельным этапом: [протокол проверки](source-and-pe-2026-10-28.md). Нативный запуск они не выполняют; существующий debug EXE прошёл PE/manifest проверку. Release-компиляция начата, успех пока не объявлен.

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
