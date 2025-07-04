use crate::utils::ask_for_confirmation;
use anyhow::Result;
use log::*;
use reqwest::blocking::Client;
use serde_yaml::Value;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

const MIHOMO_USER_AGENT: &str = "mihomo.proxy.sh/v1.0 (clash.meta)";

pub fn handle_subscription_config(
    client: &Client,
    subscription_url: Option<&str>,
    config_path: &Path,
) -> Result<()> {
    if let Some(url) = subscription_url {
        download_subscription(client, url, config_path)?;
    } else if !is_config_valid(config_path) {
        if ask_for_confirmation(
            "No valid config file found. Do you want to input config content manually?",
        ) {
            if !read_config_from_stdin(config_path) {
                warn!("No valid content input, keeping existing config file unchanged");
            }
        } else {
            warn!( "Skipping config input. You may need to put your subscription file at proxy-data/config/config.yaml and restart Mihomo.");
        }
    } else {
        info!("Valid config file already exists");
    }
    Ok(())
}

fn download_subscription(client: &Client, url: &str, config_path: &Path) -> Result<()> {
    info!("Downloading subscription from URL...");
    if !url.starts_with("http://") && !url.starts_with("https://") {
        warn!("URL does not start with http:// or https:// prefix. Skipping download.");
        return Ok(());
    }

    let response = client
        .get(url)
        .header("User-Agent", MIHOMO_USER_AGENT)
        .send()?
        .error_for_status()?;

    let content = response.text()?;
    fs::write(config_path, content)?;
    info!("Downloaded to {}", config_path.display());
    Ok(())
}

fn is_config_valid(config_path: &Path) -> bool {
    if !config_path.exists() || !config_path.is_file() {
        return false;
    }
    if let Ok(content) = fs::read_to_string(config_path) {
        if let Ok(yaml) = serde_yaml::from_str::<Value>(&content) {
            if let Some(map) = yaml.as_mapping() {
                return map.contains_key("proxies")
                    || map.contains_key("proxy-groups")
                    || map.contains_key("rules");
            }
        }
    }
    false
}

fn read_config_from_stdin(config_path: &Path) -> bool {
    info!("Please input your config content below (press Ctrl+D on a new line to finish):");
    let mut buffer = String::new();
    if io::stdin().read_to_string(&mut buffer).is_ok() && !buffer.trim().is_empty() {
        if let Ok(mut file) = File::create(config_path) {
            if file.write_all(buffer.as_bytes()).is_ok() {
                info!("Config saved to {}", config_path.display());
                return true;
            }
        }
    }
    false
}

pub fn parse_mixed_port(config_path: &Path) -> Option<u16> {
    if let Ok(content) = fs::read_to_string(config_path) {
        if let Ok(yaml) = serde_yaml::from_str::<Value>(&content) {
            if let Some(map) = yaml.as_mapping() {
                if let Some(port) = map.get("mixed-port") {
                    return port.as_u64().map(|p| p as u16);
                }
                if let Some(port) = map.get("port") {
                    return port.as_u64().map(|p| p as u16);
                }
            }
        }
    }
    None
}

pub fn update_mixed_port(config_path: &Path, new_port: u16) -> Result<()> {
    let content = fs::read_to_string(config_path)?;
    let mut yaml = serde_yaml::from_str::<Value>(&content)?;
    let map = yaml
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("Invalid YAML"))?;
    map.insert("mixed-port".into(), new_port.into());
    fs::write(config_path, serde_yaml::to_string(&yaml)?)?;
    Ok(())
}
pub fn update_external_controller(config_path: &Path, external_controller: &str) -> Result<()> {
    let content = fs::read_to_string(config_path)?;
    let mut yaml = serde_yaml::from_str::<Value>(&content)?;
    let map = yaml
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("Invalid YAML"))?;
    map.insert("external-controller".into(), external_controller.into());
    fs::write(config_path, serde_yaml::to_string(&yaml)?)?;
    Ok(())
}
