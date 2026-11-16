//! Formatação do inventário para o terminal.

use crate::workspace::{Report, Source};
use nodus_core::dotfiles::Entry;
use nodus_core::history::{Action, ExecResult, HistoryEntry, Manager, format_utc};
use nodus_core::inventory::Inventory;
use nodus_core::preset::{Plan, Suggestions};
use nodus_core::repos::{FetchResult, RepoStatus};
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
                let _ = writeln!(
                    out,
                    "[{n}] {when}  {result}  desfez [{undoes}]  dotfile restaurado: {}  (pasta-fonte {dir})",
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
}
