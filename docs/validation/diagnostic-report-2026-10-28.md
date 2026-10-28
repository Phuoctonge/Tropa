# Диагностический отчёт Tropa

Продолжение вехи 1 общего плана. В настройках добавлена кнопка «Скопировать диагностику». Это локальное действие пользователя: отчёт не отправляется в сеть и не выполняет регистрацию, запуск ядра, scanner, UAC или TUN. Реальная работа clipboard/UI пока не проверена.

## Состав

IPC `get_tropa_diagnostics` снимает состояние через существующий coordinator view и возвращает schemaVersion=1, время формирования, платформу, nativeChecksPerformedByReport=false, connection и build. Явные projections `ConnectionView::diagnostic_snapshot` и `SearchProgress::diagnostic_snapshot` включают только enums состояния/health/ошибок, needsDisconnect, searchOnly, routing mode/число приложений, счётчики поиска по транспортам, elapsed time и phase timings.

Не сериализуются error text, profile ID, preset/address-family строки, candidate ID, paths приложений, endpoints, account/ключи/tokens, runtime YAML, logs и ответы API. Отчёт не читает account store или папку secrets. Это allowlist известных полей, а не поиск и удаление секретов из сырых логов.

`warp_diagnostics::component_report` читает основной EXE и два фиксированных sidecar filename как байты в spawn_blocking. SHA-256 основного EXE помечен observed; его достоверность или соответствие исходникам этим не утверждаются. Ожидаемые версии/хэши sidecars берутся из compile-time inventory; actualSha256 и expectedSha256 показываются вместе со статусом matched/mismatch/missing/unreadable. Symlink/необычный файл и размер свыше 128 MiB отклоняются; чтение также ограничено 128 MiB при росте файла. Локальные пути и сырые IO errors в результат не входят. Hash match не является цифровой подписью или проверкой безопасности.

Сервис сначала получает готовый report, затем вызывает существующий clipboard plugin. При неудачном сборе прежний clipboard не перезаписывается. Копирование проходит через mutation funnel; пользователю показываются успех либо ошибка. Отчёт можно получить во время поиска: это не операция подключения и не меняет manual pin.

## Проверки и ограничения

Регрессии сначала воспроизвели отсутствие safe projections и copy service. Затем проверены marker strings в error/profile/application path/preset/family/candidate, сохранение safe counts/codes, actual/expected hashes синтетического файла, mismatch/missing/unreadable и интеграция pinned inventory. Clipboard service проверен mock-функциями, без доступа к системному буферу.

461 Rust library tests, 69 frontend tests в 16 файлах, TypeScript, scoped ESLint, Vite production build и locked/offline production portable-feature cargo check прошли. Независимое read-only review не выявило actionable correctness findings. Desktop clipboard и native VPN/TUN не запускались.

Отчёт — снимок состояния и файлов, не трассировка истории и не доказательство маршрутизации выбранного приложения. Hashing и снимок connection не атомарны относительно замены файлов/изменения сессии; старый отчёт не описывает автоматически новое соединение. Native TCP/UDP/IPv6, cleanup, clipboard/UI и чистая Windows остаются непроверенными. Исторический ZIP `Tropa-bound-release-2026-10-28-a.zip` эту последующую функцию не содержит и не переупаковывался.
