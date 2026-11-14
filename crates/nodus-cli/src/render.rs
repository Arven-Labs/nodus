//! Formatação do inventário para o terminal.

use nodus_core::inventory::Inventory;
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
        if let Some(FetchResult::Failed(motivo)) = &repo.fetch {
            let _ = write!(out, " [fetch falhou: {motivo}]");
        }
        let _ = writeln!(out);
    }
    out
}

/// Bytes em GiB (base 1024) com uma casa decimal.
fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1u64 << 30) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodus_core::inventory::{Disk, FlatpakApp, HardwareInfo, Inventory, Package, SystemInfo};
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
}
