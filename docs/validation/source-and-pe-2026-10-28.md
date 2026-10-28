# Статическая проверка EXE и снимок исходников — 28 октября 2026

Продолжение вехи 9 [общего плана](../superpowers/plans/2026-10-28-tropa-next-milestones.md). Приложение, UAC, TUN и VPN/scanner executables не запускались.

## Проверка PE как данных

`python scripts/inspect-tropa-exe.py <file.exe>` читает байты, не вызывает Windows loader и не создаёт процесс из анализируемого файла. Проверяются PE/COFF, AMD64/PE32+, признак EXE вместо DLL, Windows GUI subsystem, отображение resource RVA в секции, RT_MANIFEST/ID=1 и все языковые варианты embedded manifest. Для каждого требуется requestedExecutionLevel=requireAdministrator и uiAccess=false. Вывод содержит SHA-256 именно прочитанного файла.

Существующий `target/debug/clash-verge.exe` прошёл проверку: x64, GUI, requireAdministrator, один язык manifest, SHA-256 `8e5f9a3d82553f34299c82819fd5d1259ab2775ac9647801abc5958bd5465e29`. Это прежний debug-артефакт, не доказательство свежести сборки относительно текущего кода. Наличие manifest не доказывает причину предыдущих сетевых отказов, успешное подтверждение UAC или работу TUN.

Четыре regression-теста используют синтетические PE-байты: корректный файл, console/wrong architecture/asInvoker, отсутствующий manifest/unmapped RVA, truncated headers/resource loop. До реализации тестовый модуль не импортировался; после реализации проходят проверки принятия и отказов. Это не полный валидатор Windows PE и не проверка подписи/безопасности программы.

## Снимок рабочего дерева

`python scripts/snapshot-tropa-source.py create <snapshot.zip>` получает git ls-files для tracked и untracked файлов с учётом ignore rules, затем применяет явный allowlist source/config/assets. В снимок идут фактические байты рабочего дерева, а не git HEAD. Сохраняются lockfiles и обычные исходники; `.artifacts`, data/secrets, target, sidecar EXE, caches и environment files не включаются. Symlink и выход resolved path за корень отклоняются.

Каждый файл имеет SHA-256; snapshotId вычисляется из отсортированного набора путей и хэшей. После создания архив и рабочее дерево сравниваются повторно. `verify` заново перечисляет source inputs и обнаруживает изменённые, удалённые и новые выбранные файлы. Проверяются также состав и содержимое архива. Три теста подтверждают сохранение байтов/CRLF, обнаружение изменения или нового исходника и запрет private/generated paths.

Создан локальный `.artifacts/portable/Tropa-source-validation-2026-10-28.zip`: 739 файлов, 15 261 936 байт, snapshotId `bbf974bc6017ef61809e03f1918833be2d845de04d10ce7327db62a06c658d62`. Это snapshot выбранных inputs, не полный corresponding-source комплект GPL и не обещание byte-for-byte reproducible build. Внешние lockfile-зависимости, generated dist и pinned runtime binaries требуют отдельной фиксации/восстановления. Изменение файла вне allowlist не обнаруживается данным инструментом. Публичной публикации/коммита не было.

Все 13 Python-тестов упаковки, PE и source snapshot прошли. Для snapshot regression вначале выявлена особенность fixture на Windows: write_text преобразовывал LF в CRLF. Fixture исправлена на заданные байты; production snapshot сохранял фактическое содержимое правильно и не нормализует переводы строк.

## Release-компиляция

После создания snapshot запущена первая offline-компиляция:

```powershell
$env:TAURI_CONFIG = Get-Content src-tauri/tauri.tropa.conf.json -Raw
cargo build --release --locked --offline -p clash-verge --bin clash-verge --features portable
```

Default custom-protocol остаётся включённым. clippy/tropa-probe не запрашиваются, результат не запускается. До подтверждения exit code, повторной проверки source inputs, нового PE и build inputs эта попытка не считается готовой или соответствующей snapshot release-сборкой. Журнал: `.artifacts/validation/tropa-release-build-2026-10-28.log`.

## Результат release-попытки

Сессия компиляции завершилась exit code 0: `Finished release profile [optimized + debuginfo]` за 16m 04s. Новый `target/release/clash-verge.exe` — 56 592 896 байт, SHA-256 `99cfbf5d164b459849757d693c368775f1ab6e99393923ab70ae97dd29d6f2eb`. Статический inspector подтвердил x64/GUI/requireAdministrator и один языковой manifest. Cargo bin fingerprint содержит только custom-protocol/default/portable: clippy/tropa-probe не включены. Authenticode чтение вернуло NotSigned.

Снимок всех 739 выбранных source inputs повторно проверен после компиляции и упаковки: snapshotId не изменился. Сформирован `.artifacts/portable/Tropa-release-local-2026-10-28.zip`: 51 026 294 байт, SHA-256 `d30bfcff18214877545a2274cef6091207ecbf9a67ff9e7f51faf26940f4fe52`. Упаковщик и отдельный ZIP verifier подтвердили ровно 9 файлов, pinned sidecar hashes и отсутствие лишних payload-путей. EXE извлечён из ZIP только в память и проверен тем же byte-only PE inspector: его отчёт/хэш совпадает с release EXE.

