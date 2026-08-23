use super::Command;
use super::Response;
use super::seman::SemanState;
use crate::seman::Seman;
use anyhow::{Context, Result};
use itertools::Itertools;
use log::{info, warn};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener as TokioTcpListener;
use tokio::net::TcpStream as TokioTcpStream;
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;

#[derive(Serialize)]
struct ServiceInfo {
    name: String,
    command: String,
    running: bool,
    exit_code: i32,
}

#[derive(Serialize)]
struct TimerInfo {
    name: String,
    command: Option<String>,
    remaining_seconds: f64,
}

async fn respond_to_client(buf: &mut TokioTcpStream, resp: Response) {
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

pub async fn execute(state: SemanState, cmd: Command) -> Response {
    let mut seman = state.lock().await;

    seman.sync();

    match cmd {
        Command::ServerStart => Response::Error("server already running".into()),
        Command::ServerKill => Response::Ok,
        Command::Ping => Response::OkMsg("pong".into()),
        Command::ServerStatus => Response::OkMsg("server: ok!".into()),
        Command::Exec { cmd } => match TokioCommand::new("sh").args(["-c", &cmd]).spawn() {
            Ok(proc) => Response::OkMsg(format!("{}", proc.id().unwrap_or(0))),
            Err(err) => Response::Error(format!(
                "failed to spawn the command: {cmd}, in command exec, err: {err}"
            )),
        },
        Command::DefServiceStart { name, cmd } => match seman.service_define(name, cmd, true).await
        {
            Ok(()) => Response::Ok,
            Err(err) => Response::Error(format!("errors during defservice-start: {err}")),
        },
        Command::DefService { name, cmd } => match seman.service_define(name, cmd, false).await {
            Ok(()) => Response::Ok,
            Err(err) => Response::Error(format!("errors during defservice: {err}")),
        },
        Command::ServiceStart { name } => match seman.service_start(name).await {
            Ok(()) => Response::Ok,
            Err(err) => Response::Error(format!("errors during service-start: {err}")),
        },
        Command::ServiceStop { name } => match seman.service_stop(name).await {
            Ok(()) => Response::Ok,
            Err(err) => Response::Error(format!("errors during service-stop: {err}")),
        },
        Command::ServiceList { json } => {
            if json {
                let infos: Vec<ServiceInfo> = seman
                    .iter_services()
                    .sorted_by(|(a, _), (b, _)| a.cmp(b))
                    .map(|(name, svc)| ServiceInfo {
                        name: name.clone(),
                        command: svc.cmd.clone(),
                        running: svc.proc.is_some(),
                        exit_code: svc.exit_code,
                    })
                    .collect();
                match serde_json::to_string(&infos) {
                    Ok(s) => Response::OkMsg(s),
                    Err(e) => Response::Error(format!("failed to serialize service list: {e}")),
                }
            } else {
                let mut result = String::new();
                for (i, (key, value)) in seman
                    .iter_services()
                    .sorted_by(|(a, _), (b, _)| a.cmp(b))
                    .enumerate()
                {
                    let status = if value.proc.is_some() {
                        "running".to_string()
                    } else {
                        "stopped".to_string()
                    };
                    result.push_str(
                        format!(
                            "[{i}] =>\n\tname: {key}\n\tcmd: {}\n\tstatus: {status}\n\tcode: {}\n",
                            value.cmd, value.exit_code
                        )
                        .as_str(),
                    );
                }
                Response::OkMsg(result)
            }
        }
        Command::Timer { name, time, cmd } => match humantime::parse_duration(&time) {
            Ok(duration) => {
                seman.timer_add(name, duration, cmd);
                Response::Ok
            }
            Err(err) => Response::Error(format!("invalid time format: {err}")),
        },
        Command::TimerKill { name } => {
            seman.timer_kill(name);
            Response::Ok
        }
        Command::TimerList { json } => {
            let now = Instant::now();
            if json {
                let infos: Vec<TimerInfo> = seman
                    .iter_timers()
                    .sorted_by(|a, b| {
                        a.deadline
                            .cmp(&b.deadline)
                            .then_with(|| a.name.cmp(&b.name))
                    })
                    .map(|t| TimerInfo {
                        name: t.name.clone(),
                        command: t.cmd.clone(),
                        remaining_seconds: t.deadline.saturating_duration_since(now).as_secs_f64(),
                    })
                    .collect();
                match serde_json::to_string(&infos) {
                    Ok(s) => Response::OkMsg(s),
                    Err(e) => Response::Error(format!("failed to serialize timer list: {e}")),
                }
            } else {
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
                Response::OkMsg(result)
            }
        }
    }
}

async fn handle_command(state: SemanState, buf: &mut TokioTcpStream, cmd: Command) {
    let is_kill = matches!(cmd, Command::ServerKill);
    let response = execute(state.clone(), cmd).await;
    respond_to_client(buf, response).await;
    if is_kill {
        let mut seman = state.lock().await;
        seman.kill_all_services().await;
        std::process::exit(0);
    }
}

/// This function should not error and handle all errors by logging.
/// We should make sure we never get any malformed state from here on out.
async fn handle_connection(mut stream: TokioTcpStream, state: SemanState) {
    let mut line = String::new();
    let mut buf = BufReader::new(&mut stream);

    match buf.read_line(&mut line).await {
        Ok(count) => {
            if count == 0 {
                return;
            }
        }
        Err(err) => {
            respond_to_client(
                buf.get_mut(),
                Response::Error(format!("malform line read: {err}")),
            )
            .await;
            return;
        }
    }

    let string = line.trim();
    info!("input from client: {string}");
    match serde_json::from_str(string) {
        Ok(cmd) => {
            info!("command parsed: {cmd:?}");
            handle_command(state, buf.get_mut(), cmd).await;
        }
        Err(err) => {
            respond_to_client(
                buf.get_mut(),
                Response::Error(format!(
                    "malformed message sent to the server: {string}, err: {err}"
                )),
            )
            .await
        }
    }
}

async fn server_loop(state: SemanState) -> Result<()> {
    let port = super::resolve_port();
    let listener = bind_reuse_port(&port).context(format!("failed to bind to port: {port}"))?;
    loop {
        let (stream, _) = listener.accept().await.context("tcp listening failed")?;
        let state = state.clone();
        tokio::spawn(handle_connection(stream, state));
    }
}

fn bind_reuse_port(port: &str) -> Result<TokioTcpListener> {
    use std::net::TcpListener;
    let std_listener = TcpListener::bind(port).context("failed to bind tcp listener")?;
    std_listener
        .set_nonblocking(true)
        .context("failed to set listener non-blocking")?;
    TokioTcpListener::from_std(std_listener).context("failed to convert listener")
}

fn init_logger() -> Result<()> {
    use simplelog::*;
    use time::format_description::FormatItem;
    use time::format_description::parse_borrowed;
    // Clanker code holy shit...
    let fmt: Vec<FormatItem<'static>> =
        parse_borrowed::<2>("[year]-[month]-[day] [hour]:[minute]:[second]")
            .context("invalid log time format")?;
    let fmt: &'static [FormatItem<'static>] = Box::leak(fmt.into_boxed_slice());
    let mut builder = ConfigBuilder::new();
    builder.set_time_format_custom(fmt);
    let _ = builder.set_time_offset_to_local();
    let config = builder.build();
    SimpleLogger::init(LevelFilter::max(), config).context("failed to initialize logger")?;
    Ok(())
}

pub fn run_server() -> Result<()> {
    init_logger()?;

    info!("server started");

    let state = Arc::new(Mutex::new(Seman::new()));
    let result = tokio::runtime::Runtime::new()
        .context("failed to initialize tokio runtime")?
        .block_on(async {
            super::init::run_init_file(state.clone()).await?;
            #[cfg(unix)]
            shutdown_on_signal(state.clone());
            server_loop(state).await
        });

    info!("server finished");

    result
}

#[cfg(unix)]
fn shutdown_on_signal(state: SemanState) {
    use tokio::signal::unix::{signal, SignalKind};
    let kinds = [SignalKind::interrupt(), SignalKind::terminate()];
    for kind in kinds {
        if let Ok(mut sig) = signal(kind) {
            let state = state.clone();
            tokio::spawn(async move {
                sig.recv().await;
                let mut seman = state.lock().await;
                seman.kill_all_services().await;
                std::process::exit(0);
            });
        }
    }
}
