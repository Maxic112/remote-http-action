# Administrator rights

Some actions need Windows administrator rights, for example managing services or changing system settings.

## Indicator

The window header always shows an indicator:

- green shield **Administrator** — the application runs elevated;
- yellow shield **Not elevated** — regular rights.

Clicking the yellow indicator restarts the application with administrator rights.
Windows asks for confirmation (UAC); the current window then closes and the application starts again.
If the prompt is declined, the application keeps running with regular rights.

## How steps run

A step marked "Requires administrator rights" runs **only if the whole application runs as administrator**.
Otherwise the step fails with `ADMIN_REQUIRED` and its "on error" rule applies.

A UAC prompt is never shown for a remote call: there would be nobody to confirm it.
For such flows to work, the application must run elevated all the time.

Shutdown, restart and sleep usually do not need administrator rights on a workstation.
