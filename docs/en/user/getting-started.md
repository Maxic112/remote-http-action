# Getting started

Installation, the portable version, updating and removal are described in [Installation, update and removal](installation.md).
The data folder is shown in **Settings → About**.

## Start

1. Start the application. The window opens and an icon appears in the tray.
   The HTTP server starts automatically on all interfaces, port `8765`.
2. The server state is shown in the window header, next to the administrator rights indicator (see [Administrator rights](administrator-rights.md)).

## The first flow in five steps

1. **Flows → Create flow.** Enter a name, for example "Sleep". The path is filled in automatically (`sleep`).
2. On the **Actions** tab add the **Sleep** action and save the flow.
3. **Access groups → Create group.** Select the new flow.
4. **API keys → Create key.** Select the group. Copy the key that is shown: it is displayed **only once**.
5. Call the flow from another device on the network:

```bash
curl -X POST http://<computer-address>:8765/flow/sleep/ \
  -H "Authorization: Bearer <key>"
```

The result appears on the **Runs** page.

## Firewall

Windows may ask to allow incoming connections when the server starts for the first time.
Allow access for the network types you need, otherwise requests from other devices do not reach the application.
