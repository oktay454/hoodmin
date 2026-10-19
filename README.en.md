# Hoodmin

[Türkçe](README.md) · [العربية](README.ar.md) · [English](README.en.md)

Hoodmin is an **independent VPN management panel compatible with VpnHood**, written in Rust. Manage VPN servers, access keys, connections, traffic quotas and certificates in one place. It is not the official VpnHood Manager product. VPN servers and clients use the official VpnHood software.

Each organization brings its own VPN servers (BYOS). A server belongs to one tenant only. VPN traffic goes directly from clients to VPN servers; it does not pass through Hoodmin.

## Installation on Debian 13 amd64

1. Point a panel DNS name, such as `hoodmin.example.com`, to the panel host. Allow inbound TCP 80/443. The panel and VPN server can use separate hosts and names.
2. As root, from the directory containing the downloaded package:

```bash
apt install ./hoodmin_0.1.0-8~deb13u1_amd64.deb
systemctl enable --now postgresql
hoodmin-db-setup
hoodmin-create-owner vpn@example.com https://hoodmin.example.com
```

3. Configure Nginx and a trusted HTTPS certificate using the [step-by-step installation guide](docs/uretim-kurulumu.md). The guide and the detailed handbook are in Turkish. Set `VHM_PUBLIC_ORIGIN` to the exact public HTTPS origin without a trailing slash. Nginx forwards requests to `127.0.0.1:3080` and overwrites forwarded client IP headers.
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

## Operations and reference

- [Handbook](docs/elkitabi.md): menus, first server, access keys, profiles and roles.
- [AutoSign](docs/autosign.md): DNS and inbound TCP 80 must reach the VPN host. Start with the staging CA; staging certificates are not deployed. Production mode deploys the validated PFX and checks renewal every 12 hours. Do not manage the same VPN certificate with a second Certbot timer.
- [Backups and recovery](docs/yedekleme.md): run `hoodmin-backup` as root. Preserve the database dump, `/var/lib/hoodmin/master.key`, settings and the `acme` tree together. Store an encrypted copy off the machine. `hoodmin-backup-export` requires the optional `age` package and a separately stored recovery identity. Backups are not automatically deleted.
- [Troubleshooting](docs/sorun-giderme.md): `systemctl status hoodmin` and `journalctl -u hoodmin`.
- [Architecture](docs/mimari.md), [API](docs/yonetim-api.md), [protocol](docs/protokol-uyumlulugu.md), [build and validation](docs/dogrulama.md).

The executable is `/usr/bin/hoodmin`, the service is `hoodmin.service`, settings are in `/etc/hoodmin/hoodmin.env`, and data is in `/var/lib/hoodmin`. The service account, group and PostgreSQL peer role are named `hoodmin`. The database is named `vpnhood_manager`. The account name is a local identity, not a panel login. Settings use `VHM_*` and cookies use `__Host-vhm_*`.

This release requires a fresh installation; packages older than `0.1.0-8~deb13u1` are not migrated. For subsequent upgrades, back up and stop the service, install the new package, run `hoodmin-db-setup`, then restart. Preserve the master key and environment file. Validate restoration in a separate database before any recovery into a replacement installation.

## Scope

Compatibility target: VpnHood **9.0.856** on Debian 13 amd64. Windows installation is not end-to-end verified. AutoSign supports one FQDN per server with HTTP-01, without wildcard/DNS-01 or group-wide issuance. Other VPN engines, cloud IP ordering, billing, per-tenant branding and public signup are unavailable. Master-key rotation, external alerts and automatic backup retention are unavailable. Sustained load/outage tests and full desktop routing tests are incomplete; full commercial Manager equivalence is not claimed.
