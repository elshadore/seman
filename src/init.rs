use super::Command;
use super::Response;
use super::seman::SemanState;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use log::{debug, info};

fn resolve_config_path() -> String {
    if let Ok(p) = std::env::var("SEMANRC") {
        return p;
    }

    match std::env::var("XDG_CONFIG_HOME") {
        Ok(xdg_home) => {
            format!("{xdg_home}/.config/seman/.semanrc")
        }
        Err(_) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{home}/.config/seman/.semanrc")
        }
    }
}

pub async fn run_init_file(state: SemanState) -> Result<()> {
    let path = resolve_config_path();
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
