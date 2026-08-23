use super::Command;
use super::Response;
use super::seman::SemanState;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use log::{info, warn};
use std::fs::File;
use std::io::Read;

fn resolve_config_file() -> Option<File> {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(p) = std::env::var("SEMEN_CONFIG") {
        if !p.is_empty() {
            info!("SEMEN_CONFIG config environment variable read: {p}");
            candidates.push(p);
        }
    } else {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                candidates.push(format!("{xdg}/.semanrc"));
                candidates.push(format!("{xdg}/seman/.semanrc"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                candidates.push(format!("{home}/.semanrc"));
                candidates.push(format!("{home}/seman/.semanrc"));
            }
        }
    }

    for path in candidates {
        match File::open(&path) {
            Ok(file) => {
                info!("config found at {path}");
                return Some(file);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!("config not found at {path}");
            }
            Err(e) => {
                warn!("config at {path} present but unreadable: {e}");
            }
        }
    }
    info!("no valid config found!");
    None
}

pub async fn run_init_file(state: SemanState) -> Result<()> {
    let mut file = match resolve_config_file() {
        Some(f) => f,
        None => return Ok(()),
    };

    let mut text = String::new();
    if file.read_to_string(&mut text).is_err() {
        warn!("failed to read init file, skipping");
        return Ok(());
    }

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tokens = match shell_words::split(line) {
            Ok(t) => t,
            Err(e) => {
                bail!("failed to tokenize init line: {line}: {e}");
            }
        };
        let mut args = vec!["seman".to_string()];
        args.extend(tokens);
        let cmd = match Command::try_parse_from(args) {
            Ok(c) => c,
            Err(e) => {
                bail!("parsing error in init script: {line}: {e}");
            }
        };
        if matches!(
            cmd,
            Command::ServerKill | Command::ServerStart | Command::ServerStatus
        ) {
            bail!("server commands cannot be used in the init script: {line}");
        }
        let resp = super::server::execute(state.clone(), cmd).await;
        match resp {
            Response::Ok | Response::OkMsg(_) => info!("init ok: {line}"),
            Response::Error(e) => bail!("init command failed: {line} -> {e}"),
        }
    }
    Ok(())
}
