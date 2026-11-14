use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command, PresetAction};

mod apply;
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
        Command::Preset { action } => match action {
            PresetAction::Init => preset_init(),
            PresetAction::Plan { file, json } => preset_plan(&file, json),
        },
    }
}

fn preset_init() -> Result<()> {
    let machine = nodus_core::preset::preset_from_machine(&process::RealRunner)?;
    if let Some(aviso) = render::init_warning(machine.flatpak_installed) {
        eprintln!("{aviso}");
    }
    print!("{}", nodus_core::preset::to_toml(&machine.preset));
    Ok(())
}

fn preset_plan(file: &std::path::Path, json: bool) -> Result<()> {
    let text = preset_file::read_preset(file)?;
    let preset = nodus_core::preset::parse_preset(&text)
        .with_context(|| format!("lendo {}", file.display()))?;
    let plan = nodus_core::preset::collect_plan(&process::RealRunner, &preset)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print!("{}", render::render_plan(&plan));
    }
    Ok(())
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
