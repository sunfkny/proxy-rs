pub mod cli;
pub mod config;
pub mod downloader;
pub mod mihomo;
pub mod proxy_selector;
pub mod tunnel;
pub mod utils;

use std::env;

use crate::cli::{Cli, Commands};
use crate::mihomo::MihomoManager;
use crate::tunnel::try_tunnel_service;
use anyhow::Ok;
use clap::Parser;
use log::*;

fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }
    env_logger::init();
    let cli = Cli::parse();
    let manager = MihomoManager::new().unwrap_or_else(|e| {
        error!("Failed to initialize: {e}");
        std::process::exit(1);
    });

    let result = match cli.command {
        Some(Commands::Status) => manager.status(),
        Some(Commands::Start { url }) => manager.start(url.as_deref()),
        Some(Commands::Stop) => manager.stop(),
        Some(Commands::Tunnel { port }) => try_tunnel_service(port),
        None => Ok(()),
    };

    if let Err(e) = result {
        error!("An error occurred: {e}");
        std::process::exit(1);
    }
}
