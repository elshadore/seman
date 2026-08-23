use anyhow::{Result, bail};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
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
    pub deadline: Instant,
    pub handle: JoinHandle<()>,
}

impl Timer {
    pub fn new(name: String, cmd: String, duration: Duration, handle: JoinHandle<()>) -> Self {
        let deadline = Instant::now() + duration;
        Self {
            name,
            cmd,
            duration,
            deadline,
            handle,
        }
    }
}

pub type SemanState = Arc<Mutex<Seman>>;

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
