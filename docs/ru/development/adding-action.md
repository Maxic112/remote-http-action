# Добавление нового действия

Пример — гипотетическое действие `lock` (заблокировать сеанс) с параметром `delay_sec`.

## 1. Backend: `src-tauri/src/actions.rs`

1. Опишите параметры структурой с `#[serde(deny_unknown_fields)]`:

   ```rust
   #[derive(Debug, Clone, PartialEq, Deserialize, Default)]
   #[serde(deny_unknown_fields, default)]
   pub struct LockParams {
       pub delay_sec: u64,
   }
   ```

2. Добавьте вариант в `ActionParams` и ветку в `parse()` с проверкой значений.
3. Добавьте запись в `ACTION_TYPES` (`admin_hint: true`, если действию обычно нужны права администратора).
4. Реализуйте ветку в `execute()`.
   Если нужно обращение к ОС, добавьте метод в трейт `Platform` (`src/platform/mod.rs`) и реализуйте его:
   - в `SystemPlatform` — настоящее действие, а для Windows-специфичного кода `#[cfg(windows)]`;
   - в `MockPlatform` — запись вызова.
5. Тесты в `actions.rs`: валидные и невалидные параметры, выполнение через `MockPlatform`.
   Тест `every_listed_type_is_parseable` проверяет, что новый тип разбирается с параметрами по умолчанию.

## 2. Frontend

1. `src/lib/types.ts` — добавьте тип в `ActionType`.
2. `src/lib/flow.ts` — параметры по умолчанию в `defaultParams()` и тип в `ACTION_TYPES`.
3. `src/features/flows/ActionEditor.tsx` — поля формы для параметров.
4. `src/i18n/ru.ts` и `en.ts` — название в `actions.types` и подписи полей.
5. Тест в `src/features/flows/FlowEditor.test.tsx` или `src/lib/flow.test.ts`.

## 3. Документация

- `docs/en/user/actions.md` и `docs/ru/user/actions.md` — раздел с описанием и параметрами.
- При изменении трейта `Platform` — `docs/<язык>/architecture/overview.md`.

## 4. Проверка

```bash
pnpm lint
pnpm test:all
```
