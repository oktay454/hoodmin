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
mod reports;
mod server_profiles;
mod store;
mod views;
mod web;

mod cli;
mod cli_backup;
mod cli_database;
mod cli_http;
mod config;
mod legacy;
mod runtime;

pub use cli::run as run_cli;
pub use runtime::run as run_server;
