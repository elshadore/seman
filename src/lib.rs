pub mod client;
pub mod server;

use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
pub enum ClientCommand {
    ServerStart,
    ServerKill,
    ServerStatus,
    DefServiceStart { name: String, cmd: String },
    DefService { name: String, cmd: String },
    ServiceStart { name: String },
    ServiceStop { name: String },
    ServiceList,
    Timer { name: String, time: String, cmd: String },
    Timers,
    TimerKill { name: String },
    Exec { cmd: String },
    Ping,
}

pub const SOCKET: &str = "/tmp/seman.sock";
