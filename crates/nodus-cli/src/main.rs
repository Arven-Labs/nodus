use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command, DotfilesAction, HistoryAction, PresetAction, WorkspaceAction};
use nodus_core::history::Manager;
use nodus_core::runner::CommandRunner;

mod adopt;
mod apply;
mod cli;
mod config;
mod discovery;
mod dotfiles;
mod dotfiles_adopt;
mod dotfiles_link;
mod dotfiles_undo;
mod hardware;
mod history;
mod preset_file;
mod process;
mod render;
mod snapshot;
#[cfg(test)]
mod testutil;
mod undo;
mod undo_flatpak;
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
        Command::Snapshot { name } => snapshot_command(name),
        Command::Compare { a, b, all, json } => match (a, b) {
            (Some(a), Some(b)) => compare_command(&a, &b, json),
            _ if all => compare_all_command(json),
            _ => unreachable!("o clap exige dois rótulos ou --all"),
        },
        Command::History { json, action } => match action {
            None => show_history(json),
            Some(HistoryAction::Undo { n }) => history_undo(n),
        },
        Command::Dotfiles { json, action } => match action {
            None => dotfiles_command(json),
            Some(DotfilesAction::Adopt { path }) => dotfiles_adopt(&path),
            Some(DotfilesAction::Link {
                path: Some(path), ..
            }) => dotfiles_link(&path),
            Some(DotfilesAction::Link { all: true, .. }) => dotfiles_link_all(),
            Some(DotfilesAction::Link { .. }) => unreachable!("o clap exige um caminho ou --all"),
        },
        Command::Repos { dir, fetch, json } => repos(dir.as_deref(), fetch, json),
        Command::Workspace { dir, json, action } => match action {
            None => workspace_command(dir.as_deref(), json),
            Some(WorkspaceAction::Set { dir }) => workspace_set(&dir),
            Some(WorkspaceAction::Unset) => workspace_unset(),
        },
        Command::Preset { action } => match action {
            PresetAction::Init => preset_init(),
            PresetAction::Plan {
                file,
                json,
                machine,
            } => preset_plan(&file, json, machine.as_deref()),
            PresetAction::Suggest {
                file,
                json,
                all,
                machine,
            } => preset_suggest(&file, json, all, machine.as_deref()),
            PresetAction::Status { file, labels, json } => preset_status(&file, &labels, json),
            PresetAction::Adopt { file, machine } => preset_adopt(&file, machine.as_deref()),
            PresetAction::Ignored => preset_ignored(),
            PresetAction::Unignore { names } => preset_unignore(&names),
            PresetAction::Apply {
                file,
                skip_flatpak,
                machine,
            } => preset_apply(&file, skip_flatpak, machine.as_deref()),
        },
    }
}

fn read_and_parse_file(file: &std::path::Path) -> Result<nodus_core::preset::PresetFile> {
    let text = preset_file::read_preset(file)?;
    nodus_core::preset::parse_preset_file(&text)
        .with_context(|| format!("lendo {}", file.display()))
}

/// O que `[machines.<rótulo>]` fez no preset desta máquina, para o plano dizer.
struct MachineNotes {
    label: String,
    skipped: nodus_core::preset::Items,
    added: nodus_core::preset::Items,
}

fn read_for_machine(
    file: &std::path::Path,
    machine: Option<&str>,
) -> Result<(nodus_core::preset::Preset, Option<MachineNotes>)> {
    resolve_for_machine(&read_and_parse_file(file)?, machine)
}

/// O preset desta máquina: sem o que `[machines.<rótulo>]` pula e com o que acrescenta. O rótulo é
/// o de `--machine` (validado sempre) ou, só se o arquivo tem tabelas, o hostname; sem tabelas o
/// hostname nem é lido.
fn resolve_for_machine(
    parsed: &nodus_core::preset::PresetFile,
    machine: Option<&str>,
) -> Result<(nodus_core::preset::Preset, Option<MachineNotes>)> {
    let label = match machine {
        Some(label) => {
            nodus_core::snapshot::validate_name(label)?;
            Some(label.to_string())
        }
        None if parsed.has_machines() => Some(
            default_snapshot_name()
                .context("o preset tem `[machines]`; passe --machine <rótulo>")?,
        ),
        None => None,
    };
    Ok(match label {
        Some(label) => {
            let preset = parsed.for_machine(&label);
            let notes = MachineNotes {
                skipped: parsed.skipped(&label),
                added: parsed.added(&label),
                label,
            };
            (preset, Some(notes))
        }
        None => (parsed.preset.clone(), None),
    })
}

