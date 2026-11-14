use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command};

mod cli;
mod discovery;
mod hardware;
mod preset_file;
mod process;
mod render;

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Inventory { json } => inventory(json),
        Command::Updates { json } => updates(json),
        Command::Repos { dir, fetch, json } => repos(&dir, fetch, json),
    }
}

fn repos(dir: &std::path::Path, fetch: bool, json: bool) -> Result<()> {
    let found = discovery::find_repos(dir).with_context(|| format!("lendo {}", dir.display()))?;
    let mut statuses = Vec::with_capacity(found.len());
    for repo in &found {
        let path = repo
            .path
            .to_str()
            .with_context(|| format!("caminho fora de UTF-8: {}", repo.path.display()))?;
        statuses.push(nodus_core::repos::inspect_repo(
            &process::RealRunner,
            &repo.name,
            path,
            fetch,
        ));
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&statuses)?);
    } else {
        print!("{}", render::render_repos(&statuses));
    }
    Ok(())
}

fn updates(json: bool) -> Result<()> {
    let updates = nodus_core::updates::collect_updates(&process::RealRunner)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&updates)?);
    } else {
        print!("{}", render::render_updates(&updates));
    }
    Ok(())
}

fn inventory(json: bool) -> Result<()> {
    let os_release = std::fs::read_to_string("/etc/os-release").context("lendo /etc/os-release")?;
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .context("lendo /proc/sys/kernel/osrelease")?;
    let hardware = hardware::read_hardware()?;
    let inventory =
        nodus_core::inventory::collect(&process::RealRunner, &os_release, &kernel, hardware)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&inventory)?);
    } else {
        print!("{}", render::render_text(&inventory));
    }
    Ok(())
}
