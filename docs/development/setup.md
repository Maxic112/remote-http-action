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
src-tauri\target\debug\remote-http-action.exe keys list
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
pnpm tauri build
```

Результат — `src-tauri\target\release\remote-http-action.exe` и установщик MSI в `src-tauri\target\release\bundle\msi\`.
Портативная версия — тот же exe с файлом `portable.flag` рядом.
