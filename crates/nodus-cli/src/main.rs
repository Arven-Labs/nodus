use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command};

mod cli;
mod process;
mod render;

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Inventory { json } => inventory(json),
    }
}

fn inventory(json: bool) -> Result<()> {
    let os_release = std::fs::read_to_string("/etc/os-release").context("lendo /etc/os-release")?;
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .context("lendo /proc/sys/kernel/osrelease")?;
    let inventory = nodus_core::inventory::collect(&process::RealRunner, &os_release, &kernel)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&inventory)?);
    } else {
        print!("{}", render::render_text(&inventory));
    }
    Ok(())
}
