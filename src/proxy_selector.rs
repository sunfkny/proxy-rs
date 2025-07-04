use log::*;
use reqwest::blocking::Client;
use std::time::Duration;

static DIRECT_CONNECTION: &str = "Direct connection";

static GITHUB_PROXIES: &[&str] = &[
    "", // Direct connection
    "https://github.akams.cn/",
    "https://github.moeyy.xyz/",
    "https://tvv.tw/",
];

static GITHUB_SPEEDTEST_URL: &str =
    "https://raw.githubusercontent.com/microsoft/vscode/main/LICENSE.txt";

pub fn select_fastest_github_proxy() -> anyhow::Result<&'static str> {
    info!("Selecting fastest GitHub proxy...");

    let client = Client::builder().timeout(Duration::from_secs(3)).build()?;

    let results: Vec<(&'static str, Duration)> = GITHUB_PROXIES
        .iter()
        .filter_map(|proxy| {
            let url = format!("{}{}", proxy, GITHUB_SPEEDTEST_URL);
            let start_time = std::time::Instant::now();

            let proxy_name = if proxy.is_empty() {
                DIRECT_CONNECTION
            } else {
                proxy
            };
            match client.get(&url).send() {
                Ok(response) if response.status().is_success() => {
                    let elapsed = start_time.elapsed();
                    info!("{proxy_name} time: {elapsed:?}");
                    Some((*proxy, elapsed))
                }
                _ => {
                    info!("{proxy_name} is not available");
                    None
                }
            }
        })
        .collect();

    if let Some((fastest_proxy, _)) = results.iter().min_by_key(|(_, t)| *t) {
        let proxy_name = if fastest_proxy.is_empty() {
            DIRECT_CONNECTION
        } else {
            fastest_proxy
        };
        info!("Fastest GitHub proxy: {proxy_name}");
        Ok(*fastest_proxy)
    } else {
        error!("No GitHub proxy available");
        Err(anyhow::anyhow!("No GitHub proxy available"))
    }
}
