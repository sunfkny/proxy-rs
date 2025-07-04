use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Show status of Mihomo")]
    Status,
    #[command(about = "Start Mihomo", alias="run")]
    Start {
        #[arg(
            value_name = "URL",
            help = "URL to download subscription config file."
        )]
        url: Option<String>,
    },
    #[command(about = "Stop Mihomo by killing the process")]
    Stop,
    #[command(about = "Tunnel localhost:<port> through a free service")]
    Tunnel {
        #[arg(value_name = "PORT", help = "Port to tunnel through a free service")]
        port: u16,
    },
}
