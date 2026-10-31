# Окружение и сборка

## Требования

- Windows 10/11 x64.
- [Rust](https://rustup.rs/) stable, toolchain `x86_64-pc-windows-msvc`.
- Visual Studio Build Tools с компонентом «Разработка классических приложений на C++» (MSVC).
- WebView2 Runtime.
- Node.js 22+ и pnpm 10 (`corepack enable`).

## Первая настройка

```bash
pnpm install
```

## Режим разработки

```bash
pnpm tauri dev
```

Запускает Vite на порту 1420 и отладочную сборку приложения. В отладочной сборке:

- данные хранятся в `%APPDATA%\remote-http-action-dev\`;
- CLI-pipe называется `\\.\pipe\remote-http-action-dev`;
- управление через CLI включено по умолчанию;
- exe имеет консольную подсистему, вывод CLI виден сразу.

CLI отладочной сборки:

```bash
src-tauri\target\debug\remote-http-action-cli.exe keys list
```

## Переменные окружения

| Переменная | Назначение |
|---|---|
| `RHA_DATA_DIR` | Папка данных вместо стандартной |
| `RHA_IPC_ENDPOINT` | Имя pipe (или путь к сокету) |
| `RHA_DRY_RUN_POWER=1` | Не выполнять выключение, перезагрузку и сон, а только писать о них в stderr |

Переменная `RHA_DRY_RUN_POWER=1` удобна, чтобы проверять flow с действиями питания без отключения машины.

## Сборка релиза

```bash
pnpm package
```

Команда (`scripts/package.ps1`) очищает `bundle\`, выполняет `pnpm tauri build` и `scripts/package-portable.ps1`. Результат:

| Путь | Что это |
|---|---|
| `src-tauri\target\release\remote-http-action.exe` | Приложение |
| `src-tauri\target\release\remote-http-action-cli.exe` | Консольный CLI |
| `src-tauri\target\release\bundle\msi\*_ru-RU.msi`, `*_en-US.msi` | Установщики (ставят оба exe) |
| `src-tauri\target\release\bundle\portable\*_portable.zip` | Портативная версия: оба exe и `portable.flag` |
| `src-tauri\target\release\bundle\release\` | **Файлы для публикации в релиз**: MSI с именами без пробелов, zip и `SHA256SUMS.txt` |

Имена файлов в `bundle\release\` — часть контракта с механизмом обновления: программа ищет свой файл по окончанию имени
(`_x64_ru-RU.msi`, `_x64_en-US.msi`, `_x64_portable.zip`) и проверяет его по `SHA256SUMS.txt`.

Перед сборкой закройте приложение: запущенный exe нельзя перезаписать.

## Версия

Номер версии задаётся в трёх местах и должен совпадать: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.
`upgradeCode` в `tauri.conf.json` менять нельзя: по нему установщик новой версии находит и заменяет старую.

## Иконка

Исходник — `assets/app-icon.svg`. После правки пересоздайте набор:

```bash
pnpm tauri icon assets/app-icon.svg
```

Удалите появившиеся каталоги `src-tauri/icons/android` и `src-tauri/icons/ios`: мобильные платформы не используются.
