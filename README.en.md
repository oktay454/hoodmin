# Hoodmin

[Türkçe](README.md) · [العربية](README.ar.md) · [English](README.en.md)

Hoodmin is an **independent VPN management panel compatible with VpnHood**, written in Rust. Manage VPN servers, access keys, connections, traffic quotas and certificates in one place. It is not the official VpnHood Manager product. VPN servers and clients use the official VpnHood software.

Each organization brings its own VPN servers (BYOS). A server belongs to one tenant only. VPN traffic goes directly from clients to VPN servers; it does not pass through Hoodmin.

## Installation on Debian 13 amd64

1. Point a panel DNS name, such as `hoodmin.example.com`, to the panel host. Allow inbound TCP 80/443. The panel and VPN server can use separate hosts and names.
2. As root, prepare a private (0600) `/root/hoodmin-db.url` containing the application connection URL. PostgreSQL is configured separately, locally or remotely; the package does not depend on a PostgreSQL server. The following example uses a separately installed local server:

```bash
apt install ./hoodmin_0.5.0-1~deb13u1_amd64.deb
systemctl enable --now postgresql
hoodmin-cli config set --database-url-file /root/hoodmin-db.url --public-origin https://hoodmin.example.com
hoodmin-cli database init --local
hoodmin-cli owner-init --email vpn@example.com
```

3. Configure Nginx and a trusted HTTPS certificate using the [step-by-step installation guide](docs/uretim-kurulumu.md). The guide and the detailed handbook are in Turkish. Set `public_origin` to the exact public HTTPS origin without a trailing slash. Nginx forwards requests to `127.0.0.1:3080` and overwrites forwarded client IP headers.
4. Start the panel after configuring HTTPS:

```bash
systemctl enable --now hoodmin.service
curl --fail --show-error https://hoodmin.example.com/healthz
```

5. Read `/root/hoodmin-first-owner.json` on the installation machine in your trusted root terminal. It contains the initial email and random password. Sign in and change the password under **My account**. There is no default password or public registration.
6. Under **Servers**, register the public VPN IP/FQDN and TCP port. Run the generated official installer on the VPN host, or use the SSH installation form with a verified SSH host fingerprint. Wait for the server to appear online.
7. Under **Access keys**, create a key and import its `vh://` value into the official VpnHood client. Panel accounts and VPN access keys are different credentials.

The package installs the panel, not the VPN server. Do not run the isolated QA setup scripts on an existing installation.

## Features

- Turkish, Arabic and English, Arabic RTL, light/dark themes and contextual help.
- Official Linux installer commands, SSH installation/reinstallation and Windows PowerShell installer commands.
- VPN server configuration, reusable server profiles and automatic propagation to assigned groups.
- Bulk access keys, access profiles, speed/device/traffic/time limits and VPN session termination.
- Routing within a tenant's server group and encrypted address refresh/publication to external HTTPS storage.
- PFX import, shared group certificates and per-server Let's Encrypt HTTP-01 AutoSign.
- Usage charts, reported-country device counts, CSV export and audit log.
- Five team roles, expiring bot keys and a JSON management API.
- PostgreSQL RLS, Argon2id passwords and AES-256-GCM protection for stored management secrets.

The static client-side UI (SPA) renders all screens from the management API. JavaScript is required; session credentials remain in HttpOnly cookies. Turkish, English and Arabic (RTL), light/dark themes and explicit confirmations are supported.

## Key rotation, retention and alerts

As root, take a verified independent backup, run `hoodmin-cli keys status`, then `hoodmin-cli keys rotate --confirmed`. All encrypted database fields are rewrapped in one transaction. The durable keyring allows interrupted rotations to recover; previous keys remain for old backups (maximum 32 keys). VPN identities are preserved.

Retention is disabled by default. Configure a private JSON policy with `hoodmin-cli backup policy --file FILE`, preview with `backup prune`, and delete with `backup prune --apply`. Only completed, checksum-validated copies are eligible; the newest `keep_min` copies are protected. Successful backup creation/export can apply an enabled policy. Disk thresholds cannot guarantee reserved filesystem space.

