# Hoodmin

[Türkçe](README.md) · [العربية](README.ar.md) · [English](README.en.md)

Hoodmin is an **independent server and access management panel compatible with VpnHood**, written in Rust. It is not the official VpnHood Manager and does not reimplement the VPN server or client. Use the official prebuilt installer for the VpnHood server.

Each organization brings its own servers (BYOS). Each server belongs to exactly one tenant; there is no shared VPN server pool across tenants. Hoodmin does not carry VPN traffic: clients connect directly to their VPN servers.

## Status

The protocol core and tenant panel work. Compatibility with VpnHood **9.0.856** has been checked using real .NET contracts and a remote client connection. Debian 13 installation, real-domain certificate automation and isolated recovery from an encrypted backup have been verified. Sustained production load and full parity with the closed-source Manager remain unfinished. See the [validation record, in Turkish](docs/dogrulama.md).

See the [pause and continuation record](docs/calisma-durumu.md) for the latest milestone and pending work.

## Available features

- Official prebuilt Linux/Windows install commands; password/private-key SSH install/reinstall with pinned host keys and job logs.
- All 47 editable source-defined server settings; reusable server templates and atomic tenant-scoped bulk application.
- Access profiles, upload/download limits, total/cycle quotas, lifetime from first use, trial/reward plans and device policies.
- Group access, location/capacity routing, encrypted server-address publishing and access codes.
- Group refresh URLs recover from an unreachable bootstrap address by selecting a live member; verified with .NET and a real client connection.
- Shared farm PFX storage and distribution to existing, new and reassigned members, preserving VPN addresses and server secrets.
- Password-protected PFX import, certificate pinning and expiring HTTP-01 responses; operator-configured Debian Certbot hooks automate CA issuance/renewal and PFX deployment.
- Time/server-filtered CPU, memory, session and transfer-speed charts; reported devices by country; key metadata and downloads.

- Turkish, Arabic and English interface; Arabic RTL layout; light/dark themes with a sun/moon button.
- Overview, servers, access keys, VPN sessions, panel users, audit trail and account settings.
- Owner, admin, operator, access token writer and viewer roles; user/password management and protection of the last active administrator.
- Server/group registration; editable name, IP/FQDN, TCP port and group; maintenance mode. Credential rotation requires explicit confirmation and rejects stale confirmation reuse.
- VpnHood-compatible `vh://` keys; create 1–100 keys per operation; bulk revoke/enable/quota/expiry updates, search and pagination.
- Traffic, device and expiry limits; session termination decisions persist until the server acknowledges them.
- Live summary counters through SSE, server status, CSV usage export and contextual help dialogs.
- PostgreSQL FORCE RLS, tenant-aware foreign keys and append-only auditing; Argon2id, AES-256-GCM, CSRF/Origin checks and login attempt limits.

Bots have expiring/revocable API keys and a JSON management API using the panel's authorization checks. [API documentation](docs/yonetim-api.md).

## Architecture and repository

Rust/Tokio/Axum, PostgreSQL/SQLx and Leptos server-side rendering (SSR) form a modular application. Small JavaScript enhancements handle browser interaction; no browser WebAssembly or hydration is used. Nginx terminates HTTPS, Rust listens on loopback, and VpnHood servers call Hoodmin's `/api/agent/` endpoints.

- `crates/manager`: Hoodmin panel, API and local operator commands; Cargo package `hoodmin`.
- `crates/protocol`: VpnHood JSON/key formats and client proof validation.
- `crates/paste-compat`: compatibility shim using the maintained pastey macro.
- `migrations`: database schema and tenant policies.
- `scripts`: setup scripts and service definitions for dedicated Debian test containers.
- `tests`: real HTTPS/PostgreSQL/.NET integration checks in addition to Rust tests.
- `work`: private fixtures and build outputs, excluded from Git. Do not publish secrets.

