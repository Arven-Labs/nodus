//! Formatação do inventário para o terminal.

use crate::snapshot::Unreadable;
use crate::workspace::{Report, Source};
use nodus_core::compare::{Comparison, DotfilesComparison, ListComparison};
use nodus_core::dotfiles::Entry;
use nodus_core::history::{Action, ExecResult, HistoryEntry, Manager, format_utc};
use nodus_core::inventory::Inventory;
use nodus_core::preset::{Plan, PresetFile, Suggestions};
use nodus_core::repos::{FetchResult, RepoStatus};
use nodus_core::snapshot::{DotfileState, SnapshotOs};
use nodus_core::status::{MachineExceptions, MachineStatus, StatusList, exceptions_for};
use nodus_core::updates::{PacmanSource, Updates};
use std::fmt::Write;

pub fn render_text(inventory: &Inventory) -> String {
    let system = &inventory.system;
    let mut out = String::new();
    // Escrever em String não falha; o resultado é ignorado de propósito.
    let _ = match &system.os_version {
        Some(version) => writeln!(out, "Sistema: {} {version}", system.os_name),
        None => writeln!(out, "Sistema: {}", system.os_name),
    };
    let _ = writeln!(out, "Kernel: {}", system.kernel);
    let hardware = &inventory.hardware;
    let _ = writeln!(
        out,
        "CPU: {} ({} threads)",
        hardware.cpu_model, hardware.cpu_threads
    );
    let _ = writeln!(out, "Memória: {}", gib(hardware.memory_total_bytes));
    let _ = writeln!(out, "Discos ({}):", hardware.disks.len());
    for disk in &hardware.disks {
        let _ = writeln!(
            out,
            "  {} {} {} {}",
            disk.mount_point,
            disk.name,
            disk.file_system,
            gib(disk.total_bytes)
        );
    }
    let _ = writeln!(out, "Pacotes ({}):", inventory.packages.len());
    for package in &inventory.packages {
        let _ = writeln!(out, "  {} {}", package.name, package.version);
    }
    match &inventory.flatpak_apps {
        None => {
            let _ = writeln!(out, "Flatpak: não instalado");
        }
        Some(apps) => {
            let _ = writeln!(out, "Flatpak ({}):", apps.len());
            for app in apps {
                let _ = writeln!(
                    out,
                    "  {} {} {} {} {}",
                    app.application,
                    app.version.as_deref().unwrap_or("-"),
                    app.branch,
                    app.origin,
                    app.installation
                );
            }
        }
    }
    out
}

pub fn render_updates(updates: &Updates) -> String {
    let mut out = String::new();
    let _ = match updates.pacman_source {
        PacmanSource::Checkupdates => writeln!(out, "Pacman ({}):", updates.pacman.len()),
        PacmanSource::PacmanQu => writeln!(
            out,
            "Pacman ({}) [pode estar desatualizado: `checkupdates` não encontrado, usei `pacman -Qu`]:",
            updates.pacman.len()
        ),
    };
    for pkg in &updates.pacman {
        let _ = writeln!(
            out,
            "  {} {} -> {}",
            pkg.name, pkg.old_version, pkg.new_version
        );
    }
    match &updates.flatpak {
        None => {
            let _ = writeln!(out, "Flatpak: não instalado");
        }
        Some(apps) => {
            let _ = writeln!(out, "Flatpak ({}):", apps.len());
            for app in apps {
                let _ = writeln!(
                    out,
                    "  {} {} {}",
                    app.application,
                    app.version.as_deref().unwrap_or("-"),
                    app.branch
                );
            }
        }
    }
    out
}

pub fn render_repos(repos: &[RepoStatus]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Repositórios ({}):", repos.len());
    for repo in repos {
        let _ = write!(out, "  {}", repo.name);
        if let Some(erro) = &repo.error {
            let _ = writeln!(out, " ERRO: {erro}");
            continue;
        }
        match (&repo.branch, &repo.commit) {
            (Some(branch), _) => {
                let _ = write!(out, " {branch}");
            }
            (None, Some(_)) => {
                let _ = write!(out, " (HEAD solto)");
            }
            (None, None) => {
                let _ = write!(out, " (sem commits)");
            }
        }
        if let Some(commit) = &repo.commit {
            let _ = write!(out, " {commit}");
        }
        match &repo.upstream {
            None => {
                let _ = write!(out, " sem upstream");
            }
            Some(upstream) if repo.upstream_gone => {
                let _ = write!(out, " {upstream} (sumiu)");
            }
            Some(upstream) => {
                let _ = write!(out, " {upstream}");
                if let (Some(ahead), Some(behind)) = (repo.ahead, repo.behind) {
                    let _ = write!(out, " (à frente {ahead}, atrás {behind})");
                }
            }
        }
        match &repo.origin_url {
            Some(url) => {
                let _ = write!(out, " {url}");
            }
            None => {
                let _ = write!(out, " sem origin");
            }
        }
        match &repo.fetch {
            Some(FetchResult::Done) => {
                let _ = write!(out, " [fetch ok]");
            }
            Some(FetchResult::Failed(motivo)) => {
                let _ = write!(out, " [fetch falhou: {motivo}]");
            }
            None => {}
        }
        let _ = writeln!(out);
    }
    out
}

pub fn render_history(entries: &[HistoryEntry]) -> String {
    if entries.is_empty() {
        return "Nenhuma execução registrada.\n".to_string();
    }
    let plural = if entries.len() == 1 {
        "execução"
    } else {
        "execuções"
    };
    let mut out = format!("Histórico ({} {plural}):\n", entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let result = match &entry.result {
            ExecResult::Ok => "ok".to_string(),
            ExecResult::Failed { code: Some(code) } => format!("falhou (código {code})"),
            ExecResult::Failed { code: None } => "falhou".to_string(),
        };
        let when = format_utc(entry.at_unix);
        let n = index + 1;
        let requested = entry.requested.len();
        let (installed, missing) = (entry.installed.len(), entry.still_missing.len());
        match (entry.action, entry.undoes) {
            (Action::Remove, Some(undoes)) if entry.manager == Manager::Dotfiles => {
                let dir = entry.dir.as_deref().unwrap_or("?");
                // A linha de uma reversão é a mesma para adoção e ligação (ADR 0025): quem
                // distingue é a entrada que `undoes` aponta.
                let undid_link = usize::try_from(undoes)
                    .ok()
                    .and_then(|undoes| undoes.checked_sub(1))
                    .and_then(|index| entries.get(index))
                    .is_some_and(|original| original.action == Action::Link);
                let what = if undid_link {
                    "ligação desfeita"
                } else {
                    "dotfile restaurado"
                };
                let _ = writeln!(
                    out,
                    "[{n}] {when}  {result}  desfez [{undoes}]  {what}: {}  (pasta-fonte {dir})",
                    entry.requested.join(", ")
                );
            }
            (Action::Remove, Some(undoes)) => {
                let _ = writeln!(
                    out,
                    "[{n}] {when}  {result}  desfez [{undoes}]  pedidos {requested}, removidos {missing}, ainda instalados {installed}"
                );
                if !entry.still_missing.is_empty() {
                    let _ = writeln!(out, "  removidos: {}", entry.still_missing.join(", "));
                }
                if !entry.installed.is_empty() {
                    let _ = writeln!(out, "  ainda instalados: {}", entry.installed.join(", "));
                }
            }
            (Action::Adopt, _) => {
                let dir = entry.dir.as_deref().unwrap_or("?");
                let _ = writeln!(
                    out,
                    "[{n}] {when}  {result}  dotfile adotado: {}  (pasta-fonte {dir})",
                    entry.requested.join(", ")
                );
            }
            (Action::Link, _) => {
                let dir = entry.dir.as_deref().unwrap_or("?");
                let _ = writeln!(
                    out,
                    "[{n}] {when}  {result}  dotfile ligado: {}  (pasta-fonte {dir})",
                    entry.requested.join(", ")
                );
            }
            _ => {
                let manager = match entry.manager {
                    Manager::Pacman => "",
                    Manager::Flatpak => "  flatpak",
                    Manager::Dotfiles => "  dotfiles",
                };
                let _ = writeln!(
                    out,
                    "[{n}] {when}  {result}{manager}  pedidos {requested}, instalados {installed}, faltando {missing}"
                );
                if !entry.installed.is_empty() {
                    let _ = writeln!(out, "  instalados: {}", entry.installed.join(", "));
                }
                if !entry.still_missing.is_empty() {
                    let _ = writeln!(out, "  faltando: {}", entry.still_missing.join(", "));
                }
            }
        }
    }
    out.push_str(HISTORY_NOTE);
    out
}

/// O histórico guarda a intenção, não o efeito (ADR 0011, item 8).
const HISTORY_NOTE: &str = "Nota: só os pacotes pedidos são registrados; dependências instaladas ou removidas junto não aparecem aqui.\n";

/// Aviso do `--skip-flatpak`: os apps que o preset lista e que esta execução não vai instalar.
/// Vazio se não havia nenhum.
pub fn render_skipped_flatpak(skipped: &[String]) -> String {
    if skipped.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "Flatpak pulado por --skip-flatpak: estes apps NÃO serão instalados ({}):\n",
        skipped.len()
    );
    for id in skipped {
        let _ = writeln!(out, "  {id}");
    }
    out
}

/// Até duas linhas dizendo o que `[machines.<rótulo>]` tirou do preset e o que acrescentou (nada
/// se não mexeu em nada).
pub fn render_machine_changes(
    label: &str,
    skipped: &nodus_core::preset::Items,
    added: &nodus_core::preset::Items,
) -> String {
    let line = |title: &str, items: &nodus_core::preset::Items| -> String {
        if items.is_empty() {
            return String::new();
        }
        let names: Vec<&str> = items
            .pacman
            .iter()
            .chain(&items.flatpak)
            .map(String::as_str)
            .collect();
        format!("{title} nesta máquina ({label}): {}\n", names.join(", "))
    };
    line("Itens pulados", skipped) + &line("Itens acrescentados", added)
}

pub fn render_plan(plan: &Plan) -> String {
    let mut out = String::from("Plano (nada foi executado):\n");
    let _ = writeln!(
        out,
        "Pacman (instalar {}, já presentes {}):",
        plan.pacman_missing.len(),
        plan.pacman_present
    );
    for name in &plan.pacman_missing {
        let _ = writeln!(out, "  {name}");
    }
    if !plan.pacman_unavailable.is_empty() {
        let _ = writeln!(
            out,
            "Indisponíveis nos repositórios, o nodus não instala ({}):",
            plan.pacman_unavailable.len()
        );
        for name in &plan.pacman_unavailable {
            let _ = writeln!(out, "  {name}");
        }
    }
    if plan.flatpak_installed {
        let _ = writeln!(
            out,
            "Flatpak (instalar {}, já presentes {}):",
            plan.flatpak_missing.len(),
            plan.flatpak_present
        );
    } else {
        let _ = writeln!(
            out,
            "Flatpak não instalado (apps do preset: {}):",
            plan.flatpak_missing.len()
        );
    }
    for id in &plan.flatpak_missing {
        let _ = writeln!(out, "  {id}");
    }
    if plan.is_empty() {
        out.push_str(if plan.pacman_unavailable.is_empty() {
            "Nada a fazer: a máquina já está como o preset.\n"
        } else {
            "Nada a instalar: só restam itens indisponíveis nos repositórios.\n"
        });
    }
    out
}

