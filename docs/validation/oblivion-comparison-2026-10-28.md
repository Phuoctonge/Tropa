# Oblivion Desktop и основа Tropa — 28 октября 2026

Дополнение: после первоначального сравнения скачано полное дерево (238 файлов) и выполнен [углублённый разбор кода](oblivion-code-review-2026-10-28.md). Уточнены обратная семантика app-правил (исключение из VPN), хранение в системном userData и порядок регистрации/подготовки ресурсов.

Проверены метаданные репозитория, README, package.json, LICENSE.md и 10 выбранных файлов реализации. Snapshot: `9fdf217693968e2aa8ee0b090724b8372a9d2e3d`, main; GitHub сообщает `archived: true`. Последний commit snapshot датирован 10 декабря 2025. Материалы находятся в `.artifacts/research/oblivion-comparison-2026-10-28`. Приложение не устанавливалось и не запускалось; сравнительные показатели ресурсов и надёжности не измерялись.

Oblivion Desktop — предметно подходящий WARP-клиент. README перечисляет WARP, Gool, Cfon и MASQUE, Windows portable ZIP, tray, proxy/system proxy и TUN. Его нельзя отвергать как программу без portable, MASQUE или правил приложений.

## Проверенные различия

| Свойство | Oblivion Desktop snapshot | Текущая Tropa |
| --- | --- | --- |
| Оболочка | Electron/React/TypeScript, сборка Webpack | Tauri/Rust/React/TypeScript, Vite |
| Сетевой путь | WarpPlus/MasquePlus; TUN создаётся sing-box и направляется в локальный SOCKS outbound | mihomo держит TUN и WARP outbound; WarpScout используется для поиска |
| Правила приложений | В `sbConfig.ts` есть `process_name: rulesConfig.processSet`, UI принимает `app:` | Точные пути EXE; известный Firefox Portable launcher разрешается в настоящий EXE |
| Portable | ZIP опубликован в README; переносимость всех данных не проверена в этом исследовании | Существующий путь data рядом с EXE, но полный выпуск на чистой Windows ещё не подтверждён |
| MASQUE | Заявлен и имеет отдельный путь запуска MasquePlus | Поддержан закреплённым mihomo, основной сценарий Tropa ещё требует интеграции |
| Разработка upstream | Репозиторий архивирован | Основа Clash Verge Rev доступна для дальнейшего сопровождения; Tropa остаётся отдельным форком |

Electron сам по себе не доказывает худшую производительность, а наличие sing-box не доказывает худшую маршрутизацию. Без одинаковых измерений таких выводов делать нельзя. TUN в Oblivion не является подтверждением отсутствия конфликтов с античитами.

## Лицензия текущего snapshot

В `LICENSE.md` указано:

> No Sale: You are not allowed to sell or distribute modified versions of it.

> Modification Limitation: You may not alter the name, about page, copyright notice, logo, or other fundamental details of this software.

Следом разрешена binary distribution при сохранении исходной лицензии и copyright notice. Нельзя трактовать эту оговорку как явное разрешение на переименование и распространение собственного изменённого клиента Tropa. Для такого форка на основе текущего snapshot требуется отдельное разрешение правообладателей либо подтверждённая другая применимая лицензия конкретной версии. История лицензий старых версий здесь не исследовалась. Надпись Free and Open Source в README не отменяет условий LICENSE.md.

Clash Verge Rev распространяется под GPLv3: собственный изменённый клиент возможен при соблюдении условий лицензии, сохранении обязательных уведомлений и предоставлении соответствующих исходников. Это практическое отличие для выбранного направления Tropa.

## Рекомендация

Сохранить текущую основу. Главные основания — пригодность лицензии для отдельного форка, имеющиеся process rules/lifecycle и возможность WG/AWG/MASQUE внутри одного mihomo. Oblivion полезен как источник понимания WARP UX, схемы управления транспортами и альтернативного TUN-пути; перенос его кода и графики без проверки разрешений не планировать.

Первоначальный выбор Clash Verge Rev не был результатом полного сравнения с Oblivion. Повторный анализ поддерживает сохранение основы, но не устанавливает превосходство Tropa по скорости, интерфейсу или совместимости с играми.

Источники: [репозиторий](https://github.com/bepass-org/oblivion-desktop), [лицензия snapshot](https://github.com/bepass-org/oblivion-desktop/blob/9fdf217693968e2aa8ee0b090724b8372a9d2e3d/LICENSE.md), [TUN builder snapshot](https://github.com/bepass-org/oblivion-desktop/blob/9fdf217693968e2aa8ee0b090724b8372a9d2e3d/src/main/lib/sbConfig.ts).
