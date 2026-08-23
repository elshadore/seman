pub mod client;
pub mod seman;
pub mod server;
pub mod init;

use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    ServerStart,
    ServerKill,
    ServerStatus,
    #[command(name = "defservice-start")]
    DefServiceStart {
        name: String,
        cmd: String,
    },
    #[command(name = "defservice")]
    DefService {
        name: String,
        cmd: String,
    },
    ServiceStart {
        name: String,
    },
    ServiceStop {
        name: String,
    },
    #[command(visible_alias = "services")]
    ServiceList {
        #[arg(long)]
        json: bool,
    },
    Timer {
        name: String,
        time: String,
        #[arg[num_args = 0..=1]]
        cmd: Option<String>,
    },
    #[command(visible_alias = "timers")]
    TimerList {
        #[arg(long)]
        json: bool,
    },
    TimerKill {
        name: String,
    },
    Exec {
        cmd: String,
    },
    Ping,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Response {
    Ok,
    OkMsg(String),
    Error(String),
}

pub const DEFAULT_PORT: &str = "127.0.0.1:7676";

pub fn resolve_port() -> String {
    if let Ok(p) = std::env::var("SEMAN_PORT") {
        if let Ok(port) = p.trim().parse::<u16>() {
            return format!("127.0.0.1:{port}");
        }
    }
    DEFAULT_PORT.to_string()
}