/// Troca byte de controle por `?`: o SO vem de um arquivo de outra máquina (a leitura não o valida) e
/// um `ESC` ali mexeria no terminal de quem lê o relatório. Rótulos e nomes de item já são validados.
fn plain(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

fn os_label(os: &SnapshotOs) -> String {
    match &os.version {
        Some(version) => plain(&format!("{} {version}", os.name)),
        None => plain(&os.name),
    }
}

fn comparison_section(
    out: &mut String,
    title: &str,
    comparison: &Comparison,
    list: &ListComparison,
) {
    match list {
        ListComparison::Compared {
            only_a,
            only_b,
            common,
        } => {
            let _ = writeln!(out, "{title}: {common} em comum");
            for (label, only) in [(&comparison.a, only_a), (&comparison.b, only_b)] {
                if only.is_empty() {
                    continue;
                }
                let _ = writeln!(out, "  só em {label} ({}):", only.len());
                for name in only {
                    let _ = writeln!(out, "    {name}");
                }
            }
        }
        ListComparison::NotComparable { lacks } => {
            let who = lacks.join(" e ");
            let verb = if lacks.len() == 1 {
                "não tem"
            } else {
                "não têm"
            };
            let _ = writeln!(out, "{title}: não comparável, {who} {verb} {title}");
        }
    }
}

pub fn render_comparison(comparison: &Comparison) -> String {
    let mut out = format!(
        "Comparando {} com {} (só leitura):\n",
        comparison.a, comparison.b
    );
    if let Some(os) = &comparison.os {
        let _ = writeln!(
            out,
            "Aviso: sistemas diferentes: {} é {}, {} é {}",
            comparison.a,
            os_label(&os.a),
            comparison.b,
            os_label(&os.b)
        );
    }
    comparison_section(&mut out, "Pacman", comparison, &comparison.pacman);
    comparison_section(&mut out, "Flatpak", comparison, &comparison.flatpak);
    dotfiles_section(&mut out, comparison);
    if !comparison.has_differences() {
        out.push_str(if comparison.has_not_comparable() {
            "Sem divergências nas listas comparadas.\n"
        } else {
            "Sem divergências.\n"
        });
    }
    out
}

fn dotfile_state_label(state: Option<DotfileState>) -> &'static str {
    match state {
        Some(DotfileState::Linked) => "ligado",
        Some(DotfileState::File) => "arquivo",
        Some(DotfileState::Missing) => "ausente",
        Some(DotfileState::Other) => "outro",
        None => "fora da lista",
    }
}

/// Dotfiles do `compare`: sem seção quando as duas olharam e nenhuma lista tem caminho; "não
/// comparável" cita quem não olhou (snapshot v1) e como resolver. O caminho vem de um arquivo de
/// outra máquina: nada de byte de controle na tela.
fn dotfiles_section(out: &mut String, comparison: &Comparison) {
    match &comparison.dotfiles {
        DotfilesComparison::Compared { differ, same } => {
            if differ.is_empty() && *same == 0 {
                return;
            }
            let _ = writeln!(out, "Dotfiles: {same} em comum");
            if differ.is_empty() {
                return;
            }
            let _ = writeln!(out, "  diferem ({}):", differ.len());
            for diff in differ {
                let _ = writeln!(
                    out,
                    "    {}: {} {}, {} {}",
                    plain(&diff.path),
                    comparison.a,
                    dotfile_state_label(diff.a),
                    comparison.b,
                    dotfile_state_label(diff.b)
                );
            }
        }
        DotfilesComparison::NotComparable { lacks } => {
            let verb = if lacks.len() == 1 {
                "não registrou"
            } else {
                "não registraram"
            };
            let _ = writeln!(
                out,
                "Dotfiles: não comparável, {} {verb} dotfiles (snapshot v1; rode `nodusctl snapshot` de novo)",
                lacks.join(" e ")
            );
        }
    }
}

/// `compare --all`: um bloco por par, separados por uma linha em branco, e os snapshots ilegíveis
/// no fim. Com menos de dois legíveis não há par: diz quantos há (não é erro).
pub fn render_all_comparisons(
    readable: usize,
    comparisons: &[Comparison],
    unreadable: &[Unreadable],
) -> String {
    let mut out = if comparisons.is_empty() {
        format!("Preciso de pelo menos dois snapshots legíveis para comparar (há {readable}).\n")
    } else {
        comparisons
            .iter()
            .map(render_comparison)
            .collect::<Vec<_>>()
            .join("\n")
    };
    for bad in unreadable {
        let _ = writeln!(
            out,
            "Aviso: não consegui ler o snapshot {}: {}",
            plain(&bad.label),
            plain(&bad.reason)
        );
    }
    out
}

/// O snapshot guarda `pacman -Qe`: "ausente" inclui o que veio só como dependência.
const STATUS_NOTE: &str = "Nota: \"não instalado de propósito\" inclui o que veio só como dependência; o snapshot guarda `pacman -Qe`.\n";

fn status_section(out: &mut String, title: &str, machine: &str, list: &StatusList) {
    match list {
        StatusList::Compared {
            missing,
            extra,
            in_sync,
        } => {
            let _ = writeln!(out, "  {title}: {in_sync} em dia");
            let groups = [
                ("no preset e não instalado de propósito", missing),
                ("instalado de propósito e fora do preset", extra),
            ];
            for (heading, items) in groups {
                if items.is_empty() {
                    continue;
                }
                let _ = writeln!(out, "    {heading} ({}):", items.len());
                for name in items {
                    let _ = writeln!(out, "      {name}");
                }
            }
        }
        StatusList::NotComparable { .. } => {
            let _ = writeln!(out, "  {title}: não comparável, {machine} não tem {title}");
        }
    }
}

/// O trecho da exceção sugerida para a máquina (nada é escrito). Sem tabela, mostra a tabela
/// inteira para colar; com tabela, diz em qual lista acrescentar, porque repetir a chave seria
/// TOML inválido.
fn exceptions_section(out: &mut String, exceptions: &MachineExceptions) {
    let keys = [
        ("add_pacman", &exceptions.add.pacman),
        ("add_flatpak", &exceptions.add.flatpak),
        ("skip_pacman", &exceptions.skip.pacman),
        ("skip_flatpak", &exceptions.skip.flatpak),
    ];
    if exceptions.is_empty() {
        return;
    }
    let machine = &exceptions.machine;
    if exceptions.table_exists {
        let _ = writeln!(
            out,
            "  A tabela [machines.{machine}] já existe. Para registrar a exceção, acrescente (nada foi alterado):"
        );
        for (key, names) in keys.iter().filter(|(_, names)| !names.is_empty()) {
            let _ = writeln!(out, "    em `{key} = [...]`:");
            for name in *names {
                let _ = writeln!(out, "      \"{name}\",");
            }
        }
    } else {
        let _ = writeln!(
            out,
            "  Para registrar como exceção desta máquina, cole no preset (nada foi alterado):"
        );
        let _ = writeln!(out, "    [machines.{machine}]");
        for (key, names) in keys.iter().filter(|(_, names)| !names.is_empty()) {
            let quoted: Vec<String> = names.iter().map(|name| format!("\"{name}\"")).collect();
            let _ = writeln!(out, "    {key} = [{}]", quoted.join(", "));
        }
    }
}

