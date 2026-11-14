//! Formatação do inventário para o terminal.

use nodus_core::history::{Action, ExecResult, HistoryEntry, format_utc};
use nodus_core::inventory::Inventory;
use nodus_core::preset::Plan;
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
            _ => {
                let _ = writeln!(
                    out,
                    "[{n}] {when}  {result}  pedidos {requested}, instalados {installed}, faltando {missing}"
                );
                if !entry.installed.is_empty() {
                    let _ = writeln!(out, "  instalados: {}", entry.installed.join(", "));
                }
                if !entry.still_missing.is_empty() {
                    let _ = writeln!(out, "  faltando: {}", entry.still_missing.join(", "));
                }
            }
        }
        out.push_str(HISTORY_NOTE);
    }
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
        out.push_str("Nada a fazer: a máquina já está como o preset.\n");
    }
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
    use nodus_core::inventory::{Disk, FlatpakApp, HardwareInfo, Inventory, Package, SystemInfo};
    use nodus_core::preset::Plan;
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
    fn plano_texto_sem_flatpak_instalado() {
        let mut p = plano(&[], &["org.a.B"], false);
        p.flatpak_present = 0;
        assert!(
            render_plan(&p).ends_with("Flatpak não instalado (apps do preset: 1):\n  org.a.B\n")
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

    use nodus_core::history::{Action, ExecResult, HistoryEntry};

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
            version: 2,
            action: Action::Install,
            undoes: None,
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
}
