# Change checklist

## Code

- [ ] A user operation is a `Request` and is available in both the UI and the CLI.
- [ ] Types in `src/lib/types.ts` match Rust.
- [ ] UI strings exist in both `en.ts` and `ru.ts`.
- [ ] Operating-system actions go through `Platform`.
- [ ] New errors use a suitable `ErrorCode`.
- [ ] Comments are in English and explain only what is not obvious.

## Tests

- [ ] Unit tests for new logic.
- [ ] An HTTP integration test if the API changed.
- [ ] A component test if the UI changed.
- [ ] `pnpm test:all` passes.
- [ ] `pnpm lint` passes.

## Documentation

- [ ] Changes are made in both `docs/en/` and `docs/ru/`.
- [ ] The user guide (`docs/<language>/user/`) reflects the new behaviour.
- [ ] `docs/<language>/api/` is updated if the HTTP API changed.
- [ ] `docs/<language>/user/cli.md` is updated if the CLI changed.
- [ ] `docs/<language>/architecture/` is updated if the design changed.
- [ ] New pages are added to the indexes.
- [ ] `CHANGELOG.md` has an entry for user-visible changes.
- [ ] `AGENTS.md` is updated if the rules or the code map changed.
