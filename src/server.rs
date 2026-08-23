use super::ClientCommand;
use anyhow::{Context, Result, bail};
use daemonize::Daemonize;
use itertools::Itertools;
use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream as TokioUnixStream;
use tokio::process::Child as TokioProcess;
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct Service {
    pub cmd: String,
    pub proc: Option<TokioProcess>,
}

impl Service {
    pub fn new(cmd: String, proc: Option<TokioProcess>) -> Self {
        Self { cmd, proc }
    }

    pub async fn start(&mut self) -> Result<()> {
        self.kill().await?;

        let proc = TokioCommand::new("sh").args(["-c", &self.cmd]).spawn()?;

        self.proc = Some(proc);

        Ok(())
    }

    pub fn sync(&mut self) -> Result<()> {
        if let Some(mut proc) = self.proc.take() {
            match proc.try_wait()? {
                None => self.proc = Some(proc),
                Some(_) => {}
            }
        }
        Ok(())
    }

    pub async fn kill(&mut self) -> Result<()> {
        if let Some(mut proc) = self.proc.take() {
            proc.kill().await?;
        }
        Ok(())
    }
}

pub struct Timer {
    pub name: String,
    pub cmd: String,
    pub duration: Duration,
    pub handle: JoinHandle<()>,
}

impl Timer {
    pub fn new(name: String, cmd: String, duration: Duration, handle: JoinHandle<()>) -> Self {
        Self {
            name,
            cmd,
            duration,
            handle,
        }
    }
}

pub struct Seman {
    services: HashMap<String, Service>,
    timers: Vec<Timer>,
}

impl Seman {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
            timers: Vec::new(),
        }
    }

    pub async fn service_define(&mut self, name: String, cmd: String, start: bool) -> Result<()> {
        let mut service = Service::new(cmd, None);
        if start {
            service.start().await?;
        }
        if let Some(mut result) = self.services.insert(name, service) {
            result.kill().await?;
        }
        Ok(())
    }

    pub async fn service_start(&mut self, name: String) -> Result<()> {
        if let Some(result) = self.services.get_mut(&name) {
            result.start().await?;
            Ok(())
        } else {
            bail!("service: {name}, does not exist, and so cannot be started!")
        }
    }

    pub async fn service_stop(&mut self, name: String) -> Result<()> {
        if let Some(result) = self.services.get_mut(&name) {
            result.kill().await?;
            Ok(())
        } else {
            bail!("service: {name}, does not exist, and so cannot be stopped!")
        }
    }

    pub fn service_sync(&mut self) -> Result<()> {
        for (_, service) in self.services.iter_mut() {
            service.sync()?;
        }
        Ok(())
    }

    pub fn iter_services(&self) -> impl Iterator<Item = (&String, &Service)> {
        self.services.iter()
    }

    pub fn timer_add(&mut self, name: String, duration: Duration, cmd: String) {
        let cmd1 = cmd.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let _ = TokioCommand::new("sh").args(["-c", &cmd1]).output().await;
        });
        self.timers.push(Timer::new(name, cmd, duration, handle));
    }

    pub fn timer_kill(&mut self, name: impl AsRef<str>) {
        let name = name.as_ref();
        for t in self.timers.iter_mut() {
            if t.name == name {
                t.handle.abort();
            }
        }
        self.timer_sync();
    }

    pub fn timer_sync(&mut self) {
        self.timers.retain(|t| !t.handle.is_finished());
    }

    pub fn iter_timers(&self) -> impl Iterator<Item = &Timer> {
        self.timers.iter()
    }

    pub fn sync(&mut self) -> Result<()> {
        self.service_sync()?;
        self.timer_sync();
        Ok(())
    }
}

pub type SemanState = Arc<Mutex<Seman>>;

async fn respond(buf: &mut TokioUnixStream, message: impl AsRef<str>) -> Result<()> {
    buf.write_all(message.as_ref().as_bytes()).await?;
    buf.write(b"\n").await?;
    Ok(())
}

async fn handle_connection(mut stream: TokioUnixStream, state: SemanState) -> Result<()> {
    let mut line = String::new();
    let mut buf = BufReader::new(&mut stream);
    if buf.read_line(&mut line).await? == 0 {
        return Ok(());
    }
    let cmd: ClientCommand = serde_json::from_str(line.trim())?;

    let mut seman = state.lock().await;
    seman.sync()?;

    match cmd {
        ClientCommand::ServerStart => {
            respond(buf.get_mut(), "error: unknown command").await?;
        }
        ClientCommand::Ping => {
            respond(buf.get_mut(), "pong").await?;
        }
        ClientCommand::ServerKill => {
            let _ = std::fs::remove_file("/tmp/seman.sock");
            std::process::exit(0);
        }
        ClientCommand::Exec { cmd } => {
            let proc = TokioCommand::new("sh").args(["-c", &cmd]).spawn()?;
            let id = proc.id().unwrap_or(0);
            respond(buf.get_mut(), format!("{id}")).await?;
        }
        ClientCommand::DefServiceStart { name, cmd } => {
            seman.service_define(name, cmd, true).await?
        }
        ClientCommand::DefService { name, cmd } => seman.service_define(name, cmd, false).await?,
        ClientCommand::ServiceStart { name } => seman.service_start(name).await?,
        ClientCommand::ServiceStop { name } => seman.service_stop(name).await?,
        ClientCommand::ServiceList => {
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
            respond(buf.get_mut(), result).await?;
        }
        ClientCommand::ServerStatus => {
            respond(buf.get_mut(), "server: ok!").await?;
        }
        ClientCommand::Timer { name, time, cmd } => match humantime::parse_duration(&time) {
            Ok(duration) => {
                seman.timer_add(name, duration, cmd);
            }
            Err(err) => {
                respond(buf.get_mut(), format!("error: invalid time format: {err}")).await?;
            }
        },
        ClientCommand::TimerKill { name } => {
            seman.timer_kill(name);
        }
        ClientCommand::Timers => {
            let mut result = String::new();
            for (i, timer) in seman
                .iter_timers()
                .sorted_by(|a, b| {
                    a.duration
                        .cmp(&b.duration)
                        .then_with(|| a.name.cmp(&b.name))
                })
                .enumerate()
            {
                result.push_str(
                    format!(
                        "[{i}] =>\n\tname: {}\n\tcmd: {}\n\ttime: {:?}\n",
                        timer.name, timer.cmd, timer.duration
                    )
                    .as_str(),
                );
            }
        }
    }
    Ok(())
}

async fn server_loop() -> Result<()> {
    let _ = std::fs::remove_file(super::SOCKET);
    let listener = tokio::net::UnixListener::bind(super::SOCKET)?;
    let state = Arc::new(Mutex::new(Seman::new()));
    loop {
        let (stream, _) = listener.accept().await?;
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

    tokio::runtime::Runtime::new()
        .context("failed to initialize tokio runtime")?
        .block_on(server_loop())
}
