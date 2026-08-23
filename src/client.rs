use super::ClientCommand;
use anyhow::{Context, Result};
use std::os::unix::net::UnixStream;
use std::io::{Read, Write};

fn send_command(stream: &mut UnixStream, cmd: ClientCommand) -> Result<String> {
    let json = serde_json::to_string(&cmd)?;
    writeln!(stream, "{json}").context("failed to write message to server")?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("failed to read response from server")?;
    Ok(response)
}

pub fn server_command(cmd: ClientCommand) -> Result<()> {
    let mut stream = UnixStream::connect(super::SOCKET).context("server is not available")?;
    let response = send_command(&mut stream, cmd)?;
    print!("{response}");
    Ok(())
}

pub fn server_status() -> Result<()> {
    if let Ok(mut stream) = UnixStream::connect(super::SOCKET) {
        let response = send_command(&mut stream, ClientCommand::ServerStatus)?;
        print!("{response}");
    } else {
        println!("server: not found!")
    }
    Ok(())
}

