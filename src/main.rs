use anyhow::Result;
use clap::Parser;
use seman::ClientCommand;

fn exec(cmd: ClientCommand) -> Result<()> {
    match cmd {
        ClientCommand::ServerStart => {
            seman::server::start_daemon()?;
        }
        ClientCommand::ServerStatus => {
            seman::client::server_status()?;
        }
        _ => {
            seman::client::server_command(cmd)?;
        }
    }
    Ok(())
}

pub fn main() {
    let cmd = ClientCommand::parse();
    if let Err(err) = exec(cmd) {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}
