use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command, HistoryAction, PresetAction, WorkspaceAction};
use nodus_core::history::Manager;
use nodus_core::runner::CommandRunner;

mod adopt;
mod apply;
mod cli;
mod config;
mod discovery;
mod hardware;
mod history;
mod preset_file;
mod process;
mod render;
#[cfg(test)]
mod testutil;
mod undo;
mod workspace;

/// Toda saída de comando passa por aqui, não por `print!`: o `print!` do std entra em pânico com o
/// stdout fechado (`| head`, `| jq` ausente), porque o Rust ignora o `SIGPIPE` e o `write` falha com
/// `EPIPE`. Pipe fechado encerra em silêncio com 141 (128 + SIGPIPE, como `cat` e `ls`); qualquer
/// outro erro de escrita sobe normalmente. `exit` não roda destrutores: nenhum estado a gravar
/// depende de um `emit` que ainda não rodou (o `apply` grava o histórico antes do "Depois:").
fn emit(text: &str) -> Result<()> {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        Err(erro) if erro.kind() == std::io::ErrorKind::BrokenPipe => std::process::exit(141),
        Err(erro) => Err(erro).context("escrevendo no stdout"),
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Inventory { json } => inventory(json),
        Command::Updates { json } => updates(json),
        Command::History { json, action } => match action {
            None => show_history(json),
            Some(HistoryAction::Undo { n }) => history_undo(n),
        },
        Command::Repos { dir, fetch, json } => repos(dir.as_deref(), fetch, json),
        Command::Workspace { dir, json, action } => match action {
            None => workspace_command(dir.as_deref(), json),
            Some(WorkspaceAction::Set { dir }) => workspace_set(&dir),
            Some(WorkspaceAction::Unset) => workspace_unset(),
        },
        Command::Preset { action } => match action {
            PresetAction::Init => preset_init(),
            PresetAction::Plan { file, json } => preset_plan(&file, json),
            PresetAction::Suggest { file, json, all } => preset_suggest(&file, json, all),
            PresetAction::Adopt { file } => preset_adopt(&file),
            PresetAction::Ignored => preset_ignored(),
            PresetAction::Unignore { names } => preset_unignore(&names),
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
        emit(&format!("{}\n", serde_json::to_string_pretty(&entries)?))?;
    } else {
        emit(&render::render_history(&entries))?;
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
    emit(&render::render_plan(&plan))?;
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
        emit("Depois:\n")?;
        emit(&render::render_plan(&after))?;
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
    emit(&nodus_core::preset::to_toml(&machine.preset))?;
    Ok(())
}

fn preset_plan(file: &std::path::Path, json: bool) -> Result<()> {
    let preset = read_and_parse(file)?;
    let plan = nodus_core::preset::collect_plan(&process::RealRunner, &preset)?;
    if json {
        emit(&format!("{}\n", serde_json::to_string_pretty(&plan)?))?;
    } else {
        emit(&render::render_plan(&plan))?;
    }
    Ok(())
}

fn preset_suggest(file: &std::path::Path, json: bool, all: bool) -> Result<()> {
    let preset = read_and_parse(file)?;
    let suggestions = nodus_core::preset::collect_suggestions(&process::RealRunner, &preset)?;
    let config = config::read(&config_file()?)?;
    let (visible, hidden) = nodus_core::preset::hide_ignored(
        &suggestions,
        &config.ignore_pacman,
        &config.ignore_flatpak,
    );
    // Com `--all` mostra tudo; sem ele, só o que não foi ignorado, e conta o resto.
    let shown = if all { &suggestions } else { &visible };
    if json {
        emit(&format!(
            "{}\n",
            serde_json::to_string_pretty(&render::suggestions_json(shown, &hidden)?)?
        ))?;
    } else {
        emit(&render::render_suggestions(shown))?;
        emit(&render::render_ignored_note(&hidden, all))?;
    }
    Ok(())
}

fn preset_ignored() -> Result<()> {
    let config = config::read(&config_file()?)?;
    emit(&render::render_ignored(
        &config.ignore_pacman,
        &config.ignore_flatpak,
    ))
}

fn preset_unignore(names: &[String]) -> Result<()> {
    config::remove_ignored(&config_file()?, names)?;
    emit(&format!(
        "Voltam a aparecer no adopt e no suggest: {}\n",
        names.join(", ")
    ))
}

fn preset_adopt(file: &std::path::Path) -> Result<()> {
    use std::io::IsTerminal;
    let old_text = preset_file::read_preset(file)?;
    let preset = nodus_core::preset::parse_preset(&old_text)
        .with_context(|| format!("lendo {}", file.display()))?;
    let suggestions = nodus_core::preset::collect_suggestions(&process::RealRunner, &preset)?;
    // O que foi ignorado antes não é perguntado de novo.
    let config_path = config_file()?;
    let config = config::read(&config_path)?;
    let (suggestions, hidden) = nodus_core::preset::hide_ignored(
        &suggestions,
        &config.ignore_pacman,
        &config.ignore_flatpak,
    );
    if suggestions.is_empty() {
        emit(&render::render_suggestions(&suggestions))?;
        return emit(&render::render_ignored_note(&hidden, false));
    }
    let stdin = std::io::stdin();
    let adoption = adopt::ask(
        &suggestions,
        stdin.is_terminal(),
        &mut stdin.lock(),
        &mut std::io::stdout(),
    )?;
    if adoption.is_empty() && !adoption.has_ignored() {
        return emit("Nada foi adotado; o preset não foi alterado.\n");
    }
    let mut report = String::new();
    if !adoption.is_empty() {
        let new_text =
            nodus_core::preset::add_to_preset(&old_text, &adoption.pacman, &adoption.flatpak)?;
        preset_file::write_preset(file, &old_text, &new_text)?;
        report.push_str(&format!(
            "Adotado em {}: pacman {} ({}), flatpak {} ({}).\n",
            file.display(),
            adoption.pacman.len(),
            adoption.pacman.join(", "),
            adoption.flatpak.len(),
            adoption.flatpak.join(", ")
        ));
    }
    if adoption.has_ignored() {
        // O preset vem primeiro: se gravar a lista de ignorados falhar, o que foi adotado já está.
        let preset_written = !adoption.is_empty();
        config::add_ignored(
            &config_path,
            &adoption.ignored_pacman,
            &adoption.ignored_flatpak,
        )
        .with_context(|| {
            if preset_written {
                "o preset já foi escrito, mas não consegui gravar os itens ignorados"
            } else {
                "não consegui gravar os itens ignorados"
            }
        })?;
        report.push_str(&format!(
            "Ignorados para sempre (config {}): pacman {} ({}), flatpak {} ({}).\n",
            config_path.display(),
            adoption.ignored_pacman.len(),
            adoption.ignored_pacman.join(", "),
            adoption.ignored_flatpak.len(),
            adoption.ignored_flatpak.join(", ")
        ));
    }
    emit(&report)
}

/// Onde fica o config, a partir do ambiente.
fn config_file() -> Result<std::path::PathBuf> {
    config::config_path(
        std::env::var("XDG_CONFIG_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
}

/// A pasta gravada com `workspace set`, se houver. Só é chamada quando nem o argumento nem a variável
/// decidem (ver `workspace::resolve`).
fn stored_workspace() -> Result<Option<std::path::PathBuf>> {
    config::read_workspace(&config_file()?)
}

fn workspace_set(dir: &std::path::Path) -> Result<()> {
    // A mesma conferência da pasta informada: existe e é pasta. Grava o caminho real, não o digitado.
    let chosen = workspace::validate(dir)?;
    let file = config_file()?;
    config::write_workspace(&file, &chosen.path)?;
    if chosen.repo_count == 0 {
        eprintln!("aviso: a pasta não tem repositórios Git entre os filhos diretos");
    }
    emit(&format!(
        "Workspace gravado: {} (repositórios: {}); config: {}\n",
        chosen.path.display(),
        chosen.repo_count,
        file.display()
    ))
}

fn workspace_unset() -> Result<()> {
    let removed = config::clear_workspace(&config_file()?)?;
    emit(if removed {
        "Workspace gravado removido.\n"
    } else {
        "Não havia workspace gravado.\n"
    })
}

fn workspace_command(dir: Option<&std::path::Path>, json: bool) -> Result<()> {
    let report = workspace::resolve(
        dir,
        std::env::var("NODUS_WORKSPACE").ok().as_deref(),
        stored_workspace,
        std::env::var("HOME").ok().as_deref(),
    )?;
    if json {
        emit(&format!(
            "{}\n",
            serde_json::to_string_pretty(&workspace::report_json(&report))?
        ))?;
    } else {
        emit(&render::render_workspace(&report))?;
    }
    Ok(())
}

fn repos(dir: Option<&std::path::Path>, fetch: bool, json: bool) -> Result<()> {
    let (dir, source) = workspace::pick_dir(
        dir,
        std::env::var("NODUS_WORKSPACE").ok().as_deref(),
        stored_workspace,
        std::env::var("HOME").ok().as_deref(),
    )?;
    if let Some(source) = source {
        // stderr: o stdout fica só com o resultado (`--json` continua JSON puro).
        eprintln!("Workspace: {} ({})", dir.display(), source.label());
    }
    let found = discovery::find_repos(&dir).with_context(|| format!("lendo {}", dir.display()))?;
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
        emit(&format!("{}\n", serde_json::to_string_pretty(&statuses)?))?;
    } else {
        emit(&render::render_repos(&statuses))?;
    }
    Ok(())
}

fn updates(json: bool) -> Result<()> {
    let updates = nodus_core::updates::collect_updates(&process::RealRunner)?;
    if json {
        emit(&format!("{}\n", serde_json::to_string_pretty(&updates)?))?;
    } else {
        emit(&render::render_updates(&updates))?;
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
        emit(&format!("{}\n", serde_json::to_string_pretty(&inventory)?))?;
    } else {
        emit(&render::render_text(&inventory))?;
    }
    Ok(())
}
