use crate::utils::ask_for_confirmation;
use anyhow::Result;
use log::*;
use std::process::Command;
struct TunnelService<'a> {
    name: &'static str,
    command: Vec<&'a str>,
}

const SSH_DEFAULT_PARAMS: [&'static str; 6] = [
    "-o",
    "StrictHostKeyChecking=no",
    "-o",
    "ServerAliveInterval=30",
    "-o",
    "ConnectTimeout=5",
];

pub fn try_tunnel_service(port: u16) -> Result<()> {
    if which::which("ssh").is_err() {
        error!("SSH is not installed. Please install it and try again.");
        return Ok(());
    }

    info!("Tunneling the WebUI through a free service...");
    info!("Note: You can usually access the WebUI at https://<the-service-random-subdomain>/ui");
    info!(
        "Use https://<the-service-random-subdomain>/ as the control server address in the WebUI."
    );

    let port_forward_80 = &format!("-R80:localhost:{}", port);
    let port_forward_0 = &format!("-R0:localhost:{}", port);

    let services = vec![
        TunnelService {
            name: "localhost.run",
            command: vec!["ssh"]
                .into_iter()
                .chain(SSH_DEFAULT_PARAMS)
                .chain(vec![port_forward_80, "nokey@localhost.run"])
                .collect(),
        },
        TunnelService {
            name: "serveo.net",
            command: vec!["ssh"]
                .into_iter()
                .chain(SSH_DEFAULT_PARAMS)
                .chain(vec![port_forward_80, "serveo.net"])
                .collect(),
        },
        TunnelService {
            name: "pinggy.io",
            command: vec!["ssh", "-p", "443"]
                .into_iter()
                .chain(SSH_DEFAULT_PARAMS)
                .chain(vec!["-t", port_forward_0, "a.pinggy.io", "x:passpreflight"])
                .collect(),
        },
    ];

    for service in services {
        info!("Try tunneling through {}...", service.name);
        let mut cmd = Command::new(&service.command[0]);
        cmd.args(&service.command[1..]);

        let status = cmd.status();

        let exit_code = status.map(|s| s.code().unwrap_or(1)).unwrap_or(1);
        warn!(
            "Tunneling through {} exited with code {}. We don't know if it was successful.",
            service.name, exit_code
        );

        if !ask_for_confirmation("Do you want to try next service? Press n if you want to exit.") {
            break;
        }
    }

    Ok(())
}