/// O `preset status --json`: cada máquina é o `MachineStatus` mais `exceptions` (o mesmo trecho
/// que o texto imprime, ADR 0033 emenda 4). A chave está sempre presente, com listas vazias
/// quando não há exceção.
pub fn status_json(
    file: &PresetFile,
    statuses: &[MachineStatus],
    unreadable: &[Unreadable],
) -> Result<serde_json::Value, serde_json::Error> {
    let machines = statuses
        .iter()
        .map(|status| {
            let mut machine = serde_json::to_value(status)?;
            machine["exceptions"] = serde_json::to_value(exceptions_for(file, status))?;
            Ok(machine)
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    let unreadable: Vec<_> = unreadable
        .iter()
        .map(|u| serde_json::json!({"label": u.label, "reason": u.reason}))
        .collect();
    Ok(serde_json::json!({"machines": machines, "unreadable": unreadable}))
}

pub fn render_status(
    file: &PresetFile,
    statuses: &[MachineStatus],
    unreadable: &[Unreadable],
) -> String {
    if statuses.is_empty() && unreadable.is_empty() {
        return "Nenhum snapshot encontrado; rode `nodusctl snapshot` em cada máquina e traga os arquivos pelo git.\n".to_string();
    }
    let mut out = format!(
        "Preset: pacman {}, flatpak {} (só leitura, a partir dos snapshots):\n",
        file.preset.pacman.len(),
        file.preset.flatpak.len()
    );
    for status in statuses {
        let header = if !status.is_in_sync() {
            ""
        } else if status.has_not_comparable() {
            " em dia nas listas comparadas"
        } else {
            " em dia com o preset"
        };
        let _ = writeln!(
            out,
            "{} ({}):{header}",
            status.machine,
            os_label(&status.os)
        );
        status_section(&mut out, "Pacman", &status.machine, &status.pacman);
        status_section(&mut out, "Flatpak", &status.machine, &status.flatpak);
        exceptions_section(&mut out, &exceptions_for(file, status));
    }
    for bad in unreadable {
        let _ = writeln!(
            out,
            "Aviso: não consegui ler o snapshot {}: {}",
            plain(&bad.label),
            plain(&bad.reason)
        );
    }
    let missing_pacman = statuses
        .iter()
        .any(|s| matches!(&s.pacman, StatusList::Compared { missing, .. } if !missing.is_empty()));
    if missing_pacman {
        out.push_str(STATUS_NOTE);
    }
    out
}

pub fn render_suggestions(suggestions: &Suggestions) -> String {
    let mut out = String::new();
    if suggestions.is_empty() {
        out.push_str("Nada fora do preset: tudo o que foi instalado de propósito já está nele.\n");
    } else {
        out.push_str("Fora do preset (nada foi alterado):\n");
        list_section(&mut out, "Pacman", &suggestions.pacman_extra);
        list_section(
            &mut out,
            "Fora dos repositórios (AUR ou locais; o `apply` não os instala)",
            &suggestions.foreign_extra,
        );
        list_section(&mut out, "Flatpak", &suggestions.flatpak_extra);
        // O trecho para colar só leva o que o `apply` sabe instalar: sem os de fora dos repositórios.
        paste_section(&mut out, "pacman", &suggestions.pacman_extra);
        paste_section(&mut out, "flatpak", &suggestions.flatpak_extra);
    }
    if !suggestions.flatpak_installed {
        out.push_str("Flatpak não instalado: os apps Flatpak não foram conferidos.\n");
    }
    out
}

/// Rodapé do `suggest` sobre os itens ignorados. Sem `--all` só conta e diz como ver; com `--all` a
/// lista inteira já foi mostrada, então o rodapé diz quais dos listados estão ignorados.
pub fn render_ignored_note(hidden: &[String], all: bool) -> String {
    if hidden.is_empty() {
        return String::new();
    }
    if all {
        return format!(
            "Ignorados entre os listados ({}): {}\n",
            hidden.len(),
            hidden.join(", ")
        );
    }
    let noun = if hidden.len() == 1 {
        "ignorado"
    } else {
        "ignorados"
    };
    format!("{} {noun} (use --all para ver).\n", hidden.len())
}

/// O JSON das sugestões, com os nomes ignorados no campo `ignored` (escondidos, ou só marcados com
/// `--all`).
pub fn suggestions_json(
    suggestions: &Suggestions,
    ignored: &[String],
) -> serde_json::Result<serde_json::Value> {
    let mut value = serde_json::to_value(suggestions)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("ignored".into(), serde_json::json!(ignored));
    }
    Ok(value)
}

/// O que `preset ignored` mostra: as duas listas do config.
pub fn render_ignored(pacman: &[String], flatpak: &[String]) -> String {
    if pacman.is_empty() && flatpak.is_empty() {
        return "Nenhum item ignorado.\n".to_string();
    }
    let mut out = String::new();
    for (kind, names) in [("pacman", pacman), ("flatpak", flatpak)] {
        if names.is_empty() {
            continue;
        }
        let _ = writeln!(out, "Ignorados, {kind} ({}):", names.len());
        for name in names {
            let _ = writeln!(out, "  {name}");
        }
    }
    out
}

fn list_section(out: &mut String, title: &str, names: &[String]) {
    if names.is_empty() {
        return;
    }
    let _ = writeln!(out, "{title} ({}):", names.len());
    for name in names {
        let _ = writeln!(out, "  {name}");
    }
}

/// Linhas prontas para a lista `key = [...]` do preset. Os nomes já foram validados (sem aspas).
fn paste_section(out: &mut String, key: &str, names: &[String]) {
    if names.is_empty() {
        return;
    }
    let _ = writeln!(out, "Para adotar, acrescente em `{key} = [...]`:");
    for name in names {
        let _ = writeln!(out, "    \"{name}\",");
    }
}

pub fn render_workspace(report: &Report) -> String {
    let mut out = String::new();
    match &report.chosen {
        Some(chosen) => {
            let _ = writeln!(
                out,
                "Workspace: {} ({}, repositórios: {})",
                chosen.path.display(),
                chosen.source.label(),
                chosen.repo_count
            );
            if chosen.source == Source::Detected {
                out.push_str("Candidatos:\n");
                for candidate in &report.candidates {
                    let mark = if candidate.path == chosen.path {
                        "  <- escolhido"
                    } else {
                        ""
                    };
                    let _ = writeln!(
                        out,
                        "  {}  repositórios: {}{mark}",
                        candidate.path.display(),
                        candidate.repo_count
                    );
                }
            } else if chosen.repo_count == 0 {
                out.push_str("aviso: nenhum repositório Git encontrado nessa pasta\n");
            }
        }
        None => {
            out.push_str("Nenhum workspace detectado.\n");
            if !report.candidates.is_empty() {
                out.push_str("Candidatos sem repositórios:\n");
                for candidate in &report.candidates {
                    let _ = writeln!(out, "  {}", candidate.path.display());
                }
            }
            out.push_str(
                "Informe a pasta: `nodusctl workspace <pasta>` ou a variável NODUS_WORKSPACE.\n",
            );
        }
    }
    out
}

/// Uma linha por dotfile, com o caminho alinhado, e o resumo no fim.
pub fn render_dotfiles(entries: &[Entry]) -> String {
    use nodus_core::dotfiles::Kind;
    if entries.is_empty() {
        return "Nenhum dotfile na lista.\n".to_string();
    }
    let width = entries.iter().map(|e| e.path.len()).max().unwrap_or(0);
    let mut out = String::new();
    for entry in entries {
        let what = match &entry.kind {
            Kind::Missing => "ausente".to_string(),
            Kind::File { size } => format!("arquivo ({size} bytes)"),
            Kind::Dir => "pasta".to_string(),
            Kind::Other => "outro tipo".to_string(),
            Kind::Symlink {
                target,
                outside_home,
            } => {
                let note = if *outside_home { " (fora do HOME)" } else { "" };
                format!("symlink -> {target}{note}")
            }
        };
        let _ = writeln!(out, "{:<width$}  {what}", entry.path);
    }
    let present = entries.iter().filter(|e| e.kind != Kind::Missing).count();
    let _ = writeln!(out, "Presentes: {present} de {}.", entries.len());
    out
}

/// Aviso do `preset init` (vai para a saída de erro, para não sujar o TOML redirecionado).
pub fn init_warning(flatpak_installed: bool) -> Option<&'static str> {
    (!flatpak_installed).then_some("aviso: Flatpak não instalado; o preset saiu sem apps Flatpak")
}