/// Avisa o que esta máquina pulou e acrescentou (nada se não mexeu em nada).
fn emit_machine_notes(notes: Option<&MachineNotes>) -> Result<()> {
    if let Some(notes) = notes {
        emit(&render::render_machine_changes(
            &notes.label,
            &notes.skipped,
            &notes.added,
        ))?;
    }
    Ok(())
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

fn dotfiles_history_undo(
    n: u32,
    entries: &[nodus_core::history::HistoryEntry],
    history_path: &std::path::Path,
) -> Result<()> {
    use std::io::IsTerminal;
    let home = dotfiles::home_dir(std::env::var("HOME").ok().as_deref())?;
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("o relógio do sistema está antes de 1970")?
        .as_secs();
    let stdin = std::io::stdin();
    let request = dotfiles_undo::Request {
        home: &home,
        entries,
        n,
        interactive: stdin.is_terminal(),
        history_path,
        now_unix,
    };
    dotfiles_undo::undo_dotfile(&request, &mut stdin.lock(), &mut std::io::stdout())?;
    Ok(())
}

/// IDs dos apps Flatpak instalados no `--user` (o mesmo escopo do `preset apply`, ADR 0015). `--app`
/// deixa os runtimes de fora: nunca são alvo da reversão. ID fora da regra do Flatpak é descartado.
fn flatpak_user_apps() -> Result<Vec<String>> {
    let output = process::RealRunner
        .run(
            "flatpak",
            &["list", "--app", "--user", "--columns=application"],
        )
        .context("conferindo os apps Flatpak instalados")?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|id| nodus_core::preset::is_valid_flatpak_id(id))
        .map(String::from)
        .collect())
}

fn flatpak_history_undo(
    n: u32,
    entries: &[nodus_core::history::HistoryEntry],
    history_path: &std::path::Path,
) -> Result<()> {
    use std::io::IsTerminal;
    let installed = flatpak_user_apps()?;
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("o relógio do sistema está antes de 1970")?
        .as_secs();
    let stdin = std::io::stdin();
    let request = undo_flatpak::Request {
        entries,
        n,
        installed: &installed,
        interactive: stdin.is_terminal(),
        now_unix,
    };
    undo_flatpak::undo(
        &request,
        &mut stdin.lock(),
        &mut std::io::stdout(),
        |command| {
            let (program, args) = command.split_first().context("comando vazio")?;
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            Ok(process::run_interactive(program, &args)?)
        },
        flatpak_user_apps,
        history_path,
    )?;
    Ok(())
}

fn history_undo(n: u32) -> Result<()> {
    use std::io::IsTerminal;
    let history_path = history_file()?;
    let entries = history::read(&history_path)?;
    // Dotfile não passa pelo pacman: desvia antes de consultá-lo (ADR 0023).
    let is_adoption = usize::try_from(n)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|index| entries.get(index))
        .is_some_and(|entry| entry.manager == Manager::Dotfiles);
    if is_adoption {
        return dotfiles_history_undo(n, &entries, &history_path);
    }
    // Flatpak tem o seu caminho (sem simulação e sem pacman): desvia antes de consultar o pacman.
    let is_flatpak = usize::try_from(n)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|index| entries.get(index))
        .is_some_and(|entry| entry.manager == Manager::Flatpak);
    if is_flatpak {
        return flatpak_history_undo(n, &entries, &history_path);
    }
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

