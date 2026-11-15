use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command, HistoryAction, PresetAction};
use nodus_core::history::Manager;
use nodus_core::runner::CommandRunner;

mod apply;
mod cli;
mod discovery;
mod hardware;
mod history;
mod preset_file;
mod process;
mod render;
#[cfg(test)]
mod testutil;
mod undo;

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Inventory { json } => inventory(json),
        Command::Updates { json } => updates(json),
        Command::History { json, action } => match action {
            None => show_history(json),
            Some(HistoryAction::Undo { n }) => history_undo(n),
        },
        Command::Repos { dir, fetch, json } => repos(&dir, fetch, json),
        Command::Preset { action } => match action {
            PresetAction::Init => preset_init(),
            PresetAction::Plan { file, json } => preset_plan(&file, json),
            PresetAction::Apply { file } => preset_apply(&file),
        },
    }
}

fn read_and_parse(file: &std::path::Path) -> Result<nodus_core::preset::Preset> {
    let text = preset_file::read_preset(file)?;
    nodus_core::preset::parse_preset(&text).with_context(|| format!("lendo {}", file.display()))
}

/// Onde fica o histórico, a partir do ambiente. Resolvido antes de qualquer execução: sem lugar
/// para registrar, o `apply` nem começa.
fn history_file() -> Result<std::path::PathBuf> {
    history::history_path(
        std::env::var("XDG_STATE_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
}

fn show_history(json: bool) -> Result<()> {
    let entries = history::read(&history_file()?)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        print!("{}", render::render_history(&entries));
    }
    Ok(())
}

/// Nomes dos pacotes de `pacman -Q` (todos) ou `-Qe` (instalados de propósito).
fn pacman_names(query: &str) -> Result<Vec<String>> {
    let output = process::RealRunner.run("pacman", &[query])?;
    Ok(nodus_core::inventory::parse_pacman_q(&output)?
        .into_iter()
        .map(|package| package.name)
        .collect())
}

fn history_undo(n: u32) -> Result<()> {
    use std::io::IsTerminal;
    let history_path = history_file()?;
    let entries = history::read(&history_path)?;
    let installed = pacman_names("-Q")?;
    let explicit = pacman_names("-Qe")?;
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("o relógio do sistema está antes de 1970")?
        .as_secs();
    let stdin = std::io::stdin();
    let request = undo::Request {
        entries: &entries,
        n,
        installed: &installed,
        explicit: &explicit,
        interactive: stdin.is_terminal(),
        now_unix,
    };
    undo::undo(
        &request,
        &mut stdin.lock(),
        &mut std::io::stdout(),
        |command| {
            let (program, args) = command.split_first().context("comando vazio")?;
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            Ok(process::run_capture(program, &args)?)
        },
        |command| {
            let (program, args) = command.split_first().context("comando vazio")?;
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            Ok(process::run_interactive(program, &args)?)
        },
        || pacman_names("-Q"),
        &history_path,
    )?;
    Ok(())
}

fn preset_apply(file: &std::path::Path) -> Result<()> {
    use std::io::IsTerminal;
    let history_path = history_file()?;
    let preset = read_and_parse(file)?;
    let plan = nodus_core::preset::collect_plan(&process::RealRunner, &preset)?;
    print!("{}", render::render_plan(&plan));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("o relógio do sistema está antes de 1970")?
        .as_secs();
    let stdin = std::io::stdin();
    let interactive = stdin.is_terminal();
    let applied = apply::apply_with_history(
        &plan,
        interactive,
        &mut stdin.lock(),
        &mut std::io::stdout(),
        || {
            Ok(nodus_core::preset::flathub_configured(
                &process::RealRunner,
            )?)
        },
        |command| {
            let (program, args) = command.split_first().context("comando vazio")?;
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            Ok(process::run_interactive(program, &args)?)
        },
        // Confere pelo estado real, não pelo código de saída (no Flatpak, recusar o prompt e falhar
        // saem os dois com exit 1).
        || {
            Ok(nodus_core::preset::collect_plan(
                &process::RealRunner,
                &preset,
            )?)
        },
        &history_path,
        now,
    )?;
    if let (apply::Outcome::Executed, Some(after)) = (applied.outcome, applied.after) {
        println!("Depois:");
        print!("{}", render::render_plan(&after));
        // Só cobra o que foi executado: um passo recusado continua faltando por escolha do dono.
        if applied.executed.contains(&Manager::Pacman) && !after.pacman_missing.is_empty() {
            anyhow::bail!("ainda faltam pacotes do pacman depois da instalação");
        }
        if applied.executed.contains(&Manager::Flatpak) && !after.flatpak_missing.is_empty() {
            anyhow::bail!("ainda faltam apps Flatpak depois da instalação");
        }
    }
    Ok(())
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
    let preset = read_and_parse(file)?;
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
