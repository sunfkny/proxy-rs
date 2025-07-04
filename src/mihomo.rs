use crate::config::{handle_subscription_config, update_external_controller, update_mixed_port};
use crate::downloader::{decompress_gz, decompress_zip, download_file_with_progress, unzip_file};
use crate::proxy_selector::select_fastest_github_proxy;
use crate::utils::find_unused_port;
use anyhow::{anyhow, Context, Ok, Result};
use log::*;
use reqwest::blocking::Client;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
const PROXY_DATA_DIR: &str = "proxy-data";
const MIHOMO_PID_FILE: &str = "proxy-data/mihomo.pid";

pub struct MihomoManager {
    client: Client,
    proxy_data_dir: PathBuf,
    config_dir: PathBuf,
    mihomo_path: PathBuf,
}

enum ArchiveType {
    GZ,
    ZIP,
}
impl ArchiveType {
    fn as_str(&self) -> &'static str {
        match self {
            ArchiveType::GZ => "gz",
            ArchiveType::ZIP => "zip",
        }
    }
}

impl std::fmt::Display for ArchiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl MihomoManager {
    pub fn new() -> Result<Self> {
        let proxy_data_dir = PathBuf::from(PROXY_DATA_DIR);
        let config_dir = proxy_data_dir.join("config");
        let mihomo_path = proxy_data_dir.join(if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        });
        fs::create_dir_all(&proxy_data_dir)?;
        fs::write(proxy_data_dir.join(".gitignore"), "*\n")?;
        fs::create_dir_all(&config_dir)?;

