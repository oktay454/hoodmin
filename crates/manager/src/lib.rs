#![recursion_limit = "512"]

mod acme;
mod agent;
mod auth;
mod certificates;
mod crypto;
mod farms;
mod i18n;
mod install;
mod management;
mod policy;
mod publication;
mod readmodels;
mod reports;
mod server_profiles;
mod store;
mod web;

mod cli;
mod cli_backup;
mod cli_database;
mod cli_http;
mod config;
mod legacy;
mod rotation;
mod runtime;

pub use cli::run as run_cli;
pub use runtime::run as run_server;

mod retention;

mod notifications;

mod frontend;