fn preset_apply(file: &std::path::Path, skip_flatpak: bool, machine: Option<&str>) -> Result<()> {
    use std::io::IsTerminal;
    let history_path = history_file()?;
    let (preset, notes) = read_for_machine(file, machine)?;
    let mut plan = nodus_core::preset::collect_plan(&process::RealRunner, &preset)?;
    emit_machine_notes(notes.as_ref())?;
    if skip_flatpak {
        let (without, skipped) = apply::without_flatpak(&plan);
        plan = without;
        emit(&render::render_skipped_flatpak(&skipped))?;
    }
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

fn preset_plan(file: &std::path::Path, json: bool, machine: Option<&str>) -> Result<()> {
    let (preset, notes) = read_for_machine(file, machine)?;
    let plan = nodus_core::preset::collect_plan(&process::RealRunner, &preset)?;
    if json {
        emit(&format!("{}\n", serde_json::to_string_pretty(&plan)?))?;
    } else {
        emit_machine_notes(notes.as_ref())?;
        emit(&render::render_plan(&plan))?;
    }
    Ok(())
}

fn preset_status(file: &std::path::Path, labels: &[String], json: bool) -> Result<()> {
    let preset = read_and_parse_file(file)?;
    let dir = dotfiles_source_dir()?;
    let (snapshots, unreadable) = if labels.is_empty() {
        snapshot::read_all(&dir)?
    } else {
        (snapshot::read_labels(&dir, labels)?, Vec::new())
    };
    let statuses: Vec<_> = snapshots
        .iter()
        .map(|snapshot| nodus_core::status::status_for(&preset, snapshot))
        .collect();
    if json {
        let unreadable: Vec<_> = unreadable
            .iter()
            .map(|u| serde_json::json!({"label": u.label, "reason": u.reason}))
            .collect();
        let report = serde_json::json!({"machines": statuses, "unreadable": unreadable});
        emit(&format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    } else {
        emit(&render::render_status(
            &preset.preset,
            &statuses,
            &unreadable,
        ))?;
    }
    Ok(())
}

fn preset_suggest(
    file: &std::path::Path,
    json: bool,
    all: bool,
    machine: Option<&str>,
) -> Result<()> {
    let (preset, _) = read_for_machine(file, machine)?;
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

fn preset_adopt(file: &std::path::Path, machine: Option<&str>) -> Result<()> {
    use std::io::IsTerminal;
    let old_text = preset_file::read_preset(file)?;
    let parsed = nodus_core::preset::parse_preset_file(&old_text)
        .with_context(|| format!("lendo {}", file.display()))?;
    let (effective, _) = resolve_for_machine(&parsed, machine)?;
    let mut suggestions =
        nodus_core::preset::collect_suggestions(&process::RealRunner, &effective)?;
    // O que adotar vai para as listas compartilhadas. Um item que já está lá (mas esta máquina
    // pula) ficaria de fora do efetivo e seria oferecido, e `add_to_preset` o ignoraria: o
    // relatório diria "adotado" sem ter gravado nada.
    let known = &parsed.preset;
    suggestions
        .pacman_extra
        .retain(|name| !known.pacman.contains(name));
    suggestions
        .foreign_extra
        .retain(|name| !known.pacman.contains(name));
    suggestions
        .flatpak_extra
        .retain(|name| !known.flatpak.contains(name));
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

fn dotfiles_command(json: bool) -> Result<()> {
    let home = dotfiles::home_dir(std::env::var("HOME").ok().as_deref())?;
    let config = config::read(&config_file()?)?;
    let list = nodus_core::dotfiles::effective_list(&config.dotfiles_extra, &config.dotfiles_skip);
    let entries = dotfiles::report(&home, &list)?;
    if json {
        emit(&format!("{}\n", serde_json::to_string_pretty(&entries)?))?;
    } else {
        emit(&render::render_dotfiles(&entries))?;
    }
    Ok(())
}

/// A pasta-fonte (`dotfiles_dir` do config), já por `canonicalize`: o symlink aponta para dentro
/// dela, então ela precisa existir.
fn dotfiles_source_dir() -> Result<std::path::PathBuf> {
    let config_path = config_file()?;
    let source_dir = config::read(&config_path)?.dotfiles_dir.with_context(|| {
        format!(
            "falta a pasta-fonte: acrescente `dotfiles_dir = \"/caminho/absoluto\"` em {}",
            config_path.display()
        )
    })?;
    std::fs::canonicalize(&source_dir)
        .with_context(|| format!("a pasta-fonte {} não existe", source_dir.display()))
}

fn dotfiles_link(path: &str) -> Result<()> {
    use std::io::IsTerminal;
    let home = dotfiles::home_dir(std::env::var("HOME").ok().as_deref())?;
    let dir = dotfiles_source_dir()?;
    let history_path = history_file()?;
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("o relógio do sistema está antes de 1970")?
        .as_secs();
    let stdin = std::io::stdin();
    let request = dotfiles_link::Request {
        home: &home,
        dir: &dir,
        path,
        interactive: stdin.is_terminal(),
        history_path: &history_path,
        now_unix,
    };
    dotfiles_link::link(&request, &mut stdin.lock(), &mut std::io::stdout())?;
    Ok(())
}

fn dotfiles_link_all() -> Result<()> {
    use std::io::IsTerminal;
    let home = dotfiles::home_dir(std::env::var("HOME").ok().as_deref())?;
    let dir = dotfiles_source_dir()?;
    let config = config::read(&config_file()?)?;
    let paths = nodus_core::dotfiles::effective_list(&config.dotfiles_extra, &config.dotfiles_skip);
    let history_path = history_file()?;
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("o relógio do sistema está antes de 1970")?
        .as_secs();
    let stdin = std::io::stdin();
    let request = dotfiles_link::BatchRequest {
        home: &home,
        dir: &dir,
        paths: &paths,
        interactive: stdin.is_terminal(),
        history_path: &history_path,
        now_unix,
    };
    dotfiles_link::link_all(&request, &mut stdin.lock(), &mut std::io::stdout())?;
    Ok(())
}

fn dotfiles_adopt(path: &str) -> Result<()> {
    use std::io::IsTerminal;
    let home = dotfiles::home_dir(std::env::var("HOME").ok().as_deref())?;
    let dir = dotfiles_source_dir()?;
    let history_path = history_file()?;
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("o relógio do sistema está antes de 1970")?
        .as_secs();
    let stdin = std::io::stdin();
    let request = dotfiles_adopt::Request {
        home: &home,
        dir: &dir,
        path,
        interactive: stdin.is_terminal(),
        history_path: &history_path,
        now_unix,
    };
    dotfiles_adopt::adopt(&request, &mut stdin.lock(), &mut std::io::stdout())?;
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

/// Rótulo padrão: o hostname. Fora da regra do rótulo (maiúsculas, por exemplo), recusa e pede
/// `--name`, em vez de trocar o nome em silêncio.
fn default_snapshot_name() -> Result<String> {
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .context("lendo /proc/sys/kernel/hostname")?;
    let host = host.trim().to_string();
    nodus_core::snapshot::validate_name(&host)
        .with_context(|| format!("o hostname {host:?} não serve de rótulo; use --name"))?;
    Ok(host)
}

fn snapshot_command(name: Option<String>) -> Result<()> {
    // O HOME e a pasta-fonte vêm antes de qualquer programa externo: sem HOME o snapshot erra, em
    // vez de gravar `dotfiles: false` em silêncio.
    let home = dotfiles::home_dir(std::env::var("HOME").ok().as_deref())?;
    let dir = dotfiles_source_dir()?;
    let name = match name {
        Some(name) => name,
        None => default_snapshot_name()?,
    };
    // Falha antes de rodar qualquer programa externo.
    nodus_core::snapshot::validate_name(&name)?;
    let config = config::read(&config_file()?)?;
    let list = nodus_core::dotfiles::effective_list(&config.dotfiles_extra, &config.dotfiles_skip);
    let entries = dotfiles::dotfile_entries(&home, &dir, &list)?;
    let os_release = std::fs::read_to_string("/etc/os-release").context("lendo /etc/os-release")?;
    let os = nodus_core::inventory::parse_os_release(&os_release)?;
    let machine = nodus_core::preset::preset_from_machine(&process::RealRunner)?;
    let snapshot = nodus_core::snapshot::build(&name, &os, &machine)?.with_dotfiles(entries)?;
    let path = snapshot::write(&dir, &snapshot)?;
    emit(&format!(
        "Snapshot gravado em {}\nO commit e o push são seus: o nodus não roda git.\n",
        path.display()
    ))
}

fn compare_command(a: &str, b: &str, json: bool) -> Result<()> {
    let dir = dotfiles_source_dir()?;
    let first = snapshot::read(&dir, a)?;
    let second = snapshot::read(&dir, b)?;
    let comparison = nodus_core::compare::compare(&first, &second);
    if json {
        emit(&format!("{}\n", serde_json::to_string_pretty(&comparison)?))?;
    } else {
        emit(&render::render_comparison(&comparison))?;
    }
    Ok(())
}

/// Todos os pares de snapshots legíveis, em ordem de rótulo (`read_all` já ordena). Ilegível vira
/// aviso; com menos de dois legíveis a lista de pares é vazia e não é erro.
fn compare_all_command(json: bool) -> Result<()> {
    let dir = dotfiles_source_dir()?;
    let (snapshots, unreadable) = snapshot::read_all(&dir)?;
    let mut comparisons = Vec::new();
    for (i, first) in snapshots.iter().enumerate() {
        for second in &snapshots[i + 1..] {
            comparisons.push(nodus_core::compare::compare(first, second));
        }
    }
    if json {
        let unreadable: Vec<_> = unreadable
            .iter()
            .map(|u| serde_json::json!({"name": u.label, "reason": u.reason}))
            .collect();
        let report = serde_json::json!({"comparisons": comparisons, "unreadable": unreadable});
        emit(&format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    } else {
        emit(&render::render_all_comparisons(
            snapshots.len(),
            &comparisons,
            &unreadable,
        ))?;
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
