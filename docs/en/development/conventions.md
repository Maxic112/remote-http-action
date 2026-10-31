# Conventions

## Code

- **Rust:** `rustfmt` formatting, `clippy -D warnings`, edition 2024.
  - Errors are `AppError` with an `ErrorCode`.
  - New error codes are reflected in HTTP statuses (`ErrorCode::http_status`), CLI exit codes (`cli::exit_code`) and the `errors.*` translations.
- **TypeScript:** strict, ESLint (typescript-eslint, react-hooks), Prettier.
  - Imports through the `@/` alias.
  - shadcn components live in `src/components/ui/` and are changed as little as possible.
- **Base UI:** component events are `onCheckedChange`, `onValueChange`, `onOpenChange`. Use the `render` prop instead of `asChild`.
- **UI data** is loaded with the TanStack Query hooks from `src/lib/api.ts`; changes go through `useApiMutation` with the list of queries to invalidate.
- **UI strings** only through i18n. The `en` and `ru` keys must match.
- **Code comments** are in English and only where they explain something not obvious from the code: the reason for a decision, a format, a limitation. Comments that restate the code are not written.

## One set of operations

A user operation is implemented once, in `Core::handle` (a `Request` variant). Then it is connected to:

1. a CLI command in `cli.rs` (`to_request` and, if needed, table columns in `columns_for`);
2. a UI call through `call()` or `useApiMutation`;
3. a type in `src/lib/types.ts`.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, `ci:`.
One commit, one logical change, together with its tests and documentation.

## Documentation

- The documentation describes the product as it is: no development history, no specific machines, networks or dates.
- It is kept in two languages with the same structure: `docs/en/` (primary) and `docs/ru/`. A change goes into both.
- Every section has an `index.md`. New pages are added to the section index and to `docs/<language>/index.md`.
- A behaviour change and its documentation go in the same commit.
