# Соглашения

## Код

- **Rust:** форматирование `rustfmt`, проверка `clippy -D warnings`, edition 2024.
  - Ошибки — `AppError` с `ErrorCode`.
  - Новые коды ошибок отражаются в HTTP-статусах (`ErrorCode::http_status`), в кодах выхода CLI (`cli::exit_code`) и в переводах `errors.*`.
- **TypeScript:** strict, ESLint (typescript-eslint, react-hooks), Prettier.
  - Импорты через алиас `@/`.
  - Компоненты shadcn лежат в `src/components/ui/` и правятся минимально.
- **Base UI:** события компонентов называются `onCheckedChange`, `onValueChange`, `onOpenChange`. Вместо `asChild` используется проп `render`.
- **Данные в UI** загружаются через хуки TanStack Query из `src/lib/api.ts`. Изменения — через `useApiMutation` со списком инвалидируемых запросов.
- **Строки UI** только через i18n. Ключи `ru` и `en` должны совпадать.

## Единый набор операций

Пользовательская операция реализуется один раз — в `Core::handle` (вариант `Request`). Затем к ней подключаются:

1. CLI-команда в `cli.rs` (`to_request` и, при необходимости, колонки таблицы в `columns_for`);
2. вызов из UI через `call()` или `useApiMutation`;
3. тип в `src/lib/types.ts`.

## Коммиты

[Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, `ci:`.
Один коммит — одно логическое изменение вместе с тестами и документацией.

## Документация

- Документация описывает продукт таким, какой он есть: без истории разработки, упоминаний конкретных машин, сетей и дат.
- Каждый раздел `docs/` имеет `index.md`. Новые страницы добавляются в индекс раздела и в `docs/index.md`.
- Изменение поведения и документации — в одном коммите.
