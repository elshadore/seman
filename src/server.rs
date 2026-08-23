use super::Command;
use super::seman::SemanState;
use crate::seman::Seman;
use anyhow::{Context, Result};
use daemonize::Daemonize;
use itertools::Itertools;
use std::fs::File;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream as TokioUnixStream;
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;

async fn client_respond(buf: &mut TokioUnixStream, message: impl AsRef<str>) {
    _ = buf.write_all(message.as_ref().as_bytes()).await;
    _ = buf.write(b"\n").await;
}

async fn client_respond_error(buf: &mut TokioUnixStream, message: impl AsRef<str>) {
    let message = message.as_ref();
    client_respond(buf, message).await;
    eprintln!("{message}");
}

async fn handle_command(state: SemanState, buf: &mut TokioUnixStream, cmd: Command) {
    let mut seman = state.lock().await;

    if let Err(err) = seman.sync() {
        eprintln!("error: errors synchronizing state of seman: {err}");
    }

    match cmd {
        Command::ServerStart => {
            client_respond(buf, "error: unknown command").await;
        }
        Command::Ping => {
            client_respond(buf, "pong").await;
        }
        Command::ServerKill => {
            let _ = std::fs::remove_file("/tmp/seman.sock");
            std::process::exit(0);
        }
        Command::Exec { cmd } => match TokioCommand::new("sh").args(["-c", &cmd]).spawn() {
            Ok(proc) => {
                let id = proc.id().unwrap_or(0);
                client_respond(buf, format!("{id}")).await;
            }
            Err(err) => {
                eprintln!("error: failed to spawn the command: {cmd}, in command exec, err: {err}");
            }
        },
        Command::DefServiceStart { name, cmd } => {
            if let Err(err) = seman.service_define(name, cmd, true).await {
                client_respond_error(buf, format!("error: errors during defservice-start: {err}"))
                    .await;
            }
        }
        Command::DefService { name, cmd } => {
            if let Err(err) = seman.service_define(name, cmd, false).await {
                client_respond_error(buf, format!("error: errors during defservice: {err}")).await;
            }
        }
        Command::ServiceStart { name } => {
            if let Err(err) = seman.service_start(name).await {
                client_respond_error(buf, format!("error: errors during service-start: {err}"))
                    .await;
            }
        }
        Command::ServiceStop { name } => {
            if let Err(err) = seman.service_stop(name).await {
                client_respond_error(buf, format!("error: errors during service-stop: {err}"))
                    .await;
            }
        }
        Command::ServiceList => {
            let mut result = String::new();
            for (i, (key, value)) in seman
                .iter_services()
                .sorted_by(|(a, _), (b, _)| a.cmp(b))
                .enumerate()
            {
                let status = if value.proc.is_some() {
                    "running"
                } else {
                    "stopped"
                };
                result.push_str(
                    format!(
                        "[{i}] =>\n\tname: {key}\n\tcmd: {}\n\tstatus: {status}\n",
                        value.cmd
                    )
                    .as_str(),
                );
            }
            client_respond(buf, result).await;
        }
        Command::ServerStatus => {
            client_respond(buf, "server: ok!").await;
        }
        Command::Timer { name, time, cmd } => match humantime::parse_duration(&time) {
            Ok(duration) => {
                seman.timer_add(name, duration, cmd);
            }
            Err(err) => {
                client_respond(buf, format!("error: invalid time format: {err}")).await;
            }
        },
        Command::TimerKill { name } => {
            seman.timer_kill(name);
        }
        Command::TimerList => {
            let now = Instant::now();
            let mut result = String::new();
            for (i, timer) in seman
                .iter_timers()
                .sorted_by(|a, b| {
                    a.deadline
                        .cmp(&b.deadline)
                        .then_with(|| a.name.cmp(&b.name))
                })
                .enumerate()
            {
                let remaining = timer.deadline.saturating_duration_since(now);
                let empty_string = String::new();
                let cmd = timer.cmd.as_ref().unwrap_or(&empty_string);
                result.push_str(
                    format!(
                        "[{i}] =>\n\tname: {}\n\tcmd: {}\n\ttime: {:?}\n",
                        timer.name, cmd, remaining
                    )
                    .as_str(),
                );
            }
            client_respond(buf, result).await;
        }
    }
}

/// This function should not error and handle all errors by logging.
/// We should make sure we never get any malformed state from here on out.
async fn handle_connection(mut stream: TokioUnixStream, state: SemanState) {
    let mut line = String::new();
    let mut buf = BufReader::new(&mut stream);

    match buf.read_line(&mut line).await {
        Ok(count) => {
            if count == 0 {
                return;
            }
        }
        Err(err) => {
            client_respond_error(buf.get_mut(), format!("error: malform line read: {err}")).await;
            return;
        }
    }

    let string = line.trim();
    println!("input-from-client: {string}");
    match serde_json::from_str(string) {
        Ok(cmd) => {
            println!("command-parsed: {cmd:?}");
            handle_command(state, buf.get_mut(), cmd).await;
        }
        Err(err) => {
            client_respond_error(
                buf.get_mut(),
                format!("error: malformed message sent to the server: {string}, err: {err}"),
            )
            .await
        }
    }
}

async fn server_loop() -> Result<()> {
    let _ = std::fs::remove_file(super::SOCKET);
    let listener =
        tokio::net::UnixListener::bind(super::SOCKET).context("failed to bind to unix socked")?;
    let state = Arc::new(Mutex::new(Seman::new()));
    loop {
        let (stream, _) = listener.accept().await.context("socket listening failed")?;
        let state = state.clone();
        tokio::spawn(handle_connection(stream, state));
    }
}

pub fn start_daemon() -> Result<()> {
    let stdout = File::create("/tmp/seman.out")
        .context("failed to open daemon stdout file: /tmp/seman.out")?;

    let stderr = File::create("/tmp/seman.err")
        .context("failed to open daemon stderr file: /tmp/seman.err")?;

    Daemonize::new()
        .pid_file("/tmp/seman.pid")
        .stdout(stdout)
        .stderr(stderr)
        .start()
        .context("failed to start server daemon")?;
    
    println!("server-started!");
    
    tokio::runtime::Runtime::new()
        .context("failed to initialize tokio runtime")?
        .block_on(server_loop())
}
