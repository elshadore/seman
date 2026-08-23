pub mod client;
pub mod server;
pub mod seman;

use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    ServerStart,
    ServerStartKill,
    ServerKill,
    ServerStatus,
    #[command(name  = "defservice-start")]
    DefServiceStart { name: String, cmd: String },
    #[command(name  = "defservice")]
    DefService { name: String, cmd: String },
    ServiceStart { name: String },
    ServiceStop { name: String },
    #[command(visible_alias = "services")]
    ServiceList,
    Timer { name: String, time: String, cmd: String },
    #[command(visible_alias = "timers")]
    TimerList,
    TimerKill { name: String },
    Exec { cmd: String },
    Ping,
}

pub const SOCKET: &str = "/tmp/seman.sock";
