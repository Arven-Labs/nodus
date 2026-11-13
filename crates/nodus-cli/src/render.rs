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
    let _ = writeln!(out, "Pacotes ({}):", inventory.packages.len());
    for package in &inventory.packages {
        let _ = writeln!(out, "  {} {}", package.name, package.version);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodus_core::inventory::{Inventory, Package, SystemInfo};

    fn inventario(os_version: Option<&str>) -> Inventory {
        Inventory {
            system: SystemInfo {
                os_name: "Arch Linux".into(),
                os_version: os_version.map(String::from),
                kernel: "6.12.1-arch1-1".into(),
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
        }
    }

    #[test]
    fn texto_sem_versao_do_sistema() {
        let esperado = "Sistema: Arch Linux\nKernel: 6.12.1-arch1-1\nPacotes (2):\n  bash 5.2.037-2\n  linux 6.12.1.arch1-1\n";
        assert_eq!(render_text(&inventario(None)), esperado);
    }

    #[test]
    fn texto_com_versao_do_sistema() {
        assert!(render_text(&inventario(Some("41"))).starts_with("Sistema: Arch Linux 41\n"));
    }

    #[test]
    fn texto_sem_pacotes() {
        let mut inv = inventario(None);
        inv.packages.clear();
        assert!(render_text(&inv).ends_with("Pacotes (0):\n"));
    }
}
