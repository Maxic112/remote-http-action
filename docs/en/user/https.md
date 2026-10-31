# HTTPS

By default the server uses HTTP, so API keys travel over the network in clear text.
For networks where traffic may be intercepted, enable HTTPS in **Settings → HTTPS**.

## Certificate modes

| Mode | Description |
|---|---|
| Self-signed | Generated automatically when HTTPS is first enabled and stored in `<data folder>\tls\` (`cert.pem`, `key.pem`). Valid for 10 years. Covers `localhost`, the computer name, `127.0.0.1` and the IPv4 addresses of the network interfaces. |
| Own certificate | Paths to the certificate chain and private key in PEM format. For certificates issued by a trusted or internal CA. |

After saving, the server restarts and accepts `https://` connections only; the window header shows the address with the scheme.
If the certificate cannot be loaded, the server does not start: the header shows "Server error" and the cause is shown in the settings.

## Trusting a self-signed certificate

Clients do not trust a self-signed certificate by default. Options:

- **Compare the fingerprint.** The settings show the certificate's SHA-256 fingerprint; a client application can pin it.
- **Skip verification on the client** — for testing only, e.g. `curl -k` or `Invoke-WebRequest -SkipCertificateCheck`.
- **Add the certificate to the trusted store** on the client device (`cert.pem`).

```bash
curl -k -X POST https://<host>:8765/flow/sleep/ -H "Authorization: Bearer rha_..."
```

## New certificate

**Generate a new certificate** (or `remote-http-action-cli.exe tls regenerate`) replaces the self-signed certificate and restarts the server.
Clients that pinned the previous fingerprint must update it.

## Reverse proxy

If a reverse proxy terminates HTTPS, keep HTTP in the application and add the proxy address to "Trusted reverse proxies" (see [Settings](settings.md#network-access)).
