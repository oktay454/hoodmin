# Hoodmin

[Türkçe](README.md) · [English](README.en.md) · [العربية](README.ar.md)

Hoodmin is an independent management panel for VpnHood environments, written in Rust. Organizations manage their own VPN servers, access keys, sessions and certificates in separate workspaces. Clients send VPN traffic directly to VpnHood servers.

- Linux VpnHood server installation, SSH, reusable server profiles and group routing.
- Bulk access keys, access profiles, speed, device, traffic and time limits.
- PFX certificates, Let's Encrypt HTTP-01 AutoSign and encrypted address publication.
- Usage reports, audit log, team roles, bots and a JSON API.
- Turkish, English and Arabic interface, with light and dark themes.
- Local or remote PostgreSQL, automatic Debian upgrades, encrypted backups, key rotation and notifications.

The validation baseline is Debian 13 amd64 and VpnHood 9.0.856. The panel works with the official VpnHood server and clients.

[Debian installation](docs/uretim-kurulumu.md) · [User handbook](docs/elkitabi.md) · [Operations and technical documentation](docs/index.md). The detailed guides are in Turkish.

## License

Hoodmin is free software under the GNU General Public License, version 3 or, at your option, any later version (`GPL-3.0-or-later`). [License text](LICENSE).
