use super::Command;
use super::Response;
use super::seman::SemanState;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use log::{debug, info};

fn resolve_config_paths() -> Vec<String> {
    if let Ok(p) = std::env::var("SEMANRC") {
        if !p.is_empty() {
            return vec![p];
        }
    }

    let mut paths = Vec::new();
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            paths.push(format!("{xdg}/.semanrc"));
            paths.push(format!("{xdg}/seman/.semanrc"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            paths.push(format!("{home}/.semanrc"));
            paths.push(format!("{home}/seman/.semanrc"));
        }
    }
    paths
}

pub async fn run_init_file(state: SemanState) -> Result<()> {
    let path = match resolve_config_paths()
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
    {
        Some(p) => p,
        None => {
            info!("no init file found, skipping");
            return Ok(());
        }
    };

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => {
            info!("no init file at {path}, skipping");
            return Ok(());
        }
    };

    info!("loading init file: {path}");

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
            Response::Ok | Response::OkMsg(_) => debug!("init ok: {line}"),
            Response::Error(e) => bail!("init command failed: {line} -> {e}"),
        }
    }
    Ok(())
}
