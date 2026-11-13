//! Formatação do inventário para o terminal.

use nodus_core::inventory::Inventory;
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
