#[cfg(test)]
mod tests {
    //! Comparar dois snapshots (ADR 0031, fatia 2). Puro: sem arquivo, sem processo. Uma lista só é
    //! comparada se as duas máquinas têm a capacidade; "não comparável" não é "diferente".
    use super::*;
    use crate::inventory::OsInfo;
    use crate::preset::{MachinePreset, Preset};
    use crate::snapshot::{Snapshot, build};

    fn os(name: &str) -> OsInfo {
        OsInfo {
            name: name.into(),
            version: None,
        }
    }

    fn snap(label: &str, pacman: &[&str], flatpak: Option<&[&str]>) -> Snapshot {
        let machine = MachinePreset {
            preset: Preset {
                pacman: pacman.iter().map(|s| s.to_string()).collect(),
                flatpak: flatpak
                    .unwrap_or(&[])
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            },
            flatpak_installed: flatpak.is_some(),
        };
        build(label, &os("Arch Linux"), &machine).unwrap()
    }

    fn compared(c: &ListComparison) -> (&[String], &[String], usize) {
        match c {
            ListComparison::Compared {
                only_a,
                only_b,
                common,
            } => (only_a, only_b, *common),
            outro => panic!("devia ser comparável: {outro:?}"),
        }
    }

    #[test]
    fn separa_o_que_so_tem_em_cada_maquina_e_conta_o_comum() {
        let a = snap("pc-a", &["base", "git", "vim"], Some(&["org.a.A"]));
        let b = snap(
            "pc-b",
            &["base", "git", "htop"],
            Some(&["org.a.A", "org.b.B"]),
        );
        let c = compare(&a, &b);
        assert_eq!((c.a.as_str(), c.b.as_str()), ("pc-a", "pc-b"));
        let (so_a, so_b, comum) = compared(&c.pacman);
        assert_eq!(
            (so_a, so_b, comum),
            (&["vim".to_string()][..], &["htop".to_string()][..], 2)
        );
        let (so_a, so_b, comum) = compared(&c.flatpak);
        assert!(so_a.is_empty());
        assert_eq!(so_b, ["org.b.B"]);
        assert_eq!(comum, 1);
    }

    #[test]
    fn as_listas_de_diferencas_saem_ordenadas() {
        let a = snap("pc-a", &["zsh", "vim", "awk"], None);
        let b = snap("pc-b", &[], None);
        let c = compare(&a, &b);
        assert_eq!(compared(&c.pacman).0, ["awk", "vim", "zsh"]);
    }

    #[test]
    fn trocar_a_ordem_dos_argumentos_troca_os_lados() {
        let a = snap("pc-a", &["git", "vim"], Some(&["org.a.A"]));
        let b = snap("pc-b", &["git", "htop"], Some(&[]));
        let ab = compare(&a, &b);
        let ba = compare(&b, &a);
        assert_eq!(compared(&ab.pacman).0, compared(&ba.pacman).1);
        assert_eq!(compared(&ab.pacman).1, compared(&ba.pacman).0);
        assert_eq!(compared(&ab.pacman).2, compared(&ba.pacman).2);
        assert_eq!(compared(&ab.flatpak).0, compared(&ba.flatpak).1);
    }

    #[test]
    fn maquinas_iguais_nao_tem_divergencia_e_comparar_com_si_mesma_tambem() {
        let a = snap("pc-a", &["base", "git"], Some(&["org.a.A"]));
        let b = snap("pc-b", &["git", "base"], Some(&["org.a.A"]));
        assert!(!compare(&a, &b).has_differences());
        assert!(!compare(&a, &a).has_differences());
    }

    #[test]
    fn qualquer_item_so_de_um_lado_ou_so_diferente_conta_como_divergencia() {
        let a = snap("pc-a", &["base"], Some(&[]));
        let so_pacman = snap("pc-b", &["base", "vim"], Some(&[]));
        let so_flatpak = snap("pc-b", &["base"], Some(&["org.a.A"]));
        assert!(compare(&a, &so_pacman).has_differences());
        assert!(compare(&a, &so_flatpak).has_differences());
    }

    #[test]
    fn sem_flatpak_num_lado_a_lista_nao_e_comparavel_e_nao_acusa_falta() {
        let a = snap("pc-a", &["base"], Some(&["org.a.A"]));
        let b = snap("pc-b", &["base"], None);
        let c = compare(&a, &b);
        assert_eq!(
            c.flatpak,
            ListComparison::NotComparable {
                lacks: vec!["pc-b".into()]
            }
        );
        // Sem Flatpak não há como "faltar" app: só o pacman é comparado e está igual.
        assert!(!c.has_differences());
        assert!(c.has_not_comparable());
    }

    #[test]
    fn a_capacidade_manda_mesmo_que_o_arquivo_traga_apps_sem_flatpak() {
        // Arquivo editado à mão: sem a capacidade, mas com uma lista. A lista não vale.
        let a = snap("pc-a", &["base"], Some(&["org.a.A"]));
        let mut b = snap("pc-b", &["base"], None);
        b.flatpak = vec!["org.x.X".into()];
        let c = compare(&a, &b);
        assert!(matches!(c.flatpak, ListComparison::NotComparable { .. }));
    }

    #[test]
    fn as_duas_sem_flatpak_citam_as_duas() {
        let a = snap("pc-a", &["base"], None);
        let b = snap("pc-b", &["base"], None);
        let c = compare(&a, &b);
        assert_eq!(
            c.flatpak,
            ListComparison::NotComparable {
                lacks: vec!["pc-a".into(), "pc-b".into()]
            }
        );
    }

    #[test]
    fn sem_pacman_num_lado_o_pacman_tambem_nao_e_comparavel() {
        let a = snap("pc-a", &["base"], None);
        let mut b = snap("pc-b", &[], None);
        b.capabilities.pacman = false;
        let c = compare(&a, &b);
        assert_eq!(
            c.pacman,
            ListComparison::NotComparable {
                lacks: vec!["pc-b".into()]
            }
        );
    }

    #[test]
    fn so_diferente_aparece_como_aviso_e_a_lista_continua_comparada() {
        let a = snap("pc-a", &["base"], None);
        let mut b = snap("pc-b", &["base", "vim"], None);
        assert!(compare(&a, &b).os.is_none());
        b.os.name = "Fedora Linux".into();
        b.os.version = Some("41".into());
        let c = compare(&a, &b);
        let dif = c.os.as_ref().expect("SO diferente");
        assert_eq!(dif.a.name, "Arch Linux");
        assert_eq!(dif.b.name, "Fedora Linux");
        assert_eq!(compared(&c.pacman).1, ["vim"]);
        assert!(c.has_differences());
    }

    #[test]
    fn so_diferente_sozinho_conta_como_divergencia() {
        let a = snap("pc-a", &["base"], None);
        let mut b = snap("pc-b", &["base"], None);
        b.os.name = "Fedora Linux".into();
        assert!(compare(&a, &b).has_differences());
    }

    #[test]
    fn o_json_tem_este_formato() {
        let a = snap("pc-a", &["base", "vim"], Some(&["org.a.A"]));
        let b = snap("pc-b", &["base", "git"], None);
        let json = serde_json::to_value(compare(&a, &b)).unwrap();
        let esperado = serde_json::json!({
            "a": "pc-a",
            "b": "pc-b",
            "os": null,
            "pacman": {
                "status": "compared",
                "only_a": ["vim"],
                "only_b": ["git"],
                "common": 1
            },
            "flatpak": {"status": "not_comparable", "lacks": ["pc-b"]}
        });
        assert_eq!(json, esperado);
    }
}
