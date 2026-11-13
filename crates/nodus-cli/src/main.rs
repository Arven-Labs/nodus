use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command};

mod cli;
mod hardware;
mod process;
mod render;

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Inventory { json } => inventory(json),
        Command::Updates { json } => updates(json),
    }
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
