# Agent workflow

## Before starting

1. Read `AGENTS.md` and the parts of `docs/en/` relevant to the task.
2. Find the entry point:
   - a new user operation → `src-tauri/src/core.rs` (`Request`);
   - HTTP behaviour → `src-tauri/src/http/mod.rs`;
   - flow execution → `runner.rs`, `actions.rs`;
   - the interface → `src/features/<section>/`.
3. Look at the existing tests next to the code you change: they show the expected behaviour and ready-made helpers.

## While working

- First a test for the new behaviour, then the implementation. For HTTP — a scenario in `http/tests.rs`.
- Never call the operating system directly from the core: only through the `Platform` trait, so the code can be tested with `MockPlatform`.
- Change Rust and TypeScript types together (`model.rs`/`settings.rs`/`core.rs` ↔ `src/lib/types.ts`).
- Add new UI strings to both locales at once.
- Update the documentation in both `docs/en/` and `docs/ru/`.
- Code comments are in English and only for what is not obvious.
- If a format stored in the database changes:
  - add a new migration at the end of `MIGRATIONS` (never edit existing ones);
  - or use `#[serde(default)]` for fields of JSON documents.

## Before finishing

1. `pnpm lint` and `pnpm test:all` pass without errors or warnings.
2. The documentation is updated ([checklist](checklist.md)).
3. If the repository rules or the code map changed, `AGENTS.md` is updated.

## Never

- Run real shutdown, restart or sleep in tests.
- Log or return key secrets.
- Add operations that change the configuration to the HTTP API.
- Give the CLI access to the `cli.enabled` switch.
- Mention the development process, specific machines or networks in the documentation.
