use anyhow::Result;
use clap::Parser;
use seman::Command;

fn exec(cmd: Command) -> Result<()> {
    match cmd {
        Command::ServerStart => {
            seman::server::run_server()?;
        }
        Command::ServerStatus => {
            seman::client::server_status()?;
        }
        _ => {
            seman::client::server_command(cmd)?;
        }
    }
    Ok(())
}

pub fn main() {
    let cmd = Command::parse();
    if let Err(err) = exec(cmd) {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}
