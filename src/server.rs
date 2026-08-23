use super::Command;
use super::Response;
use super::seman::SemanState;
use crate::seman::Seman;
use log::{debug, error, info, warn};
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

async fn respond(buf: &mut TokioUnixStream, resp: Response) {
    let mut s = match serde_json::to_string(&resp) {
        Ok(s) => s,
        Err(_) => "{\"Error\":\"failed to serialize response\"}".to_string(),
    };
    s.push('\n');
    let _ = buf.write_all(s.as_bytes()).await;
    if let Response::Error(msg) = &resp {
        warn!("response error: {msg}");
    }
}

macro_rules! respond_ok {
    ($buf:expr, $($arg:tt)*) => {
        respond($buf, Response::OkMsg(format!($($arg)*)))
    };
}

macro_rules! respond_err {
    ($buf:expr, $($arg:tt)*) => {
        respond($buf, Response::Error(format!($($arg)*)))
    };
}

async fn handle_command(state: SemanState, buf: &mut TokioUnixStream, cmd: Command) {
    let mut seman = state.lock().await;
    
    seman.sync();

    match cmd {
        Command::ServerStart => {
            respond_err!(buf, "unknown command").await;
        }
        Command::Ping => {
            respond_ok!(buf, "pong").await;
        }
        Command::ServerKill => {
            let _ = std::fs::remove_file("/tmp/seman.sock");
            respond(buf, Response::Ok).await;
            std::process::exit(0);
        }
        Command::Exec { cmd } => match TokioCommand::new("sh").args(["-c", &cmd]).spawn() {
            Ok(proc) => {
                let id = proc.id().unwrap_or(0);
                respond_ok!(buf, "{}", id).await;
            }
            Err(err) => {
                respond_err!(
                    buf,
                    "failed to spawn the command: {cmd}, in command exec, err: {err}"
                )
                .await;
            }
        },
        Command::DefServiceStart { name, cmd } => {
            if let Err(err) = seman.service_define(name, cmd, true).await {
                respond_err!(buf, "errors during defservice-start: {err}").await;
            } else {
                respond(buf, Response::Ok).await;
            }
        }
        Command::DefService { name, cmd } => {
            if let Err(err) = seman.service_define(name, cmd, false).await {
                respond_err!(buf, "errors during defservice: {err}").await;
            } else {
                respond(buf, Response::Ok).await;
            }
        }
        Command::ServiceStart { name } => {
            if let Err(err) = seman.service_start(name).await {
                respond_err!(buf, "errors during service-start: {err}").await;
            } else {
                respond(buf, Response::Ok).await;
            }
        }
        Command::ServiceStop { name } => {
            if let Err(err) = seman.service_stop(name).await {
                respond_err!(buf, "errors during service-stop: {err}").await;
            } else {
                respond(buf, Response::Ok).await;
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
            respond_ok!(buf, "{}", result).await;
        }
        Command::ServerStatus => {
            respond_ok!(buf, "server: ok!").await;
        }
        Command::Timer { name, time, cmd } => match humantime::parse_duration(&time) {
            Ok(duration) => {
                seman.timer_add(name, duration, cmd);
                respond(buf, Response::Ok).await;
            }
            Err(err) => {
                respond_err!(buf, "invalid time format: {err}").await;
            }
        },
        Command::TimerKill { name } => {
            seman.timer_kill(name);
            respond(buf, Response::Ok).await;
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
            respond_ok!(buf, "{}", result).await;
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
            respond_err!(buf.get_mut(), "malform line read: {err}").await;
            return;
        }
    }

    let string = line.trim();
    debug!("input-from-client: {string}");
    match serde_json::from_str(string) {
        Ok(cmd) => {
            debug!("command-parsed: {cmd:?}");
            handle_command(state, buf.get_mut(), cmd).await;
        }
        Err(err) => {
            respond_err!(
                buf.get_mut(),
                "malformed message sent to the server: {string}, err: {err}"
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

fn init_logger() -> Result<()> {
    use simplelog::*;
    use time::format_description::parse_borrowed;
    let log_path = "/tmp/seman.log";
    let file = File::create(log_path).context("failed to open log file: {log_path}")?;
    let fmt: Vec<time::format_description::FormatItem<'static>> =
        parse_borrowed::<2>("[year]-[month]-[day] [hour]:[minute]:[second]")
            .context("invalid log time format")?;
    let fmt: &'static [time::format_description::FormatItem<'static>] =
        Box::leak(fmt.into_boxed_slice());
    let mut builder = ConfigBuilder::new();
    builder.set_time_format_custom(fmt);
    let _ = builder.set_time_offset_to_local();
    let config = builder.build();
    let level = std::env::var("SEMAN_LOG_LEVEL")
        .ok()
        .and_then(|s| s.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::Info);
    WriteLogger::init(level, config, file).context("failed to initialize logger")?;
    Ok(())
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

    init_logger()?;
    
    info!("server started");

    let result = tokio::runtime::Runtime::new()
        .context("failed to initialize tokio runtime")?
        .block_on(server_loop());
    
    info!("server finished");
    
    result
}
