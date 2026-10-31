# Autostart

Many actions need administrator rights. A regular autostart through the `Run` registry key would start the application without elevation, or with a UAC prompt at every sign-in.
That is why autostart uses **Windows Task Scheduler**.

## Setting up

**Settings → Autostart → Set up autostart** or `remote-http-action-cli.exe autostart install`.

The application must be running **as administrator**: Windows allows only administrators to create tasks with the highest privileges.
Without the rights, restart the application elevated (yellow shield in the header) and try again.

A `Remote HTTP Action` task is created in the root of the Task Scheduler library:

| Task setting | Value |
|---|---|
| Trigger | At sign-in of the current user |
| Rights | Highest (`HighestAvailable`) — no UAC prompt |
| Action | Start `remote-http-action.exe --minimized` (directly to the tray) |
| Execution time limit | None |
| On battery | Allowed |
| If already running | Do not start a second instance |

## State

| State | Meaning |
|---|---|
| Autostart is set up | The task exists and starts this executable. |
| Autostart is not set up | There is no task. |
| The task points to a different executable | The application was moved or updated into another folder. Click Reinstall. |

## Removing

**Remove autostart** or `remote-http-action-cli.exe autostart remove`.
A task created with administrator rights may require administrator rights to remove.

The task can be viewed and edited in Task Scheduler (`taskschd.msc`).
