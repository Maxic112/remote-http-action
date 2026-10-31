# Командная строка (CLI)

Все операции, доступные в окне, можно выполнять из терминала тем же исполняемым файлом:

```bash
remote-http-action.exe <группа> <команда> [параметры]
```

Запуск без параметров открывает приложение. `--minimized` запускает его сразу в трей.
`--headless` запускает без окна и иконки: работают только HTTP-сервер и CLI.

## Условия работы

- Приложение должно быть **запущено**. CLI передаёт команды работающему экземпляру и не изменяет данные напрямую.
- В настройках должно быть включено **«Разрешить управление через командную строку»**.
  В отладочных сборках CLI включён по умолчанию, в релизных — выключен.
- Команды принимаются только от того же пользователя Windows, под которым работает приложение (или от администратора).
  Терминал без повышения прав может управлять приложением, запущенным с правами администратора.
- Параметр `cli.enabled` нельзя изменить через CLI.

`--help` и `--version` работают всегда, в том числе без запущенного приложения.

## Общие параметры

| Параметр | Описание |
|---|---|
| `--json` | Вывод в JSON вместо таблиц. Удобно для скриптов. |
| `--help` | Справка по любой группе или команде. |

## Команды

### API-ключи

```bash
remote-http-action.exe keys list
remote-http-action.exe keys create "Телефон" --groups 1,2
remote-http-action.exe keys update 3 --name "Планшет" --disable --groups 2
remote-http-action.exe keys update 3 --no-groups
remote-http-action.exe keys rotate 3
remote-http-action.exe keys delete 3
```

`create` и `rotate` печатают секрет ключа один раз.

### Группы доступа

```bash
remote-http-action.exe groups list
remote-http-action.exe groups create "Питание" --description "..." --flows 1,2 --keys 3
remote-http-action.exe groups update 1 --flows 1,2,4 --no-keys
remote-http-action.exe groups delete 1
```

### Flow

Flow задаются JSON-файлом в формате, описанном в [Flow](flows.md) и [Действиях](actions.md). Опущенные поля получают значения по умолчанию.
Вывод `flows get <flow> --json` можно отредактировать и передать в `flows update`.

```bash
remote-http-action.exe flows list
remote-http-action.exe flows get sleep --json > sleep.json
remote-http-action.exe flows create --file sleep.json
remote-http-action.exe flows update 5 --file sleep.json
remote-http-action.exe flows enable sleep
remote-http-action.exe flows disable 5
remote-http-action.exe flows delete sleep
remote-http-action.exe flows run sleep --wait
```

Flow указывается числовым идентификатором или путём.

### Запуски

```bash
remote-http-action.exe runs list --limit 20 --flow 5
remote-http-action.exe runs get <id>
remote-http-action.exe runs cancel <id>
```

### Настройки

```bash
remote-http-action.exe settings get
remote-http-action.exe settings get server.port
remote-http-action.exe settings set server.port 9000
remote-http-action.exe settings set server.allowed_ips '["192.168.1.0/24"]'
```

Значение разбирается как JSON; если это не JSON, оно считается строкой.

### Сервер, блокировки, система, приложение

```bash
remote-http-action.exe server status|start|stop
remote-http-action.exe bans list
remote-http-action.exe bans remove 10.0.0.9
remote-http-action.exe system info
remote-http-action.exe system action-types
remote-http-action.exe app show|quit|restart-elevated
```

## Коды выхода

| Код | Значение |
|---|---|
| 0 | Успех |
| 1 | Внутренняя ошибка |
| 2 | Неверные аргументы |
| 3 | Приложение не запущено |
| 4 | Управление через CLI выключено |
| 5 | Доступ запрещён |
| 6 | Объект не найден |
| 7 | Ошибка проверки данных |
| 8 | Конфликт (например, flow уже выполняется) или слишком частый запуск |
