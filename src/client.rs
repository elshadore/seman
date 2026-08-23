use super::{Command, Response};
use anyhow::{Context, Result, bail};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

fn send_command(stream: &mut UnixStream, cmd: Command) -> Result<String> {
    let json = serde_json::to_string(&cmd)?;
    writeln!(stream, "{json}").context("failed to write message to server")?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("failed to read response from server")?;
    Ok(response)
}

fn handle_response(response: String) -> Result<()> {
    let resp: Response = serde_json::from_str(response.trim()).unwrap_or(Response::Ok);
    match resp {
        Response::Ok => Ok(()),
        Response::OkMsg(msg) => {
            print!("{msg}");
            Ok(())
        }
        Response::Error(msg) => bail!(msg),
    }
}

pub fn server_command(cmd: Command) -> Result<()> {
    let mut stream = UnixStream::connect(super::SOCKET).context("server is not available")?;
    let response = send_command(&mut stream, cmd)?;
    handle_response(response)
}

pub fn server_kill_if_running() {
    let _ = server_command(Command::ServerKill);
}

pub fn server_status() -> Result<()> {
    if let Ok(mut stream) = UnixStream::connect(super::SOCKET) {
        let response = send_command(&mut stream, Command::ServerStatus)?;
        handle_response(response)?;
    } else {
        println!("server: not found!")
    }
    Ok(())
}
