# AGENTS.md

Инструкции для AI-агентов и разработчиков, работающих с репозиторием **remote-http-action**.
Подробности — в [docs/](docs/index.md), расширенные правила для агентов — в [docs/agents/](docs/agents/index.md).

## Что это за проект

Десктопное приложение для Windows (Tauri 2 + React/TypeScript). Приложение держит HTTP-сервер:
запрос с API-ключом к адресу flow запускает на компьютере заранее настроенную последовательность действий.
Всё, что доступно в окне, доступно и через CLI (`remote-http-action-cli.exe`).

## Стек

- Backend: Rust (`src-tauri/`), axum, tokio, rusqlite (SQLite), windows-rs.
- Frontend: React 19 + TypeScript (`src/`), Vite, Tailwind CSS v4, shadcn/ui на Base UI, TanStack Query, i18next (ru/en).
- Тесты: `cargo test` (unit и интеграционные), Vitest + Testing Library.
- Пакетный менеджер: **pnpm**.

## Команды

| Задача | Команда |
|---|---|
| Установить зависимости | `pnpm install` |
| Запуск в режиме разработки | `pnpm tauri dev` |
| Все проверки типов и тесты | `pnpm test:all` |
| Линтеры (ESLint, Prettier, rustfmt, clippy) | `pnpm lint` |
| Автоформатирование | `pnpm format` |
| Сборка релиза (MSI) | `pnpm tauri build` |
| Релиз целиком: MSI + портативный zip | `pnpm package` |

Перед завершением любой задачи выполняются `pnpm lint` и `pnpm test:all`. Обе команды должны проходить без ошибок.

## Карта кода

| Путь | Назначение |
|---|---|
| `src-tauri/src/core.rs` | `Core` и перечисление `Request`: **единый набор операций** для UI и CLI |
| `src-tauri/src/http/` | HTTP API: маршрутизация, авторизация, сетевые фильтры, защита от перебора |
| `src-tauri/src/runner.rs` | Выполнение flow: очередь, cooldown, задержка старта, таймауты, отмена, история |
| `src-tauri/src/actions.rs` | Типы действий: валидация параметров и исполнение |
| `src-tauri/src/platform/` | Интеграция с ОС за трейтом `Platform` (реальная реализация и mock для тестов) |
| `src-tauri/src/ipc.rs`, `cli.rs` | CLI: разбор аргументов, named pipe к запущенному экземпляру |
| `src-tauri/src/bin/remote-http-action-cli.rs` | Консольный exe для CLI (терминал ждёт его завершения) |
| `src-tauri/src/headless.rs` | Режим `--headless`: сервер и CLI без окна |
| `src-tauri/src/logs.rs` | Журналы http/cli/flow в JSON Lines, ротация |
| `src-tauri/src/tls.rs` | HTTPS: самоподписанный сертификат, загрузка PEM, отпечаток |
| `src-tauri/src/platform/autostart.rs` | Автозапуск через планировщик заданий |
| `src-tauri/src/db.rs` | Схема SQLite, миграции, репозитории |
| `src-tauri/src/settings.rs` | Настройки, значения по умолчанию, валидация |
| `src-tauri/src/gui.rs` | Окно, трей, команда `api`, пересылка событий в UI |
| `src/lib/` | Типы (зеркало Rust-моделей), API-клиент, хелперы |
| `src/features/` | Страницы: flows, keys, groups, runs, logs, settings |
| `scripts/` | `smoke.ps1` — сквозной тест реального exe; `package-portable.ps1` — портативный zip |
| `assets/app-icon.svg` | Исходник иконки; набор в `src-tauri/icons/` генерируется `pnpm tauri icon assets/app-icon.svg` |
| `src/i18n/` | Переводы ru/en (наборы ключей должны совпадать) |

## Обязательные правила

1. **Паритет UI и CLI.** Новая пользовательская операция добавляется как вариант `Request` в `core.rs`.
   Затем она получает CLI-команду в `cli.rs` и вызов из UI через `call()`. Логику в UI и CLI не дублировать.
2. **Тесты к каждой фиче.** Rust: unit-тесты рядом с кодом, сценарии HTTP — в `src-tauri/src/http/tests.rs`.
   Frontend: `*.test.ts(x)` рядом с компонентом. Действия ОС тестируются только через `MockPlatform`.
   Тесты не должны выключать, перезагружать или усыплять машину.
3. **Документация вместе с кодом.** Любое изменение поведения сопровождается правкой соответствующего раздела `docs/`.
   Если меняются правила работы с репозиторием, обновляется и этот файл.
4. **Стиль документации — как у самостоятельного продукта.** Без упоминаний процесса разработки, конкретных машин, сетей и дат проверки.
5. **Безопасность.**
   - В журналы не пишутся секреты: в HTTP-журнале — имя ключа, в CLI-журнале — только запросы.
   - Секреты API-ключей не хранятся (только SHA-256), не логируются и не возвращаются списками.
   - Через CLI нельзя менять настройку `cli.*`.
   - Новые эндпоинты требуют ключ, если нет явной причины обратного.
6. **Синхронизация типов.** Изменения в моделях Rust (`model.rs`, `settings.rs`, `Request`) повторяются в `src/lib/types.ts`.
7. **Переводы.** Новые строки UI добавляются в `src/i18n/ru.ts` и `src/i18n/en.ts` одновременно; тест проверяет совпадение ключей.
8. **Новые действия** добавляются по инструкции [docs/development/adding-action.md](docs/development/adding-action.md).
9. **Git.** Ветка `main`, сообщения в стиле Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`).

## Документация

- [docs/index.md](docs/index.md) — оглавление.
- [docs/user/](docs/user/index.md) — руководство пользователя.
- [docs/api/](docs/api/index.md) — HTTP API.
- [docs/architecture/](docs/architecture/index.md) — устройство приложения.
- [docs/development/](docs/development/index.md) — сборка, тесты, соглашения.
- [docs/agents/](docs/agents/index.md) — расширенные правила для агентов.