Локальное evidence JSON: `.artifacts/validation/tropa-release-evidence-2026-10-28.json`. Там записаны command, features, source snapshot/archive hashes, PE, NotSigned, package SHA и inventory, а также наблюдавшиеся после сборки dist file hashes. Generated dist/external dependencies не были полностью заморожены до начала сборки: это не доказательство byte-for-byte воспроизводимости или полного corresponding-source комплекта.

Прямые PE imports нового EXE перечислены в `.artifacts/validation/tropa-release-imports-2026-10-28.json`: отдельный VCRUNTIME DLL не импортируется; Windows API/UCRT imports присутствуют. Это не исключает динамически загружаемые зависимости, в том числе WebView2, и не заменяет чистую Windows.

Архив остаётся локальным development-test комплектом с publicReleaseReady=false, хотя его основной EXE скомпилирован в release profile. Запуск приложения/sidecars, UAC/TUN, проверка подключения/очистки, подпись, полный license audit и чистая Windows не выполнялись. Предыдущая запись «сборка начата» описывает этап до этого успешного завершения.

## Результат контролируемой сборки

Новый `scripts/build-tropa.py --name Tropa-bound-release-2026-10-28-a` завершился exit 0 за 10m 08s. До Cargo сохранён snapshot 748 выбранных файлов; после Cargo повторно подтверждены набор и байты source inputs и всех dist-файлов. Основной bin compiler-artifact имеет ровно custom-protocol/default/portable, opt_level=3 и test=false. PE inspector подтвердил x64/GUI/requireAdministrator; EXE не запускался.

- Source snapshot ID: `c471c6ec67edc418ff07c7702baca2643c520c42a1bf877b857a5e115f079066`.
- Source ZIP SHA-256: `6fb4ae11014d4dad574968a40cdbc873dfbb58776754b76344041a275b1fe562`.
- EXE SHA-256: `fdb87f6af90c17ac9242456b508cebc2ab3d4cba9d4eeb0aaa562894932de983`.
- Portable ZIP: `.artifacts/portable/Tropa-bound-release-2026-10-28-a.zip`, 51 203 645 байт, 14 файлов, SHA-256 `86f27aab5b58332081ffda408d3c625a0ea38467952cbad9bf8e851333dea255`.

Упаковщик принял build.json/source.zip, повторно проверил inputs и EXE и встроил localBuild в build-info.json. Отдельный ZIP verifier подтвердил текущий состав, hashes и pinned inventory/notices. EXE из ZIP прочитан только в память: его PE report совпал с build record, как и встроенный localBuild. Authenticode: NotSigned.

Локальные evidence: `.artifacts/validation/Tropa-bound-release-2026-10-28-a/{source.zip,cargo.jsonl,build.json,package-evidence.json}`. Исторические архивы выше сохранены без перезаписи. Новый ZIP содержит внешние IPv6 endpoint и текущий inventory; его native runtime ещё не проверен.

Это development-test комплект с publicReleaseReady=false. Нативный запуск, подключение, UDP/IPv6-разделение, очистка, чистая Windows и подпись не проверены. Локальная запись не аутентифицирована; dependencies/toolchain/environment и полный corresponding source не зафиксированы полностью. Из неизменности записанных source/dist не следует независимая воспроизводимость или соответствие dist исходникам без отдельного frontend build; непосредственно перед этой попыткой локальный Vite build завершился успешно.

## Portable с расширенным подбором

28 октября wrapper Tropa-tuning-release-2026-10-28-b завершил offline locked release-компиляцию за 10m 09s, exit 0. В эту сборку входят исправления quality sort, очистки прошлой ошибки прохода, диагностический экспорт, optional I1 и отдельный расширенный подбор AWG/MASQUE с проверкой соответствия export. Выбранные source inputs и dist оставались неизменными во время компиляции. Cargo features: custom-protocol/default/portable; opt_level=3, test=false.

Архив `.artifacts/portable/Tropa-tuning-release-2026-10-28-b.zip`: 51 239 492 байта, 14 файлов, SHA-256 `f682707c3f361c3e5927dade05c5679274a700247456d536b840ec6647362178`. EXE SHA-256 `05871b057064234a7b8d71cd41758dbf4fd2c6662d921d8a95ec680664c23cd2`. Статический inspector подтверждает x64 / windowsGui / requireAdministrator / uiAccess=false; Authenticode NotSigned. Отдельный ZIP verifier прошёл. EXE из ZIP повторно прочитан как байты: весь результат PE inspector совпал с build.json; localBuild в ZIP равен записи сборки. Первая ручная проверка неверно исключила SHA из сравниваемого объекта; после исправления самой проверки равенство подтверждено без изменения архива.

Source snapshot ID `cf141595cb9b9b5807de3c2777aeddb90557364ab50a384c0855444874ad704e`, source.zip SHA-256 `31da5b05da7f3e86d7138359982f7407fa7e93efd0fceff6c094bda83f33d1af`. Evidence: `.artifacts/validation/Tropa-tuning-release-2026-10-28-b/{source.zip,cargo.jsonl,build.json,package-evidence.json}`. Прежние архивы сохранены. Пройдены 22 Python tests упаковки/PE/source/build binding.

Это локальная тестовая поставка без подписи, native запуска, подтверждения split UDP/IPv6, clean Windows или завершения license/source audit. publicReleaseReady=false. Сама запись сборки не является независимой аттестацией; source snapshot — выбранные inputs, не полный corresponding source.