Further documentation, currently in Turkish: [architecture](docs/mimari.md), [interface and languages](docs/arayuz-ve-diller.md), [console comparison](docs/console-karsilastirmasi.md).

## Build and validation

The validated environment is Debian 13 / PostgreSQL 17. Rust **1.99.1** is pinned in `rust-toolchain.toml` and dependencies in `Cargo.lock`. Rust/Cargo, OpenSSL development libraries and native build tools are required. .NET comparison checks also require SDK 10 and the sibling `VpnHood-Current` source tree; these are test tools, not the VPN server installation method.

Toolchain paths in the test container:

```bash
export CARGO_HOME=/opt/vpnhood-toolchain/cargo
export RUSTUP_HOME=/opt/vpnhood-toolchain/rustup
cargo build --release --locked
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo audit --deny warnings
```

`cargo audit` must be installed separately. The latest recorded checks passed 22 Rust tests, the real HTTPS/PostgreSQL/.NET panel/protocol/profile/report suites and theme/language HTTPS checks; this is not a general security guarantee.

A Debian 13 amd64 `.deb` package is available; fresh installation, upgrade, migration checksums and recovery have been tested. See the [production guide](docs/uretim-kurulumu.md) and [deployment record](docs/uretim-ortami.md). `scripts/setup-test-manager.sh` and `scripts/setup-test-tls.sh` target dedicated test environments only; do not blindly rerun them on an existing deployment. The TLS script creates a private, short-lived test CA rather than a production certificate solution. Consult the [VPS reverse-proxy notes](docs/vps-ters-vekil.md) for the legacy test deployment. Keep certificate verification enabled.

For packaged deployments use `hoodmin-create-owner EMAIL HTTPS_ORIGIN`, described in the production guide. In the legacy test environment, create the initial account with the local `create-account` command or `scripts/bootstrap-panel.sh TENANT_UUID`. The script stores its random initial password in `work/panel-admin.txt` with mode 0600 and does not reset an existing password. Consult the validation record for prerequisites of `scripts/test-contract.sh`, `tests/test_panel.py` and `tests/test_preferences.py`. Integration tests use separate tenants.

## Limitations and remaining work

- Real ACME issuance/renewal and PFX deployment are verified through an external Certbot worker. Tenant self-service AutoSign configuration is not implemented.
- Encrypted server/group address publishing supports HTTPS GET. External POST/PUT publishing jobs and automatic propagation through farm-level server profile assignment remain unimplemented.
- Client policies are advertised; closed-source commercial country-plan decisions are not claimed to be identical. Rewarded ads require a trusted server verifier, not an unverified client report or direct ad-network integration.
- Debian packaging, encrypted remote backups and isolated recovery work. Master-key rotation and external failure notifications remain unfinished. Job states persist, but interrupted installations are not retried automatically.
- Tenant creation uses CLI; public signup/platform administration is absent.
- Usage deltas lack report IDs; exactly-once accounting under interruptions is not guaranteed.
- Actual TCP/QUIC payloads, 1 Mbps shaping and network blocking were checked. The public VPN node carried UDP DNS through TCP, UDP and QUIC packet channels; server TUN/NAT counters confirmed forwarding. Full desktop routing, sustained load and every kernel-dependent setting need separate validation.

See the current [protocol compatibility record](docs/protokol-uyumlulugu.md). Console comparison documents retain their historical observations.

## Naming and installation compatibility

The product and main Cargo package are named **Hoodmin**. The existing test installation retains the `vpnhood-manager` executable/service, `vpnhood_manager` database, `vhmgr` system account, `VHM_*` settings and `__Host-vhm_*` cookies. The repository directory remains `VpnHood`. The new Debian package uses `/usr/bin/hoodmin`, `hoodmin.service` and `/var/lib/hoodmin`; the database role, environment variables and temporary SSH path remain compatible. It does not automatically migrate the legacy test installation. References to VpnHood identify the compatible protocol, server and client.