Administrators use **Notifications** to add HTTPS webhook or implicit-TLS SMTP channels, test delivery, and inspect the persistent queue. Offline/recovery, AutoSign and publication failure/recovery events are deduplicated. Failed delivery retries up to eight times; receivers should deduplicate event IDs. Credentials are encrypted and masked. An operator may opt one tenant into backup/disk status events using `backup.notify_tenant`; a stopped service or database requires independent monitoring. [Full operations reference](docs/isletim-politikalari.md).

## Operations and reference

- [Rust CLI](docs/cli.md): setup, maintenance and all management API operations.
- [Handbook](docs/elkitabi.md): menus, first server, access keys, profiles and roles.
- [AutoSign](docs/autosign.md): DNS and inbound TCP 80 must reach the VPN host. Start with the staging CA; staging certificates are not deployed. Production mode deploys the validated PFX and checks renewal every 12 hours. Do not manage the same VPN certificate with a second Certbot timer.
- [Backups and recovery](docs/yedekleme.md): run `hoodmin-cli backup create` as root. Preserve the database dump, `/var/lib/hoodmin/master.key`, settings and the `acme` tree together. Store an encrypted copy off the machine. `hoodmin-cli backup export` requires the optional `age` package and a separately stored recovery identity. Retention is opt-in; preview with `hoodmin-cli backup prune` before enabling deletion.
- [Troubleshooting](docs/sorun-giderme.md): `systemctl status hoodmin` and `journalctl -u hoodmin`.
- [Architecture](docs/mimari.md), [API](docs/yonetim-api.md), [protocol](docs/protokol-uyumlulugu.md), [build and validation](docs/dogrulama.md).

The executable is `/usr/bin/hoodmin`, the service is `hoodmin.service`, settings are in `/etc/hoodmin/hoodmin.json`, and data is in `/var/lib/hoodmin`. The service account/group are `hoodmin`. Database host, port, database and login are configured explicitly in the JSON settings. Local sockets can use peer authentication; TCP requires `sslmode=verify-full`. The account name is a local identity, not a panel login. Cookies use `__Host-vhm_*`.

Package upgrades start the root-only `hoodmin-upgrade.service`, which verifies and backs up pending migrations before applying them through the native Rust PostgreSQL driver. The web service only checks readiness. Package configuration waits at most 20 seconds; a temporary database outage defers work without failing installation. The worker retries, and every web service start triggers it again. A deliberately stopped web service stays stopped. Local peer administration is automatic; for remote PostgreSQL, configure a root-owned 0600 URL file targeting the **same application database** with `hoodmin-cli upgrade --configure --admin-url-file FILE`. Require verified TLS. Keep appropriate `pg_dump` tools and backup disk space available. Configuration and master keys are preserved. Unknown or modified migration history blocks readiness; automatic schema downgrade is unsupported. Inspect `hoodmin-cli upgrade --status` and `journalctl -u hoodmin-upgrade.service`. [Upgrade guide](docs/yukseltme.md).

## Scope

Compatibility target: VpnHood **9.0.856** on Debian 13 amd64. Windows installation is not end-to-end verified. AutoSign supports one FQDN per server with HTTP-01, without wildcard/DNS-01 or group-wide issuance. Other VPN engines, cloud IP ordering, billing, per-tenant branding and public signup are unavailable. Sustained load/outage tests and full desktop routing tests are incomplete; full commercial Manager equivalence is not claimed.

## Static frontend deployment

One package installs the complete public frontend at `/usr/share/hoodmin/web/`. Both modes retain client-side rendering. By default Hoodmin serves that directory. For direct Nginx serving, run `hoodmin-cli config web --serve-static false` on the backend. On the same host Nginx reads the installed directory; on a separate host copy only that entire directory, using the same Hoodmin version. No second package, database credentials or key files are needed on the frontend host. UI routes start at `/`, `/servers`, `/access-keys` and `/account`; there is no `/panel` prefix.

Use `deploy/nginx-hoodmin.conf` for built-in serving, `deploy/nginx-hoodmin-static.conf` for local Nginx static serving, or `deploy/nginx-hoodmin-static-remote.conf` with `deploy/nginx-hoodmin-backend.conf` for separate hosts. Replace all example names, certificates and the allowed frontend source IP. Remote backend HTTPS verifies the certificate and hostname. Browser/API use one public origin; `/api/`, `/s/`, `/f/` and health checks remain dynamic. Nginx serves the UI even while the daemon is stopped; the browser shows a simple error and retries. [Copying, upgrades and complete steps](docs/web-sunumu.md).
