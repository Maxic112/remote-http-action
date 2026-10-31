# Adding an action

The example is a hypothetical `lock` action (lock the session) with a `delay_sec` parameter.

## 1. Backend: `src-tauri/src/actions.rs`

1. Describe the parameters as a struct with `#[serde(deny_unknown_fields)]`:

   ```rust
   #[derive(Debug, Clone, PartialEq, Deserialize, Default)]
   #[serde(deny_unknown_fields, default)]
   pub struct LockParams {
       pub delay_sec: u64,
   }
   ```

2. Add a variant to `ActionParams` and a branch to `parse()` that validates the values.
3. Add an entry to `ACTION_TYPES` (`admin_hint: true` if the action usually needs administrator rights).
4. Implement the branch in `execute()`.
   If the action needs the operating system, add a method to the `Platform` trait (`src/platform/mod.rs`) and implement it:
   - in `SystemPlatform` — the real action, with `#[cfg(windows)]` for Windows-specific code;
   - in `MockPlatform` — recording the call.
5. Tests in `actions.rs`: valid and invalid parameters, execution through `MockPlatform`.
   `every_listed_type_is_parseable` checks that the new type parses with default parameters.

## 2. Frontend

1. `src/lib/types.ts` — add the type to `ActionType`.
2. `src/lib/flow.ts` — default parameters in `defaultParams()` and the type in `ACTION_TYPES`.
3. `src/features/flows/ActionEditor.tsx` — form fields for the parameters.
4. `src/i18n/en.ts` and `ru.ts` — the name in `actions.types` and the field labels.
5. A test in `src/features/flows/ActionEditor.test.tsx` or `src/lib/flow.test.ts`.

## 3. Documentation

- `docs/en/user/actions.md` and `docs/ru/user/actions.md` — a section with the description and parameters.
- When the `Platform` trait changes — `docs/<language>/architecture/overview.md`.

## 4. Checks

```bash
pnpm lint
pnpm test:all
```
