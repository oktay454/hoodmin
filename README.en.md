# Hoodmin

[Türkçe](README.md) · [العربية](README.ar.md) · [English](README.en.md)

Hoodmin is an **independent server and access management panel compatible with VpnHood**, written in Rust. It is not the official VpnHood Manager and does not reimplement the VPN server or client. Use the official prebuilt installer for the VpnHood server.

Each organization brings its own servers (BYOS). Each server belongs to exactly one tenant; there is no shared VPN server pool across tenants. Hoodmin does not carry VPN traffic: clients connect directly to their VPN servers.

## Status

The protocol core and tenant panel work. Compatibility with VpnHood **9.0.856** has been checked using real .NET contracts and a remote client connection. Production distribution and full feature parity with the closed-source Manager are unfinished. See the [validation record, in Turkish](docs/dogrulama.md).

## Available features

- Turkish, Arabic and English interface; Arabic RTL layout; light/dark themes with a sun/moon button.
- Overview, servers, access keys, VPN sessions, panel users, audit trail and account settings.
- Admin, operator and viewer roles; user/password management and protection of the last active administrator.
- Server/group registration; editable name, IP/FQDN, TCP port and group; maintenance mode. Credential rotation requires explicit confirmation and rejects stale confirmation reuse.
- VpnHood-compatible `vh://` keys; create 1–100 keys per operation; bulk revoke/enable/quota/expiry updates, search and pagination.
- Traffic, device and expiry limits; session termination decisions persist until the server acknowledges them.
- Live summary counters through SSE, server status, CSV usage export and contextual help dialogs.
- PostgreSQL FORCE RLS, tenant-aware foreign keys and append-only auditing; Argon2id, AES-256-GCM, CSRF/Origin checks and login attempt limits.

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

`cargo audit` must be installed separately. The latest recorded checks passed 13 Rust tests, the real HTTPS/PostgreSQL/.NET panel suite and 18 theme/language HTTPS checks; this is not a general security guarantee.

There is no `.deb` package yet. `scripts/setup-test-manager.sh` and `scripts/setup-test-tls.sh` target dedicated test environments only; do not blindly rerun them on an existing deployment. The TLS script creates a private, short-lived test CA rather than a production certificate solution. Consult the [VPS reverse-proxy notes](docs/vps-ters-vekil.md) for the current deployment. Keep certificate verification enabled.

Create the initial account with the local `create-account` command or `scripts/bootstrap-panel.sh TENANT_UUID`. The script stores its random initial password in `work/panel-admin.txt` with mode 0600 and does not reset an existing password. Consult the validation record for prerequisites of `scripts/test-contract.sh`, `tests/test_panel.py` and `tests/test_preferences.py`. Integration tests use separate tenants.

## Limitations and remaining work

- Apply the server connection configuration manually; SSH automation and a persistent job queue are absent.
- Groups currently organize servers; load balancing, group access keys and token URLs are absent.
- IP/FQDN edits do not update previously distributed static keys. Newly generated keys use the current address.
- ACME automation, plan/profile and speed-limit UI, bot API keys, Debian packaging and production backup/key-rotation procedures remain unfinished.
- Tenant creation is a local CLI operation. There is no public signup or platform administration UI. The schema allows multiple memberships, but the panel uses the account's default tenant.
- Usage reports lack a unique report identifier. Interruptions can cause undercounting; exactly-once accounting is not guaranteed. Quota enforcement depends on reporting intervals.
- The remote connection probe uses `NullVpnAdapter`; actual VPN traffic, OS routing, sustained load and performance have not been validated.

The [comparison document](docs/console-karsilastirmasi.md) distinguishes console features, architectural requirements and optional improvements. Listing a feature does not authorize its implementation.

## Naming and installation compatibility

The product and main Cargo package are named **Hoodmin**. The existing test installation retains the `vpnhood-manager` executable/service, `vpnhood_manager` database, `vhmgr` system account, `VHM_*` settings and `__Host-vhm_*` cookies. The repository directory remains `VpnHood`. A data- and key-preserving transition to Hoodmin installation names will be handled during Debian packaging. References to VpnHood identify the compatible protocol, server and client.