        Ok(Self {
            client: Client::new(),
            proxy_data_dir,
            config_dir,
            mihomo_path,
        })
    }

    pub fn start(&self, url: Option<&str>) -> Result<()> {
        if let Some(pid) = self.is_running()? {
            info!("Mihomo is already running (pid: {pid}). Stopping it first...");
            self.stop()?;
        }

        if !self.mihomo_path.exists() {
            self.download_mihomo()?;
        }

        self.download_metacubexd_if_necessary()?;
        self.download_geodata_if_necessary()?;

        let config_path = self.config_dir.join("config.yaml");
        handle_subscription_config(&self.client, url, &config_path)?;

        let ext_port = find_unused_port(9090).context("Failed to find an unused port")?;
        info!("Found unused port: {ext_port}");

        let metacubexd_path = self.proxy_data_dir.join("metacubexd");
        let absolute_metacubexd_path = dunce::canonicalize(metacubexd_path)?;

        let mut command = Command::new(&self.mihomo_path);
        command
            .arg("-d")
            .arg(&self.config_dir)
            .arg("-ext-ctl")
            .arg(format!("127.0.0.1:{}", ext_port))
            .arg("-ext-ui")
            .arg(absolute_metacubexd_path);

        let log_file = File::create(self.proxy_data_dir.join("mihomo.log"))?;
        let stdout = Stdio::from(log_file);
        let stderr = Stdio::from(File::create(self.proxy_data_dir.join("mihomo.err"))?);

        let child = command.stdout(stdout).stderr(stderr).spawn()?;
        self.save_pid(child)?;

        info!("Mihomo started in the background!");

        let mixed_port = find_unused_port(7890).context("Failed to find unused port")?;

        update_mixed_port(&config_path, mixed_port)?;
        info!("Mihomo mixed-port is set to: {mixed_port}");
        update_external_controller(&config_path, &format!("127.0.0.1:{}", ext_port))?;
        info!("Web UI: http://127.0.0.1:{ext_port}/ui");

        self.write_env_setup_script(mixed_port)?;

        info!(
            "To stop Mihomo, run: `{} stop`",
            std::env::current_exe()?
                .as_path()
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("<executable>")
        );

        Ok(())
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        let pid = self.load_pid();
        let system = sysinfo::System::new_all();
        let process = pid.and_then(|pid| system.process(pid));

        match process {
            Some(p) => {
                p.kill_and_wait()
                    .map_err(|e| anyhow!("Failed to stop: {e:?}"))?;
                info!("Mihomo stopped.");
            }
            None => {
                warn!("Mihomo process not found.");
            }
        }
        let _ = fs::remove_file(MIHOMO_PID_FILE);
        Ok(())
    }

    pub fn status(&self) -> anyhow::Result<()> {
        if let Some(pid) = self.is_running()? {
            info!("Mihomo is running (pid: {pid}).");
        } else {
            info!("Mihomo is not running.");
        }
        Ok(())
    }

    fn is_running(&self) -> Result<Option<u32>, anyhow::Error> {
        if let Some(pid) = self.load_pid() {
            let system = sysinfo::System::new_all();
            let process = system.process(pid);
            match process {
                Some(_) => Ok(Some(pid.as_u32())),
                None => {
                    let _ = fs::remove_file(MIHOMO_PID_FILE);
                    return Ok(None);
                }
            }
        } else {
            Ok(None)
        }
    }

    fn save_pid(&self, child: Child) -> Result<()> {
        fs::write(MIHOMO_PID_FILE, child.id().to_string())?;
        Ok(())
    }
    fn load_pid(&self) -> Option<sysinfo::Pid> {
        fs::read_to_string(MIHOMO_PID_FILE)
            .ok()
            .and_then(|pid_str| sysinfo::Pid::from_str(&pid_str).ok())
    }

    fn download_mihomo(&self) -> Result<()> {
        info!("Downloading Mihomo...");
        let proxy = select_fastest_github_proxy()?;

        let version_url = format!(
            "{}https://github.com/MetaCubeX/mihomo/releases/latest/download/version.txt",
            proxy
        );
        let version = self
            .client
            .get(&version_url)
            .send()?
            .text()?
            .trim()
            .to_string();
        info!("Latest version: {version}");

        let os = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            return Err(anyhow!("Unsupported OS"));
        };

        let arch = if cfg!(target_arch = "x86_64") {
            "amd64"
        } else if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            return Err(anyhow!("Unsupported architecture"));
        };

        let archive_type = if cfg!(windows) {
            ArchiveType::ZIP
        } else {
            ArchiveType::GZ
        };

        let download_url = format!(
            "{}https://github.com/MetaCubeX/mihomo/releases/download/{}/mihomo-{}-{}-{}.{}",
            proxy, version, os, arch, version, archive_type
        );

        let archive_path = self.proxy_data_dir.join(format!("mihomo.{archive_type}"));
        download_file_with_progress(&self.client, &download_url, &archive_path)?;

        match archive_type {
            ArchiveType::GZ => decompress_gz(&archive_path, &self.mihomo_path)?,
            ArchiveType::ZIP => decompress_zip(&archive_path, &self.mihomo_path)?,
        };

        fs::remove_file(&archive_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.mihomo_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&self.mihomo_path, perms)?;
        }

        Ok(())
    }

    fn download_metacubexd_if_necessary(&self) -> Result<()> {
        let metacubexd_path = self.proxy_data_dir.join("metacubexd");
        if metacubexd_path.exists() {
            info!("metacubexd already exists, skip downloading.");
            return Ok(());
        }

        info!("Downloading metacubexd...");
        let proxy = select_fastest_github_proxy()?;
        let url = format!(
            "{}https://github.com/MetaCubeX/metacubexd/archive/refs/heads/gh-pages.zip",
            proxy
        );
        let zip_path = self.proxy_data_dir.join("metacubexd.zip");

        download_file_with_progress(&self.client, &url, &zip_path)?;
        unzip_file(&zip_path, &self.proxy_data_dir)?;
        fs::remove_file(&zip_path)?;

        // The unzipped folder is named metacubexd-gh-pages, rename it
        let unzipped_folder = self.proxy_data_dir.join("metacubexd-gh-pages");
        if unzipped_folder.exists() {
            fs::rename(unzipped_folder, metacubexd_path)?;
        }

        Ok(())
    }

    fn download_geodata_if_necessary(&self) -> Result<()> {
        self.download_geofile("geosite.dat")?;
        self.download_geofile("geoip.dat")?;
        Ok(())
    }

    fn download_geofile(&self, filename: &str) -> Result<()> {
        let file_path = self.config_dir.join(filename);
        if file_path.exists() {
            return Ok(());
        }

        info!("Downloading {filename}...");
        let proxy = select_fastest_github_proxy()?;
        let url = format!(
            "{}https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/{}",
            proxy, filename
        );

        if download_file_with_progress(&self.client, &url, &file_path).is_err() {
            warn!("Failed to download {filename}");
        }
        Ok(())
    }

    fn write_env_setup_script(&self, mixed_port: u16) -> Result<()> {
        let on_script_path = self.proxy_data_dir.join("on");
        let off_script_path = self.proxy_data_dir.join("off");

        let on_content = format!(
            r#"#!/bin/sh
export http_proxy="http://127.0.0.1:{}"
export HTTP_PROXY=$http_proxy
export https_proxy=$http_proxy
export HTTPS_PROXY=$http_proxy
export all_proxy=$http_proxy
export ALL_PROXY=$http_proxy
"#,
            mixed_port
        );

        let off_content = r#"#!/bin/sh
unset http_proxy HTTP_PROXY https_proxy HTTPS_PROXY all_proxy ALL_PROXY
"#;

        fs::write(on_script_path, on_content)?;
        fs::write(off_script_path, off_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                self.proxy_data_dir.join("on"),
                fs::Permissions::from_mode(0o755),
            )?;
            fs::set_permissions(
                self.proxy_data_dir.join("off"),
                fs::Permissions::from_mode(0o755),
            )?;
        }

        info!("Environment setup scripts written to proxy-data/on and proxy-data/off");
        info!("You can `source proxy-data/on` to set up the proxy environment, and `source proxy-data/off` to unset it.");

        Ok(())
    }
}