/// Bytes em GiB (base 1024) com uma casa decimal.
fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1u64 << 30) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Candidate, Chosen, Report, Source};
    use nodus_core::inventory::{Disk, FlatpakApp, HardwareInfo, Inventory, Package, SystemInfo};
    use nodus_core::preset::{Plan, Suggestions};
    use nodus_core::updates::{FlatpakUpdate, PacmanUpdate};

    #[test]
    fn aviso_dos_apps_flatpak_pulados() {
        assert_eq!(
            render_skipped_flatpak(&["org.a.B".to_string(), "org.c.D".to_string()]),
            "Flatpak pulado por --skip-flatpak: estes apps NÃO serão instalados (2):\n  org.a.B\n  org.c.D\n"
        );
    }

    #[test]
    fn sem_app_pulado_nao_ha_aviso() {
        assert_eq!(render_skipped_flatpak(&[]), "");
    }

    fn inventario(os_version: Option<&str>) -> Inventory {
        Inventory {
            system: SystemInfo {
                os_name: "Arch Linux".into(),
                os_version: os_version.map(String::from),
                kernel: "6.12.1-arch1-1".into(),
            },
            hardware: HardwareInfo {
                cpu_model: "AMD Ryzen 5 5600".into(),
                cpu_threads: 12,
                memory_total_bytes: 34_359_738_368,
                disks: vec![Disk {
                    name: "/dev/sda3".into(),
                    mount_point: "/home".into(),
                    file_system: "ext4".into(),
                    total_bytes: 200_000_000_000,
                }],
            },
            packages: vec![
                Package {
                    name: "bash".into(),
                    version: "5.2.037-2".into(),
                },
                Package {
                    name: "linux".into(),
                    version: "6.12.1.arch1-1".into(),
                },
            ],
            flatpak_apps: Some(vec![FlatpakApp {
                application: "org.nongnu.lingot".into(),
                version: Some("1.1.1".into()),
                branch: "stable".into(),
                origin: "flathub".into(),
                installation: "user".into(),
            }]),
        }
    }

    #[test]
    fn texto_sem_versao_do_sistema() {
        let esperado = "Sistema: Arch Linux\nKernel: 6.12.1-arch1-1\nCPU: AMD Ryzen 5 5600 (12 threads)\nMemória: 32.0 GiB\nDiscos (1):\n  /home /dev/sda3 ext4 186.3 GiB\nPacotes (2):\n  bash 5.2.037-2\n  linux 6.12.1.arch1-1\nFlatpak (1):\n  org.nongnu.lingot 1.1.1 stable flathub user\n";
        assert_eq!(render_text(&inventario(None)), esperado);
    }

    #[test]
    fn texto_sem_discos() {
        let mut inv = inventario(None);
        inv.hardware.disks.clear();
        assert!(render_text(&inv).contains("Discos (0):\nPacotes (2):\n"));
    }

    #[test]
    fn texto_com_versao_do_sistema() {
        assert!(render_text(&inventario(Some("41"))).starts_with("Sistema: Arch Linux 41\n"));
    }

    #[test]
    fn texto_sem_pacotes() {
        let mut inv = inventario(None);
        inv.packages.clear();
        assert!(render_text(&inv).contains("Pacotes (0):\nFlatpak (1):\n"));
    }

    #[test]
    fn texto_flatpak_nao_instalado() {
        let mut inv = inventario(None);
        inv.flatpak_apps = None;
        assert!(render_text(&inv).ends_with("Flatpak: não instalado\n"));
    }

    #[test]
    fn texto_flatpak_sem_apps() {
        let mut inv = inventario(None);
        inv.flatpak_apps = Some(vec![]);
        assert!(render_text(&inv).ends_with("Flatpak (0):\n"));
    }

    fn atualizacoes(fonte: PacmanSource) -> Updates {
        Updates {
            pacman_source: fonte,
            pacman: vec![PacmanUpdate {
                name: "bash".into(),
                old_version: "5.2-1".into(),
                new_version: "5.3-1".into(),
            }],
            flatpak: Some(vec![FlatpakUpdate {
                application: "org.example.App".into(),
                version: None,
                branch: "stable".into(),
            }]),
        }
    }

    fn repo(nome: &str) -> RepoStatus {
        RepoStatus {
            name: nome.into(),
            path: format!("/ws/{nome}"),
            branch: Some("main".into()),
            commit: Some("1a2b3c4".into()),
            upstream: Some("origin/main".into()),
            ahead: Some(1),
            behind: Some(2),
            upstream_gone: false,
            origin_url: Some("git@github.com:x/y.git".into()),
            fetch: None,
            error: None,
        }
    }

    #[test]
    fn repos_texto_completo() {
        let esperado = "Repositórios (1):\n  arca main 1a2b3c4 origin/main (à frente 1, atrás 2) git@github.com:x/y.git\n";
        assert_eq!(render_repos(&[repo("arca")]), esperado);
    }

    #[test]
    fn repos_texto_head_solto_sem_commits_e_sem_upstream() {
        let mut solto = repo("a");
        solto.branch = None;
        solto.upstream = None;
        solto.ahead = None;
        solto.behind = None;
        solto.origin_url = None;
        assert_eq!(
            render_repos(&[solto]),
            "Repositórios (1):\n  a (HEAD solto) 1a2b3c4 sem upstream sem origin\n"
        );
        let mut vazio = repo("b");
        vazio.branch = None;
        vazio.commit = None;
        vazio.upstream = None;
        vazio.ahead = None;
        vazio.behind = None;
        vazio.origin_url = None;
        assert!(render_repos(&[vazio]).contains("  b (sem commits) sem upstream sem origin\n"));
    }

    #[test]
    fn repos_texto_erro_fetch_e_upstream_sumido() {
        let mut com_erro = repo("e");
        com_erro.error = Some("git terminou com erro".into());
        assert!(render_repos(&[com_erro]).contains("  e ERRO: git terminou com erro\n"));
        let mut falho = repo("f");
        falho.fetch = Some(FetchResult::Failed("sem remoto origin".into()));
        assert!(render_repos(&[falho]).contains("[fetch falhou: sem remoto origin]"));
        let mut gone = repo("g");
        gone.upstream_gone = true;
        gone.ahead = None;
        gone.behind = None;
        assert!(render_repos(&[gone]).contains("origin/main (sumiu)"));
    }

    #[test]
    fn repos_texto_fetch_ok() {
        let mut ok = repo("d");
        ok.fetch = Some(FetchResult::Done);
        let esperado = "Repositórios (1):\n  d main 1a2b3c4 origin/main (à frente 1, atrás 2) git@github.com:x/y.git [fetch ok]\n";
        assert_eq!(render_repos(&[ok]), esperado);
    }

    fn plano(pacman: &[&str], flatpak: &[&str], flatpak_installed: bool) -> Plan {
        Plan {
            pacman_missing: pacman.iter().map(|s| s.to_string()).collect(),
            pacman_present: 2,
            pacman_unavailable: vec![],
            flatpak_missing: flatpak.iter().map(|s| s.to_string()).collect(),
            flatpak_present: 1,
            flatpak_installed,
        }
    }

    #[test]
    fn plano_texto_com_faltantes() {
        let esperado = "Plano (nada foi executado):\nPacman (instalar 2, já presentes 2):\n  zsh\n  neovim\nFlatpak (instalar 1, já presentes 1):\n  org.a.B\n";
        assert_eq!(
            render_plan(&plano(&["zsh", "neovim"], &["org.a.B"], true)),
            esperado
        );
    }

    #[test]
    fn plano_texto_vazio_diz_que_nada_a_fazer() {
        let esperado = "Plano (nada foi executado):\nPacman (instalar 0, já presentes 2):\nFlatpak (instalar 0, já presentes 1):\nNada a fazer: a máquina já está como o preset.\n";
        assert_eq!(render_plan(&plano(&[], &[], true)), esperado);
    }

    #[test]
    fn plano_texto_mostra_os_indisponiveis_a_parte_depois_do_pacman() {
        let mut p = plano(&["zsh"], &[], true);
        p.pacman_unavailable = vec!["brave-bin".into(), "google-chrome".into()];
        let esperado = "Plano (nada foi executado):\nPacman (instalar 1, já presentes 2):\n  zsh\nIndisponíveis nos repositórios, o nodus não instala (2):\n  brave-bin\n  google-chrome\nFlatpak (instalar 0, já presentes 1):\n";
        assert_eq!(render_plan(&p), esperado);
    }

    #[test]
    fn plano_so_com_indisponiveis_nao_diz_que_a_maquina_ja_esta_como_o_preset() {
        let mut p = plano(&[], &[], true);
        p.pacman_unavailable = vec!["brave-bin".into()];
        let texto = render_plan(&p);
        assert!(
            texto.contains(
                "Indisponíveis nos repositórios, o nodus não instala (1):\n  brave-bin\n"
            )
        );
        assert!(
            texto.ends_with("Nada a instalar: só restam itens indisponíveis nos repositórios.\n")
        );
        assert!(!texto.contains("a máquina já está como o preset"));
    }

    #[test]
    fn plano_sem_indisponiveis_nao_tem_a_secao() {
        assert!(!render_plan(&plano(&["zsh"], &[], true)).contains("Indisponíveis"));
    }

    #[test]
    fn plano_texto_sem_flatpak_instalado() {
        let mut p = plano(&[], &["org.a.B"], false);
        p.flatpak_present = 0;
        assert!(
            render_plan(&p).ends_with("Flatpak não instalado (apps do preset: 1):\n  org.a.B\n")
        );
    }

    fn sugestoes(
        pacman: &[&str],
        foreign: &[&str],
        flatpak: &[&str],
        flatpak_installed: bool,
    ) -> Suggestions {
        let v = |itens: &[&str]| itens.iter().map(|s| s.to_string()).collect();
        Suggestions {
            pacman_extra: v(pacman),
            foreign_extra: v(foreign),
            flatpak_extra: v(flatpak),
            flatpak_installed,
        }
    }

    #[test]
    fn sugestoes_texto_completo_com_o_trecho_para_colar() {
        let esperado = "Fora do preset (nada foi alterado):\n\
Pacman (2):\n  vim\n  zsh\n\
Fora dos repositórios (AUR ou locais; o `apply` não os instala) (1):\n  brave-bin\n\
Flatpak (1):\n  org.gnome.Calculator\n\
Para adotar, acrescente em `pacman = [...]`:\n    \"vim\",\n    \"zsh\",\n\
Para adotar, acrescente em `flatpak = [...]`:\n    \"org.gnome.Calculator\",\n";
        assert_eq!(
            render_suggestions(&sugestoes(
                &["vim", "zsh"],
                &["brave-bin"],
                &["org.gnome.Calculator"],
                true
            )),
            esperado
        );
    }

    #[test]
    fn sugestoes_texto_omite_secao_vazia_e_nao_poe_estrangeiro_no_trecho() {
        let texto = render_suggestions(&sugestoes(&[], &["brave-bin"], &[], true));
        assert_eq!(
            texto,
            "Fora do preset (nada foi alterado):\n\
Fora dos repositórios (AUR ou locais; o `apply` não os instala) (1):\n  brave-bin\n"
        );
        assert!(!texto.contains("Para adotar"));
        assert!(!texto.contains("\"brave-bin\""));
    }

    #[test]
    fn sugestoes_texto_vazio_diz_que_nada_esta_fora() {
        assert_eq!(
            render_suggestions(&sugestoes(&[], &[], &[], true)),
            "Nada fora do preset: tudo o que foi instalado de propósito já está nele.\n"
        );
    }

    #[test]
    fn sugestoes_texto_avisa_quando_o_flatpak_nao_esta_instalado() {
        let texto = render_suggestions(&sugestoes(&["vim"], &[], &[], false));
        assert!(texto.ends_with("Flatpak não instalado: os apps Flatpak não foram conferidos.\n"));
        assert!(texto.contains("Pacman (1):\n  vim\n"));
    }

    fn ws(path: &str, source: Source, repo_count: usize) -> Chosen {
        Chosen {
            path: path.into(),
            source,
            repo_count,
        }
    }

    fn cand(path: &str, repo_count: usize) -> Candidate {
        Candidate {
            path: path.into(),
            repo_count,
        }
    }

    #[test]
    fn workspace_detectado_lista_os_candidatos_e_marca_o_escolhido() {
        let r = Report {
            chosen: Some(ws("/h/ws", Source::Detected, 3)),
            candidates: vec![cand("/h/ws", 3), cand("/h/Projects", 0)],
        };
        let esperado = "Workspace: /h/ws (detectado, repositórios: 3)\n\
Candidatos:\n  /h/ws  repositórios: 3  <- escolhido\n  /h/Projects  repositórios: 0\n";
        assert_eq!(render_workspace(&r), esperado);
    }

    #[test]
    fn workspace_informado_ou_do_ambiente_nao_lista_candidatos() {
        let informado = Report {
            chosen: Some(ws("/x", Source::Informed, 2)),
            candidates: vec![],
        };
        assert_eq!(
            render_workspace(&informado),
            "Workspace: /x (informado, repositórios: 2)\n"
        );
        let ambiente = Report {
            chosen: Some(ws("/x", Source::Environment, 2)),
            candidates: vec![],
        };
        assert_eq!(
            render_workspace(&ambiente),
            "Workspace: /x (ambiente, repositórios: 2)\n"
        );
    }

    #[test]
    fn workspace_manual_sem_repositorios_avisa() {
        let r = Report {
            chosen: Some(ws("/x", Source::Informed, 0)),
            candidates: vec![],
        };
        assert_eq!(
            render_workspace(&r),
            "Workspace: /x (informado, repositórios: 0)\naviso: nenhum repositório Git encontrado nessa pasta\n"
        );
    }

    #[test]
    fn workspace_nenhum_detectado_lista_os_vazios_e_ensina_a_informar() {
        let r = Report {
            chosen: None,
            candidates: vec![cand("/h/Projects", 0)],
        };
        let esperado = "Nenhum workspace detectado.\n\
Candidatos sem repositórios:\n  /h/Projects\n\
Informe a pasta: `nodusctl workspace <pasta>` ou a variável NODUS_WORKSPACE.\n";
        assert_eq!(render_workspace(&r), esperado);
    }

    #[test]
    fn workspace_nenhum_detectado_e_nenhum_candidato() {
        let r = Report {
            chosen: None,
            candidates: vec![],
        };
        assert_eq!(
            render_workspace(&r),
            "Nenhum workspace detectado.\nInforme a pasta: `nodusctl workspace <pasta>` ou a variável NODUS_WORKSPACE.\n"
        );
    }

    #[test]
    fn aviso_do_init_so_quando_falta_o_flatpak() {
        assert_eq!(
            init_warning(false),
            Some("aviso: Flatpak não instalado; o preset saiu sem apps Flatpak")
        );
        assert_eq!(init_warning(true), None);
    }

    #[test]
    fn repos_texto_sem_repositorios() {
        assert_eq!(render_repos(&[]), "Repositórios (0):\n");
    }

    #[test]
    fn updates_texto_com_checkupdates() {
        let esperado =
            "Pacman (1):\n  bash 5.2-1 -> 5.3-1\nFlatpak (1):\n  org.example.App - stable\n";
        assert_eq!(
            render_updates(&atualizacoes(PacmanSource::Checkupdates)),
            esperado
        );
    }

    #[test]
    fn updates_texto_avisa_quando_a_fonte_e_pacman_qu() {
        let texto = render_updates(&atualizacoes(PacmanSource::PacmanQu));
        assert!(texto.starts_with("Pacman (1) [pode estar desatualizado"));
    }

    #[test]
    fn updates_texto_flatpak_nao_instalado() {
        let mut up = atualizacoes(PacmanSource::Checkupdates);
        up.flatpak = None;
        assert!(render_updates(&up).ends_with("Flatpak: não instalado\n"));
    }

    #[test]
    fn texto_flatpak_sem_versao_mostra_traco() {
        let mut inv = inventario(None);
        inv.flatpak_apps = Some(vec![FlatpakApp {
            application: "org.example.App".into(),
            version: None,
            branch: "stable".into(),
            origin: "flathub".into(),
            installation: "system".into(),
        }]);
        assert!(render_text(&inv).ends_with("  org.example.App - stable flathub system\n"));
    }

    // --- render_history ---

    use nodus_core::history::{Action, ExecResult, HistoryEntry, Manager};

    /// Uma reversão: `installed` = ainda instalados depois; `missing` = removidos.
    fn remocao(
        at: u64,
        undoes: u32,
        requested: &[&str],
        installed: &[&str],
        missing: &[&str],
        result: ExecResult,
    ) -> HistoryEntry {
        let mut e = execucao(at, requested, installed, missing, result);
        e.action = Action::Remove;
        e.undoes = Some(undoes);
        e.command = vec!["sudo".into(), "pacman".into(), "-Rs".into(), "--".into()];
        e
    }

    fn execucao(
        at: u64,
        requested: &[&str],
        installed: &[&str],
        missing: &[&str],
        result: ExecResult,
    ) -> HistoryEntry {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        HistoryEntry {
            version: 3,
            action: Action::Install,
            undoes: None,
            manager: Manager::Pacman,
            dir: None,
            at_unix: at,
            command: v(&["sudo", "pacman", "-S", "--needed", "--"]),
            requested: v(requested),
            installed: v(installed),
            still_missing: v(missing),
            result,
        }
    }

    /// A nota que fecha o histórico (ADR 0011, item 8): só o pedido é registrado.
    const NOTA: &str = "Nota: só os pacotes pedidos são registrados; dependências instaladas ou removidas junto não aparecem aqui.\n";

    #[test]
    fn historico_marca_a_entrada_flatpak_e_nao_marca_a_do_pacman() {
        let mut flatpak = execucao(
            1_700_000_000,
            &["org.gnome.Gedit"],
            &["org.gnome.Gedit"],
            &[],
            ExecResult::Ok,
        );
        flatpak.manager = Manager::Flatpak;
        let saida = render_history(&[flatpak]);
        assert!(
            saida.contains("  ok  flatpak  pedidos 1, instalados 1, faltando 0"),
            "{saida}"
        );
        let pacman = execucao(1_700_000_000, &["sl"], &["sl"], &[], ExecResult::Ok);
        assert!(!render_history(&[pacman]).contains("flatpak"));
    }

    #[test]
    fn historico_mostra_a_adocao_de_dotfile_com_o_caminho_e_a_pasta_fonte() {
        let adocao = nodus_core::history::entry_from_adopt(
            ".bashrc",
            "/home/u/dotfiles",
            &["nodusctl".to_string()],
            1_700_000_000,
        );
        let saida = render_history(&[adocao]);
        assert!(
            saida.contains("[1] 2023-11-14T22:13:20Z  ok  dotfile adotado: .bashrc  (pasta-fonte /home/u/dotfiles)\n"),
            "{saida}"
        );
        // A linha da entrada não usa o formato de pacotes (a nota de rodapé fala de "pedidos").
        let linha = saida.lines().nth(1).unwrap_or_default();
        assert!(!linha.contains("pedidos"), "{saida}");
    }

    #[test]
    fn historico_distingue_a_reversao_de_uma_ligacao_da_de_uma_adocao() {
        let comando = ["nodusctl".to_string()];
        let adocao = nodus_core::history::entry_from_adopt(
            ".bashrc",
            "/home/u/dotfiles",
            &comando,
            1_700_000_000,
        );
        let ligacao = nodus_core::history::entry_from_link(
            ".profile",
            "/home/u/dotfiles",
            &comando,
            1_700_000_100,
        );
        let desfaz = |n| {
            nodus_core::history::entry_for_dotfile_undo(
                n,
                if n == 1 { ".bashrc" } else { ".profile" },
                "/home/u/dotfiles",
                &comando,
                1_700_000_400,
            )
        };
        let saida = render_history(&[adocao, ligacao, desfaz(1), desfaz(2)]);
        assert!(
            saida.contains(
                "desfez [1]  dotfile restaurado: .bashrc  (pasta-fonte /home/u/dotfiles)\n"
            ),
            "{saida}"
        );
        assert!(
            saida.contains(
                "desfez [2]  ligação desfeita: .profile  (pasta-fonte /home/u/dotfiles)\n"
            ),
            "{saida}"
        );
    }

    #[test]
    fn historico_vazio_diz_que_nao_ha_execucoes() {
        assert_eq!(render_history(&[]), "Nenhuma execução registrada.\n");
    }

    #[test]
    fn historico_com_entradas_termina_com_a_nota_uma_vez_so() {
        let entradas: Vec<_> = (0..3)
            .map(|i| execucao(i, &["sl"], &["sl"], &[], ExecResult::Ok))
            .collect();
        let texto = render_history(&entradas);
        assert!(texto.ends_with(NOTA), "{texto}");
        assert_eq!(texto.matches("Nota:").count(), 1, "{texto}");
    }

    #[test]
    fn historico_de_remocao_tambem_termina_com_a_nota() {
        let e = remocao(0, 1, &["sl"], &[], &["sl"], ExecResult::Ok);
        assert!(render_history(&[e]).ends_with(NOTA));
    }

    #[test]
    fn historico_com_sucesso_lista_instalados() {
        let e = execucao(
            1_794_626_444,
            &["sl", "cowsay"],
            &["sl", "cowsay"],
            &[],
            ExecResult::Ok,
        );
        assert_eq!(
            render_history(&[e]),
            format!(
                "Histórico (1 execução):\n\
                 [1] 2026-11-14T03:20:44Z  ok  pedidos 2, instalados 2, faltando 0\n  instalados: sl, cowsay\n{NOTA}"
            )
        );
    }

    #[test]
    fn historico_numera_a_partir_de_1_em_ordem_de_gravacao() {
        let entradas: Vec<_> = (0..11)
            .map(|i| execucao(i, &["sl"], &["sl"], &[], ExecResult::Ok))
            .collect();
        let texto = render_history(&entradas);
        assert!(texto.contains("\n[1] 1970-01-01T00:00:00Z  ok"), "{texto}");
        assert!(texto.contains("\n[2] 1970-01-01T00:00:01Z  ok"), "{texto}");
        assert!(texto.contains("\n[11] 1970-01-01T00:00:10Z  ok"), "{texto}");
        assert!(!texto.contains("[0]"), "{texto}");
    }

    #[test]
    fn historico_mostra_a_remocao_e_qual_entrada_ela_desfez() {
        let instalou = execucao(0, &["sl", "cowsay"], &["sl", "cowsay"], &[], ExecResult::Ok);
        let desfez = remocao(
            1_794_626_444,
            1,
            &["sl", "cowsay"],
            &["cowsay"],
            &["sl"],
            ExecResult::Failed { code: Some(1) },
        );
        assert_eq!(
            render_history(&[instalou, desfez]),
            format!(
                "Histórico (2 execuções):\n\
                 [1] 1970-01-01T00:00:00Z  ok  pedidos 2, instalados 2, faltando 0\n  instalados: sl, cowsay\n\
                 [2] 2026-11-14T03:20:44Z  falhou (código 1)  desfez [1]  pedidos 2, removidos 1, ainda instalados 1\n  removidos: sl\n  ainda instalados: cowsay\n{NOTA}"
            )
        );
    }

    #[test]
    fn historico_de_remocao_total_nao_lista_ainda_instalados() {
        let e = remocao(0, 3, &["sl"], &[], &["sl"], ExecResult::Ok);
        let texto = render_history(&[e]);
        assert!(
            texto.contains("[1] 1970-01-01T00:00:00Z  ok  desfez [3]  pedidos 1, removidos 1, ainda instalados 0\n"),
            "{texto}"
        );
        assert!(
            texto.ends_with(&format!("  removidos: sl\n{NOTA}")),
            "{texto}"
        );
        assert!(!texto.contains("ainda instalados:"), "{texto}");
    }

    #[test]
    fn historico_com_falha_mostra_codigo_e_faltantes() {
        let ok = execucao(0, &["sl"], &["sl"], &[], ExecResult::Ok);
        let falha = execucao(
            1_794_626_444,
            &["sl", "cowsay"],
            &["sl"],
            &["cowsay"],
            ExecResult::Failed { code: Some(1) },
        );
        assert_eq!(
            render_history(&[ok, falha]),
            format!(
                "Histórico (2 execuções):\n\
                 [1] 1970-01-01T00:00:00Z  ok  pedidos 1, instalados 1, faltando 0\n  instalados: sl\n\
                 [2] 2026-11-14T03:20:44Z  falhou (código 1)  pedidos 2, instalados 1, faltando 1\n  instalados: sl\n  faltando: cowsay\n{NOTA}"
            )
        );
    }

    #[test]
    fn nota_de_ignorados_sem_nenhum_e_vazia() {
        assert_eq!(render_ignored_note(&[], false), "");
        assert_eq!(render_ignored_note(&[], true), "");
    }

    #[test]
    fn nota_de_ignorados_conta_e_manda_usar_all() {
        let um = vec!["a".to_string()];
        assert_eq!(
            render_ignored_note(&um, false),
            "1 ignorado (use --all para ver).\n"
        );
        let tres: Vec<String> = ["a", "b", "org.x.A"].map(String::from).to_vec();
        assert_eq!(
            render_ignored_note(&tres, false),
            "3 ignorados (use --all para ver).\n"
        );
    }

    #[test]
    fn nota_de_ignorados_com_all_diz_quais_dos_listados_estao_ignorados() {
        let dois: Vec<String> = ["a", "org.x.A"].map(String::from).to_vec();
        assert_eq!(
            render_ignored_note(&dois, true),
            "Ignorados entre os listados (2): a, org.x.A\n"
        );
    }

    #[test]
    fn json_das_sugestoes_traz_os_campos_de_sempre_e_os_ignorados() {
        let escondidos = vec!["x".to_string()];
        let json = suggestions_json(
            &sugestoes(&["a"], &["aur"], &["org.x.A"], true),
            &escondidos,
        )
        .unwrap();
        assert_eq!(json["pacman_extra"], serde_json::json!(["a"]));
        assert_eq!(json["foreign_extra"], serde_json::json!(["aur"]));
        assert_eq!(json["flatpak_extra"], serde_json::json!(["org.x.A"]));
        assert_eq!(json["flatpak_installed"], serde_json::json!(true));
        assert_eq!(json["ignored"], serde_json::json!(["x"]));
        let sem = suggestions_json(&sugestoes(&[], &[], &[], true), &[]).unwrap();
        assert_eq!(sem["ignored"], serde_json::json!([]));
    }

    #[test]
    fn lista_de_ignorados_em_texto() {
        let pacman: Vec<String> = ["a", "b"].map(String::from).to_vec();
        let flatpak = vec!["org.x.A".to_string()];
        assert_eq!(
            render_ignored(&pacman, &flatpak),
            "Ignorados, pacman (2):\n  a\n  b\nIgnorados, flatpak (1):\n  org.x.A\n"
        );
        assert_eq!(
            render_ignored(&[], &flatpak),
            "Ignorados, flatpak (1):\n  org.x.A\n"
        );
        assert_eq!(render_ignored(&[], &[]), "Nenhum item ignorado.\n");
    }

    #[test]
    fn historico_com_falha_sem_codigo() {
        let e = execucao(0, &["sl"], &[], &["sl"], ExecResult::Failed { code: None });
        let texto = render_history(&[e]);
        assert!(
            texto.contains("  falhou  pedidos 1, instalados 0, faltando 1\n")
                && texto.starts_with("Histórico (1 execução):\n[1] "),
            "{texto}"
        );
        assert!(texto.ends_with(&format!("  faltando: sl\n{NOTA}")));
        assert!(!texto.contains("instalados:"));
    }

    fn entrada(path: &str, kind: nodus_core::dotfiles::Kind) -> nodus_core::dotfiles::Entry {
        nodus_core::dotfiles::Entry {
            path: path.into(),
            kind,
        }
    }

    #[test]
    fn dotfiles_em_texto_uma_linha_por_item_e_o_resumo() {
        use nodus_core::dotfiles::Kind;
        let entradas = vec![
            entrada(".bashrc", Kind::File { size: 1670 }),
            entrada(
                ".profile",
                Kind::Symlink {
                    target: "dotfiles/profile".into(),
                    outside_home: false,
                },
            ),
            entrada(".gitconfig", Kind::Missing),
            entrada(
                ".vimrc",
                Kind::Symlink {
                    target: "/etc/vimrc".into(),
                    outside_home: true,
                },
            ),
            entrada(".config/git", Kind::Dir),
            entrada(".tmux.conf", Kind::Other),
        ];
        let texto = render_dotfiles(&entradas);

        let linha = |path: &str| {
            texto
                .lines()
                .find(|l| l.starts_with(path))
                .unwrap_or_else(|| panic!("sem linha de {path}: {texto}"))
        };
        assert!(linha(".bashrc").ends_with("arquivo (1670 bytes)"));
        assert!(linha(".profile").ends_with("symlink -> dotfiles/profile"));
        assert!(linha(".gitconfig").ends_with("ausente"));
        assert!(linha(".vimrc").ends_with("symlink -> /etc/vimrc (fora do HOME)"));
        assert!(linha(".config/git").ends_with("pasta"));
        assert!(linha(".tmux.conf").ends_with("outro tipo"));
        assert!(texto.ends_with("Presentes: 5 de 6.\n"), "{texto}");
    }

    #[test]
    fn dotfiles_com_lista_vazia_diz_que_nao_ha_nada() {
        assert_eq!(render_dotfiles(&[]), "Nenhum dotfile na lista.\n");
    }

    // --- compare (ADR 0031, fatia 2) ---

    fn snapshot_de(
        label: &str,
        os: &str,
        pacman: &[&str],
        flatpak: Option<&[&str]>,
    ) -> nodus_core::snapshot::Snapshot {
        let os = nodus_core::inventory::OsInfo {
            name: os.into(),
            version: None,
        };
        let machine = nodus_core::preset::MachinePreset {
            preset: nodus_core::preset::Preset {
                pacman: pacman.iter().map(|s| s.to_string()).collect(),
                flatpak: flatpak
                    .unwrap_or(&[])
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            },
            flatpak_installed: flatpak.is_some(),
        };
        // As máquinas "olharam" os dotfiles (lista vazia): uma seção de dotfiles sem itens não sai.
        nodus_core::snapshot::build(label, &os, &machine)
            .unwrap()
            .with_dotfiles(vec![])
            .unwrap()
    }

    fn com_dotfiles(
        snapshot: nodus_core::snapshot::Snapshot,
        estados: &[(&str, nodus_core::snapshot::DotfileState)],
    ) -> nodus_core::snapshot::Snapshot {
        let entradas = estados
            .iter()
            .map(|(path, state)| nodus_core::snapshot::DotfileEntry {
                path: (*path).into(),
                state: *state,
            })
            .collect();
        snapshot.with_dotfiles(entradas).unwrap()
    }

    fn snapshot_v1(label: &str) -> nodus_core::snapshot::Snapshot {
        nodus_core::snapshot::parse(&format!(
            "{{\"version\": 1, \"name\": \"{label}\", \"os\": {{\"name\": \"Arch Linux\", \"version\": null}}, \
             \"capabilities\": {{\"pacman\": true, \"flatpak\": true}}, \"pacman\": [\"base\"], \"flatpak\": []}}"
        ))
        .unwrap()
    }

    // --- compare --all (Fase 2, fatia 7) ---

    fn comparar(
        a: &nodus_core::snapshot::Snapshot,
        b: &nodus_core::snapshot::Snapshot,
    ) -> nodus_core::compare::Comparison {
        nodus_core::compare::compare(a, b)
    }

    fn ilegivel(label: &str, reason: &str) -> crate::snapshot::Unreadable {
        crate::snapshot::Unreadable {
            label: label.into(),
            reason: reason.into(),
        }
    }

    #[test]
    fn compare_all_com_menos_de_dois_snapshots_diz_quantos_ha_e_nao_compara() {
        assert_eq!(
            render_all_comparisons(0, &[], &[]),
            "Preciso de pelo menos dois snapshots legíveis para comparar (há 0).\n"
        );
        assert_eq!(
            render_all_comparisons(1, &[], &[]),
            "Preciso de pelo menos dois snapshots legíveis para comparar (há 1).\n"
        );
    }

    #[test]
    fn compare_all_com_um_par_e_igual_ao_compare_de_dois_rotulos() {
        let a = snapshot_de("pc-a", "Arch Linux", &["base", "git"], Some(&["org.a.A"]));
        let b = snapshot_de("pc-b", "Arch Linux", &["base"], None);
        let par = comparar(&a, &b);
        assert_eq!(
            render_all_comparisons(2, std::slice::from_ref(&par), &[]),
            render_comparison(&par)
        );
    }

    #[test]
    fn compare_all_separa_os_pares_por_uma_linha_em_branco_na_ordem_dada() {
        let a = snapshot_de("pc-a", "Arch Linux", &["base"], Some(&[]));
        let b = snapshot_de("pc-b", "Arch Linux", &["base", "vim"], Some(&[]));
        let c = snapshot_de("pc-c", "Arch Linux", &["base"], Some(&[]));
        let pares = [comparar(&a, &b), comparar(&a, &c), comparar(&b, &c)];
        let texto = render_all_comparisons(3, &pares, &[]);
        let esperado = format!(
            "{}\n{}\n{}",
            render_comparison(&pares[0]),
            render_comparison(&pares[1]),
            render_comparison(&pares[2])
        );
        assert_eq!(texto, esperado);
        assert_eq!(texto.matches("\n\nComparando ").count(), 2, "{texto}");
    }

    #[test]
    fn compare_all_lista_os_snapshots_ilegiveis_no_fim_e_sem_byte_de_controle() {
        let a = snapshot_de("pc-a", "Arch Linux", &["base"], Some(&[]));
        let b = snapshot_de("pc-b", "Arch Linux", &["base"], Some(&[]));
        let par = comparar(&a, &b);
        let texto = render_all_comparisons(
            2,
            std::slice::from_ref(&par),
            &[ilegivel("ruim", "JSON inválido\x1b[31m")],
        );
        assert!(texto.starts_with(&render_comparison(&par)), "{texto}");
        assert!(
            texto.ends_with("Aviso: não consegui ler o snapshot ruim: JSON inválido?[31m\n"),
            "{texto:?}"
        );
        assert!(!texto.contains('\x1b'), "{texto:?}");
    }

    #[test]
    fn compare_all_com_poucos_snapshots_ainda_mostra_os_avisos() {
        let texto = render_all_comparisons(1, &[], &[ilegivel("ruim", "não é JSON")]);
        assert_eq!(
            texto,
            "Preciso de pelo menos dois snapshots legíveis para comparar (há 1).\n\
             Aviso: não consegui ler o snapshot ruim: não é JSON\n"
        );
    }

    #[test]
    fn compare_com_dotfiles_iguais_mostra_quantos_em_comum() {
        use nodus_core::snapshot::DotfileState::{Linked, Missing};
        let a = com_dotfiles(
            snapshot_de("pc-a", "Arch Linux", &["base"], Some(&[])),
            &[(".bashrc", Linked), (".vimrc", Missing)],
        );
        let b = com_dotfiles(
            snapshot_de("pc-b", "Arch Linux", &["base"], Some(&[])),
            &[(".bashrc", Linked), (".vimrc", Missing)],
        );
        let esperado = "\
Comparando pc-a com pc-b (só leitura):
Pacman: 1 em comum
Flatpak: 0 em comum
Dotfiles: 2 em comum
Sem divergências.
";
        assert_eq!(
            render_comparison(&nodus_core::compare::compare(&a, &b)),
            esperado
        );
    }

    #[test]
    fn compare_com_dotfiles_diferentes_lista_cada_caminho_com_os_dois_estados() {
        use nodus_core::snapshot::DotfileState::{File, Linked, Missing, Other};
        let a = com_dotfiles(
            snapshot_de("pc-a", "Arch Linux", &["base"], Some(&[])),
            &[
                (".bashrc", Linked),
                (".gitconfig", File),
                (".vimrc", Other),
                (".zshrc", File),
            ],
        );
        let b = com_dotfiles(
            snapshot_de("pc-b", "Arch Linux", &["base"], Some(&[])),
            &[(".bashrc", File), (".gitconfig", File), (".vimrc", Missing)],
        );
        let esperado = "\
Comparando pc-a com pc-b (só leitura):
Pacman: 1 em comum
Flatpak: 0 em comum
Dotfiles: 1 em comum
  diferem (3):
    .bashrc: pc-a ligado, pc-b arquivo
    .vimrc: pc-a outro, pc-b ausente
    .zshrc: pc-a arquivo, pc-b fora da lista
";
        let texto = render_comparison(&nodus_core::compare::compare(&a, &b));
        assert_eq!(texto, esperado);
        assert!(!texto.contains("Sem divergências"), "{texto}");
    }

    #[test]
    fn compare_com_v1_de_um_lado_diz_que_dotfiles_nao_sao_comparaveis_e_como_resolver() {
        use nodus_core::snapshot::DotfileState::Linked;
        let antigo = snapshot_v1("antigo");
        let novo = com_dotfiles(
            snapshot_de("novo", "Arch Linux", &["base"], Some(&[])),
            &[(".bashrc", Linked)],
        );
        let texto = render_comparison(&nodus_core::compare::compare(&antigo, &novo));
        assert!(
            texto.contains("Dotfiles: não comparável, antigo não registrou dotfiles (snapshot v1; rode `nodusctl snapshot` de novo)\n"),
            "{texto}"
        );
        assert!(
            texto.ends_with("Sem divergências nas listas comparadas.\n"),
            "{texto}"
        );
    }

    #[test]
    fn compare_com_v1_dos_dois_lados_cita_os_dois() {
        let texto = render_comparison(&nodus_core::compare::compare(
            &snapshot_v1("pc-a"),
            &snapshot_v1("pc-b"),
        ));
        assert!(
            texto.contains("Dotfiles: não comparável, pc-a e pc-b não registraram dotfiles (snapshot v1; rode `nodusctl snapshot` de novo)\n"),
            "{texto}"
        );
    }

    #[test]
    fn compare_nao_escapa_caminho_de_dotfile_hostil() {
        // O caminho é validado na leitura; ainda assim, nada de byte de controle na saída.
        use nodus_core::snapshot::DotfileState::File;
        let mut a = com_dotfiles(
            snapshot_de("pc-a", "Arch Linux", &["base"], Some(&[])),
            &[(".bashrc", File)],
        );
        a.dotfiles[0].path = ".bashrc\x1b[31m".into();
        let b = snapshot_de("pc-b", "Arch Linux", &["base"], Some(&[]));
        let texto = render_comparison(&nodus_core::compare::compare(&a, &b));
        assert!(!texto.contains('\x1b'), "{texto:?}");
    }

    #[test]
    fn compare_mostra_o_que_so_tem_em_cada_lado() {
        let a = snapshot_de(
            "pc-a",
            "Arch Linux",
            &["base", "git", "vim"],
            Some(&["org.a.A"]),
        );
        let b = snapshot_de(
            "pc-b",
            "Arch Linux",
            &["base", "git", "htop"],
            Some(&["org.a.A"]),
        );
        let esperado = "\
Comparando pc-a com pc-b (só leitura):
Pacman: 2 em comum
  só em pc-a (1):
    vim
  só em pc-b (1):
    htop
Flatpak: 1 em comum
";
        assert_eq!(
            render_comparison(&nodus_core::compare::compare(&a, &b)),
            esperado
        );
    }

    #[test]
    fn compare_de_maquinas_iguais_diz_que_nao_ha_divergencia() {
        let a = snapshot_de("pc-a", "Arch Linux", &["base", "git"], Some(&["org.a.A"]));
        let b = snapshot_de("pc-b", "Arch Linux", &["git", "base"], Some(&["org.a.A"]));
        let esperado = "\
Comparando pc-a com pc-b (só leitura):
Pacman: 2 em comum
Flatpak: 1 em comum
Sem divergências.
";
        assert_eq!(
            render_comparison(&nodus_core::compare::compare(&a, &b)),
            esperado
        );
    }

    #[test]
    fn compare_sem_flatpak_num_lado_nao_acusa_falta_e_nao_diz_que_esta_tudo_igual() {
        let a = snapshot_de("pc-a", "Arch Linux", &["base"], Some(&["org.a.A"]));
        let b = snapshot_de("pc-b", "Arch Linux", &["base"], None);
        let texto = render_comparison(&nodus_core::compare::compare(&a, &b));
        let esperado = "\
Comparando pc-a com pc-b (só leitura):
Pacman: 1 em comum
Flatpak: não comparável, pc-b não tem Flatpak
Sem divergências nas listas comparadas.
";
        assert_eq!(texto, esperado);
        assert!(!texto.contains("só em"));
    }

    #[test]
    fn compare_com_as_duas_sem_flatpak_cita_as_duas() {
        let a = snapshot_de("pc-a", "Arch Linux", &["base"], None);
        let b = snapshot_de("pc-b", "Arch Linux", &["base"], None);
        let texto = render_comparison(&nodus_core::compare::compare(&a, &b));
        assert!(
            texto.contains("Flatpak: não comparável, pc-a e pc-b não têm Flatpak\n"),
            "{texto}"
        );
    }

    #[test]
    fn compare_avisa_quando_o_so_e_diferente() {
        let a = snapshot_de("pc-a", "Arch Linux", &["base"], None);
        let mut b = snapshot_de("pc-b", "Fedora Linux", &["base"], None);
        b.os.version = Some("41".into());
        let texto = render_comparison(&nodus_core::compare::compare(&a, &b));
        let aviso = "Aviso: sistemas diferentes: pc-a é Arch Linux, pc-b é Fedora Linux 41\n";
        assert!(texto.contains(aviso), "{texto}");
        assert!(!texto.contains("Sem divergências"), "{texto}");
    }

    #[test]
    fn compare_nao_escapa_nome_hostil_do_arquivo_de_outra_maquina() {
        // Os nomes de pacote são validados na leitura; ainda assim, nada de byte de controle na saída.
        let a = snapshot_de("pc-a", "Arch Linux", &["base"], None);
        let mut b = snapshot_de("pc-b", "Arch Linux", &["base"], None);
        b.os.name = "Arch\x1b[31m Linux".into();
        let texto = render_comparison(&nodus_core::compare::compare(&a, &b));
        assert!(!texto.contains('\x1b'), "{texto:?}");
    }

    // --- preset status (critério 3 do SPEC §16) ---

    fn preset_de(pacman: &[&str], flatpak: &[&str]) -> nodus_core::preset::PresetFile {
        preset_com_tabelas(pacman, flatpak, "")
    }

    /// `tabelas` é o texto das `[machines.<rótulo>]` (vazio se não há).
    fn preset_com_tabelas(
        pacman: &[&str],
        flatpak: &[&str],
        tabelas: &str,
    ) -> nodus_core::preset::PresetFile {
        let texto = format!("version = 1\npacman = {pacman:?}\nflatpak = {flatpak:?}\n{tabelas}");
        nodus_core::preset::parse_preset_file(&texto).unwrap()
    }

    fn estado(
        preset: &nodus_core::preset::PresetFile,
        snapshot: &nodus_core::snapshot::Snapshot,
    ) -> nodus_core::status::MachineStatus {
        nodus_core::status::status_for(preset, snapshot)
    }

    const NOTA_STATUS: &str = "Nota: \"não instalado de propósito\" inclui o que veio só como dependência; o snapshot guarda `pacman -Qe`.\n";

    #[test]
    fn status_mostra_o_que_falta_e_o_que_sobra_por_maquina() {
        let p = preset_de(&["base", "git", "htop"], &["org.a.A"]);
        let s = snapshot_de(
            "pc-a",
            "Arch Linux",
            &["base", "git", "vim"],
            Some(&["org.a.A"]),
        );
        let esperado = format!(
            "\
Preset: pacman 3, flatpak 1 (só leitura, a partir dos snapshots):
pc-a (Arch Linux):
  Pacman: 2 em dia
    no preset e não instalado de propósito (1):
      htop
    instalado de propósito e fora do preset (1):
      vim
  Flatpak: 1 em dia
  Para registrar como exceção desta máquina, cole no preset (nada foi alterado):
    [machines.pc-a]
    add_pacman = [\"vim\"]
    skip_pacman = [\"htop\"]
{NOTA_STATUS}"
        );
        assert_eq!(render_status(&p, &[estado(&p, &s)], &[]), esperado);
    }

    #[test]
    fn status_de_maquina_em_dia_nao_tem_listas_nem_nota() {
        let p = preset_de(&["base", "git"], &["org.a.A"]);
        let s = snapshot_de("pc-a", "Arch Linux", &["git", "base"], Some(&["org.a.A"]));
        let esperado = "\
Preset: pacman 2, flatpak 1 (só leitura, a partir dos snapshots):
pc-a (Arch Linux): em dia com o preset
  Pacman: 2 em dia
  Flatpak: 1 em dia
";
        assert_eq!(render_status(&p, &[estado(&p, &s)], &[]), esperado);
    }

    #[test]
    fn status_sem_flatpak_nao_acusa_falta_e_nao_diz_que_esta_tudo_em_dia() {
        let p = preset_de(&["base"], &["org.a.A"]);
        let s = snapshot_de("pc-b", "Arch Linux", &["base"], None);
        let esperado = "\
Preset: pacman 1, flatpak 1 (só leitura, a partir dos snapshots):
pc-b (Arch Linux): em dia nas listas comparadas
  Pacman: 1 em dia
  Flatpak: não comparável, pc-b não tem Flatpak
";
        assert_eq!(render_status(&p, &[estado(&p, &s)], &[]), esperado);
    }

    #[test]
    fn status_mostra_varias_maquinas_na_ordem_recebida() {
        let p = preset_de(&["base"], &[]);
        let a = snapshot_de("pc-a", "Arch Linux", &["base"], Some(&[]));
        let b = snapshot_de("pc-b", "Arch Linux", &[], Some(&[]));
        let texto = render_status(&p, &[estado(&p, &a), estado(&p, &b)], &[]);
        let pos_a = texto.find("pc-a (").unwrap();
        let pos_b = texto.find("pc-b (").unwrap();
        assert!(pos_a < pos_b, "{texto}");
        assert!(texto.contains("pc-a (Arch Linux): em dia com o preset\n"));
        assert!(texto.contains("pc-b (Arch Linux):\n"));
    }

    #[test]
    fn status_avisa_do_snapshot_ilegivel_e_continua_com_os_outros() {
        let p = preset_de(&["base"], &[]);
        let a = snapshot_de("pc-a", "Arch Linux", &["base"], Some(&[]));
        let ruim = crate::snapshot::Unreadable {
            label: "pc-x".into(),
            reason: "versão 2 do snapshot não é suportada".into(),
        };
        let texto = render_status(&p, &[estado(&p, &a)], &[ruim]);
        assert!(
            texto.contains("pc-a (Arch Linux): em dia com o preset\n"),
            "{texto}"
        );
        let aviso =
            "Aviso: não consegui ler o snapshot pc-x: versão 2 do snapshot não é suportada\n";
        assert!(texto.contains(aviso), "{texto}");
    }

    #[test]
    fn status_sem_nenhum_snapshot_diz_como_gerar() {
        let p = preset_de(&["base"], &[]);
        let esperado = "Nenhum snapshot encontrado; rode `nodusctl snapshot` em cada máquina e traga os arquivos pelo git.\n";
        assert_eq!(render_status(&p, &[], &[]), esperado);
    }

    #[test]
    fn status_nao_escapa_byte_de_controle_do_so_nem_do_motivo() {
        let p = preset_de(&["base"], &[]);
        let mut s = snapshot_de("pc-a", "Arch Linux", &["base"], Some(&[]));
        s.os.name = "Arch\x1b[31m Linux".into();
        let ruim = crate::snapshot::Unreadable {
            label: "pc-x".into(),
            reason: "erro\x1b[2J".into(),
        };
        let texto = render_status(&p, &[estado(&p, &s)], &[ruim]);
        assert!(!texto.contains('\x1b'), "{texto:?}");
    }

    // Fatia 8: o trecho `[machines.<rótulo>]` com a exceção sugerida.

    #[test]
    fn status_sugere_add_numa_tabela_nova_para_o_que_sobra() {
        let p = preset_de(&["git", "vim"], &["org.A.A"]);
        let s = snapshot_de(
            "notebook",
            "Arch Linux",
            &["git", "vim", "steam"],
            Some(&["org.A.A"]),
        );
        let esperado = "\
Preset: pacman 2, flatpak 1 (só leitura, a partir dos snapshots):
notebook (Arch Linux):
  Pacman: 2 em dia
    instalado de propósito e fora do preset (1):
      steam
  Flatpak: 1 em dia
  Para registrar como exceção desta máquina, cole no preset (nada foi alterado):
    [machines.notebook]
    add_pacman = [\"steam\"]
";
        assert_eq!(render_status(&p, &[estado(&p, &s)], &[]), esperado);
    }

    #[test]
    fn status_sugere_skip_para_o_que_falta_e_mantem_a_nota_da_dependencia() {
        let p = preset_de(&["git", "vim"], &[]);
        let s = snapshot_de("notebook", "Arch Linux", &["git"], Some(&[]));
        let esperado = format!(
            "\
Preset: pacman 2, flatpak 0 (só leitura, a partir dos snapshots):
notebook (Arch Linux):
  Pacman: 1 em dia
    no preset e não instalado de propósito (1):
      vim
  Flatpak: 0 em dia
  Para registrar como exceção desta máquina, cole no preset (nada foi alterado):
    [machines.notebook]
    skip_pacman = [\"vim\"]
{NOTA_STATUS}"
        );
        assert_eq!(render_status(&p, &[estado(&p, &s)], &[]), esperado);
    }

    #[test]
    fn status_com_pacman_e_flatpak_poe_as_quatro_chaves_na_ordem_add_depois_skip() {
        let p = preset_de(&["git", "vim"], &["org.A.A"]);
        let s = snapshot_de(
            "notebook",
            "Arch Linux",
            &["git", "steam"],
            Some(&["org.B.B"]),
        );
        let texto = render_status(&p, &[estado(&p, &s)], &[]);
        let esperado = "    [machines.notebook]
    add_pacman = [\"steam\"]
    add_flatpak = [\"org.B.B\"]
    skip_pacman = [\"vim\"]
    skip_flatpak = [\"org.A.A\"]
";
        assert!(texto.contains(esperado), "{texto}");
    }

    #[test]
    fn status_com_a_tabela_ja_existente_manda_acrescentar_na_lista_sem_repetir_a_tabela() {
        let p = preset_com_tabelas(
            &["git", "vim"],
            &[],
            "\n[machines.notebook]\nskip_pacman = [\"vim\"]\n",
        );
        let s = snapshot_de("notebook", "Arch Linux", &["git", "steam"], Some(&[]));
        let texto = render_status(&p, &[estado(&p, &s)], &[]);
        let esperado = "  A tabela [machines.notebook] já existe. Para registrar a exceção, acrescente (nada foi alterado):
    em `add_pacman = [...]`:
      \"steam\",
";
        assert!(texto.contains(esperado), "{texto}");
        assert!(!texto.contains("\n    [machines.notebook]\n"), "{texto}");
    }

    #[test]
    fn status_nao_sugere_nada_para_pulo_deliberado_nem_para_add_proprio() {
        let p = preset_com_tabelas(
            &["git", "vim"],
            &[],
            "\n[machines.notebook]\nskip_pacman = [\"vim\"]\nadd_pacman = [\"steam\"]\n",
        );
        // `vim` pulado e instalado (sobra), `steam` acrescentado e não instalado (falta).
        let s = snapshot_de("notebook", "Arch Linux", &["git", "vim"], Some(&[]));
        let texto = render_status(&p, &[estado(&p, &s)], &[]);
        assert!(texto.contains("      vim\n"), "{texto}");
        assert!(texto.contains("      steam\n"), "{texto}");
        assert!(!texto.contains("Para registrar"), "{texto}");
        assert!(!texto.contains("já existe"), "{texto}");
    }

    #[test]
    fn status_de_maquina_em_dia_ou_sem_flatpak_nao_tem_trecho() {
        let p = preset_de(&["base"], &["org.a.A"]);
        let em_dia = snapshot_de("pc-a", "Arch Linux", &["base"], Some(&["org.a.A"]));
        let sem_flatpak = snapshot_de("pc-b", "Arch Linux", &["base"], None);
        let texto = render_status(&p, &[estado(&p, &em_dia), estado(&p, &sem_flatpak)], &[]);
        assert!(!texto.contains("Para registrar"), "{texto}");
        assert!(!texto.contains("[machines."), "{texto}");
    }

    #[test]
    fn status_poe_um_trecho_por_maquina_dentro_do_bloco_dela() {
        let p = preset_de(&["git"], &[]);
        let a = snapshot_de("pc-a", "Arch Linux", &["git", "steam"], Some(&[]));
        let b = snapshot_de("pc-b", "Arch Linux", &["git", "htop"], Some(&[]));
        let texto = render_status(&p, &[estado(&p, &a), estado(&p, &b)], &[]);
        let (pos_a, pos_b) = (texto.find("pc-a (").unwrap(), texto.find("pc-b (").unwrap());
        let trecho_a = texto.find("[machines.pc-a]").unwrap();
        let trecho_b = texto.find("[machines.pc-b]").unwrap();
        assert!(pos_a < trecho_a && trecho_a < pos_b, "{texto}");
        assert!(pos_b < trecho_b, "{texto}");
        assert!(texto.contains("add_pacman = [\"steam\"]"), "{texto}");
        assert!(texto.contains("add_pacman = [\"htop\"]"), "{texto}");
    }

    #[test]
    fn status_nao_poe_nome_invalido_no_trecho() {
        // O trecho tem só nomes que o parse do preset aceita; a linha é exatamente esta.
        let p = preset_de(&["git"], &[]);
        let mut s = snapshot_de("notebook", "Arch Linux", &["git"], Some(&[]));
        s.pacman.push("a\x1b[31mb".into());
        s.pacman.push("--overwrite=*".into());
        s.pacman.push("steam".into());
        let texto = render_status(&p, &[estado(&p, &s)], &[]);
        assert!(
            texto.contains("\n    add_pacman = [\"steam\"]\n"),
            "{texto}"
        );
    }

    // --- limite de 20 nomes por lista no trecho (ADR 0033, emenda 5) ---

    /// `pkg01`, `pkg02`, ... (ordem alfabética igual à numérica).
    fn pkgs(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("pkg{i:02}")).collect()
    }

    /// Texto do `status` de uma máquina `notebook` com `n` pacotes instalados fora do preset
    /// (`git` é o único do preset), e as `tabelas` dadas.
    fn texto_com_extras(n: usize, tabelas: &str) -> String {
        let p = preset_com_tabelas(&["git"], &[], tabelas);
        let mut instalados = vec!["git".to_string()];
        instalados.extend(pkgs(n));
        let refs: Vec<&str> = instalados.iter().map(String::as_str).collect();
        let s = snapshot_de("notebook", "Arch Linux", &refs, Some(&[]));
        render_status(&p, &[estado(&p, &s)], &[])
    }

    fn linha_add_pacman_ate(n: usize) -> String {
        let nomes: Vec<String> = pkgs(n).iter().map(|nome| format!("\"{nome}\"")).collect();
        format!("    add_pacman = [{}]", nomes.join(", "))
    }

    #[test]
    fn trecho_sem_tabela_corta_em_20_e_fecha_com_comentario_toml() {
        let texto = texto_com_extras(25, "");
        let esperado = format!(
            "{}  # +5 omitidos (lista completa acima)\n",
            linha_add_pacman_ate(20)
        );
        assert!(texto.contains(&esperado), "{texto}");
        assert!(!texto.contains("\"pkg21\""), "{texto}");
    }

    #[test]
    fn trecho_com_tabela_existente_corta_em_20_e_poe_o_comentario_depois() {
        let texto = texto_com_extras(25, "\n[machines.notebook]\nskip_pacman = []\n");
        let esperado = "      \"pkg20\",\n      # ... +5 omitidos (lista completa acima)\n";
        assert!(texto.contains(esperado), "{texto}");
        assert!(!texto.contains("\"pkg21\""), "{texto}");
    }

    #[test]
    fn trecho_com_exatamente_20_nao_tem_comentario() {
        let texto = texto_com_extras(20, "");
        assert!(
            texto.contains(&format!("{}\n", linha_add_pacman_ate(20))),
            "{texto}"
        );
        assert!(!texto.contains("omitidos"), "{texto}");
    }

    #[test]
    fn trecho_com_21_omite_um() {
        let texto = texto_com_extras(21, "");
        let esperado = format!(
            "{}  # +1 omitidos (lista completa acima)\n",
            linha_add_pacman_ate(20)
        );
        assert!(texto.contains(&esperado), "{texto}");
    }

    #[test]
    fn trecho_corta_so_a_lista_grande_e_deixa_a_pequena_inteira() {
        // 25 de fora do preset (add) e 3 do preset que faltam (skip).
        let p = preset_de(&["a1", "a2", "a3"], &[]);
        let instalados = pkgs(25);
        let refs: Vec<&str> = instalados.iter().map(String::as_str).collect();
        let s = snapshot_de("notebook", "Arch Linux", &refs, Some(&[]));
        let texto = render_status(&p, &[estado(&p, &s)], &[]);
        assert!(texto.contains("+5 omitidos"), "{texto}");
        assert!(
            texto.contains("\n    skip_pacman = [\"a1\", \"a2\", \"a3\"]\n"),
            "{texto}"
        );
    }

    #[test]
    fn listas_acima_do_trecho_continuam_completas_com_25() {
        let texto = texto_com_extras(25, "");
        for nome in pkgs(25) {
            assert!(
                texto.contains(&format!("\n      {nome}\n")),
                "{nome}: {texto}"
            );
        }
    }

    #[test]
    fn json_continua_com_os_25_nomes() {
        let p = preset_de(&["git"], &[]);
        let mut instalados = vec!["git".to_string()];
        instalados.extend(pkgs(25));
        let refs: Vec<&str> = instalados.iter().map(String::as_str).collect();
        let s = snapshot_de("notebook", "Arch Linux", &refs, Some(&[]));
        let json = status_json(&p, &[estado(&p, &s)], &[]).unwrap();
        assert_eq!(
            json["machines"][0]["exceptions"]["add"]["pacman"]
                .as_array()
                .unwrap()
                .len(),
            25
        );
    }

    // --- `preset status --json` com `exceptions` (ADR 0033, emenda 4) ---

    use serde_json::{Value, json};

    fn json_de(file: &PresetFile, snapshots: &[nodus_core::snapshot::Snapshot]) -> Value {
        let estados: Vec<_> = snapshots.iter().map(|s| estado(file, s)).collect();
        status_json(file, &estados, &[]).unwrap()
    }

    #[test]
    fn json_sem_tabela_poe_o_que_falta_em_skip_e_table_exists_falso() {
        let p = preset_de(&["git", "vim"], &[]);
        let s = snapshot_de("pc-a", "Arch Linux", &["vim"], Some(&[]));
        let json = json_de(&p, &[s]);
        assert_eq!(
            json["machines"][0]["exceptions"],
            json!({
                "table_exists": false,
                "add": {"pacman": [], "flatpak": []},
                "skip": {"pacman": ["git"], "flatpak": []}
            })
        );
    }

    #[test]
    fn json_poe_o_que_sobra_em_add() {
        let p = preset_de(&["git"], &[]);
        let s = snapshot_de("pc-a", "Arch Linux", &["git", "sl"], Some(&[]));
        let json = json_de(&p, &[s]);
        assert_eq!(
            json["machines"][0]["exceptions"]["add"]["pacman"],
            json!(["sl"])
        );
    }

    #[test]
    fn json_diz_que_a_tabela_da_maquina_ja_existe() {
        let p = preset_com_tabelas(
            &["git", "vim"],
            &[],
            "\n[machines.notebook]\nskip_pacman = [\"vim\"]\n",
        );
        let s = snapshot_de("notebook", "Arch Linux", &["git", "steam"], Some(&[]));
        let outra = snapshot_de("pc-b", "Arch Linux", &["git", "steam"], Some(&[]));
        let json = json_de(&p, &[s, outra]);
        assert_eq!(
            json["machines"][0]["exceptions"]["table_exists"],
            json!(true)
        );
        assert_eq!(
            json["machines"][0]["exceptions"]["add"]["pacman"],
            json!(["steam"])
        );
        assert_eq!(
            json["machines"][1]["exceptions"]["table_exists"],
            json!(false)
        );
    }

    #[test]
    fn json_de_maquina_em_dia_ou_sem_flatpak_traz_a_chave_com_listas_vazias() {
        let p = preset_de(&["base"], &["org.a.A"]);
        let em_dia = snapshot_de("pc-a", "Arch Linux", &["base"], Some(&["org.a.A"]));
        let sem_flatpak = snapshot_de("pc-b", "Arch Linux", &["base"], None);
        let json = json_de(&p, &[em_dia, sem_flatpak]);
        let vazio = json!({
            "table_exists": false,
            "add": {"pacman": [], "flatpak": []},
            "skip": {"pacman": [], "flatpak": []}
        });
        assert_eq!(json["machines"][0]["exceptions"], vazio);
        assert_eq!(json["machines"][1]["exceptions"], vazio);
    }

    #[test]
    fn json_nao_poe_nome_invalido_nem_pulo_deliberado_nas_excecoes() {
        let p = preset_com_tabelas(
            &["git", "vim"],
            &[],
            "\n[machines.notebook]\nskip_pacman = [\"vim\"]\n",
        );
        // `vim` pulado e instalado (pulo deliberado) e nomes que o parse recusaria.
        let mut s = snapshot_de("notebook", "Arch Linux", &["git", "vim"], Some(&[]));
        s.pacman.push("--overwrite=*".into());
        s.pacman.push("steam".into());
        let json = json_de(&p, &[s]);
        assert_eq!(
            json["machines"][0]["exceptions"]["add"]["pacman"],
            json!(["steam"])
        );
        assert_eq!(
            json["machines"][0]["exceptions"]["skip"]["pacman"],
            json!([])
        );
    }

    #[test]
    fn json_mantem_o_resto_do_formato_e_os_ilegiveis() {
        let p = preset_de(&["git", "vim"], &[]);
        let s = snapshot_de("pc-a", "Arch Linux", &["vim"], Some(&[]));
        let estados = vec![estado(&p, &s)];
        let ilegiveis = vec![Unreadable {
            label: "quebrado".into(),
            reason: "JSON inválido".into(),
        }];
        let json = status_json(&p, &estados, &ilegiveis).unwrap();
        let mut maquina = json["machines"][0].clone();
        maquina.as_object_mut().unwrap().remove("exceptions");
        assert_eq!(
            maquina,
            serde_json::to_value(&estados[0]).unwrap(),
            "fora de `exceptions`, a máquina é o `MachineStatus` de antes"
        );
        assert_eq!(
            json["unreadable"],
            json!([{"label": "quebrado", "reason": "JSON inválido"}])
        );
        assert_eq!(
            json.as_object().unwrap().len(),
            2,
            "só `machines` e `unreadable`"
        );
    }
}
