use anyhow::{Result, bail};
use log::info;
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
    pub exit_code: i32,
}

impl Service {
    pub fn new(cmd: String, proc: Option<TokioProcess>) -> Self {
        Self {
            cmd,
            proc,
            exit_code: 0,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let proc = TokioCommand::new("sh").args(["-c", &self.cmd]).spawn()?;

        self.kill().await;
        self.proc = Some(proc);
        self.exit_code = 0;

        Ok(())
    }

    pub fn is_active(&mut self) -> bool {
        if let Some(mut proc) = self.proc.take() {
            match proc.try_wait() {
                Ok(None) => {
                    self.proc = Some(proc);
                    true
                }
                Ok(Some(status)) => {
                    self.exit_code = status.code().unwrap_or(0);
                    false
                }
                Err(_) => false,
            }
        } else {
            false
        }
    }

    pub async fn kill(&mut self) {
        if let Some(mut proc) = self.proc.take() {
            _ = proc.kill().await;
        }
        self.exit_code = 0;
    }
}

pub struct Timer {
    pub name: String,
    pub cmd: Option<String>,
    pub duration: Duration,
    pub deadline: Instant,
    pub handle: JoinHandle<()>,
}

impl Timer {
    pub fn new(
        name: String,
        cmd: Option<String>,
        duration: Duration,
        handle: JoinHandle<()>,
    ) -> Self {
        let deadline = Instant::now() + duration;
        Self {
            name,
            cmd,
            duration,
            deadline,
            handle,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
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
        
        info!("service: {}, defined!", name);
        
        if let Some(mut result) = self.services.insert(name, service) {
            result.kill().await;
        }
        Ok(())
    }

    pub async fn service_start(&mut self, name: String) -> Result<()> {
        if let Some(result) = self.services.get_mut(&name) {
            result.start().await?;
            info!("service: {}, started successfully!", name);
            Ok(())
        } else {
            bail!("service: {name}, does not exist, and so cannot be started!")
        }
    }

    pub async fn service_stop(&mut self, name: String) -> Result<()> {
        if let Some(result) = self.services.get_mut(&name) {
            result.kill().await;
            info!("service: {}, killed successfully!", name);
            Ok(())
        } else {
            bail!("service: {name}, does not exist, and so cannot be stopped!")
        }
    }

    pub fn service_sync(&mut self) {
        for (_, service) in self.services.iter_mut() {
            _ = service.is_active();
        }
    }

    pub fn iter_services(&self) -> impl Iterator<Item = (&String, &Service)> {
        self.services.iter()
    }

    pub fn timer_add(&mut self, name: String, duration: Duration, cmd: Option<String>) {
        let cmd1 = cmd.clone();
        let name1 = name.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let name = name1;
            if let Some(cmd) = cmd1 {
                info!("timer: {name}, finished, executing command: {cmd}");
                let _ = TokioCommand::new("sh").args(["-c", &cmd]).output().await;
            } else {
                info!("timer: {name}, finished! no command to execute!");
            }
        });
        if let Some(ref cmd) = cmd {
            info!("timer: {name}, added with command: {cmd}");
        } else {
            info!("timer: {name}, added with no command!");
        }
        self.timers.push(Timer::new(name, cmd, duration, handle));
        self.timer_sync();
    }

    pub fn timer_kill(&mut self, name: impl AsRef<str>) {
        let name = name.as_ref();
        let mut count: usize = 0;
        for t in self.timers.iter_mut() {
            if t.name == name {
                t.handle.abort();
                count += 1;
            }
        }
        info!("timers with name: {}, killed! count: {}", name, count);
        self.timer_sync();
    }

    pub fn timer_sync(&mut self) {
        self.timers.retain(|t| !t.handle.is_finished());
    }

    pub fn iter_timers(&self) -> impl Iterator<Item = &Timer> {
        self.timers.iter()
    }

    pub fn sync(&mut self) {
        self.service_sync();
        self.timer_sync();
    }
}
