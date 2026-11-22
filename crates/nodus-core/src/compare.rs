//! Comparação de dois snapshots (ADR 0031, fatia 2). Puro: quem lê os arquivos é o CLI.
//!
//! Uma lista só é comparada se **as duas** máquinas têm a capacidade correspondente. Uma máquina
//! sem Flatpak não "perdeu" os apps: dizer "faltam apps" seria acusar falta onde não há como ter.
//! A capacidade manda, mesmo que um arquivo editado à mão traga a lista.

use crate::snapshot::{Snapshot, SnapshotOs};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comparison {
    /// Rótulos das duas máquinas, na ordem dos argumentos.
    pub a: String,
    pub b: String,
    /// Só quando o SO (nome ou versão) é diferente. Informativo: as listas continuam comparadas.
    pub os: Option<OsDifference>,
    pub pacman: ListComparison,
    pub flatpak: ListComparison,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OsDifference {
    pub a: SnapshotOs,
    pub b: SnapshotOs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ListComparison {
    Compared {
        /// Itens que só a máquina `a` tem, ordenados.
        only_a: Vec<String>,
        /// Itens que só a máquina `b` tem, ordenados.
        only_b: Vec<String>,
        common: usize,
    },
    /// Alguma máquina não tem a capacidade; `lacks` traz o rótulo de quem não tem.
    NotComparable { lacks: Vec<String> },
}

impl Comparison {
    /// Há algo diferente: SO, ou item só de um lado numa lista comparada. "Não comparável" não conta.
    pub fn has_differences(&self) -> bool {
        has_items_only_on_one_side(&self.pacman) || has_items_only_on_one_side(&self.flatpak)
    }

    pub fn has_not_comparable(&self) -> bool {
        matches!(self.pacman, ListComparison::NotComparable { .. })
            || matches!(self.flatpak, ListComparison::NotComparable { .. })
    }
}

fn has_items_only_on_one_side(list: &ListComparison) -> bool {
    match list {
        ListComparison::Compared { only_a, only_b, .. } => !only_a.is_empty() || !only_b.is_empty(),
        ListComparison::NotComparable { .. } => false,
    }
}

pub fn compare(a: &Snapshot, b: &Snapshot) -> Comparison {
    Comparison {
        a: a.name.clone(),
        b: b.name.clone(),
        os: (a.os != b.os).then(|| OsDifference {
            a: a.os.clone(),
            b: b.os.clone(),
        }),
        pacman: compare_list(
            a,
            b,
            (a.capabilities.pacman, b.capabilities.pacman),
            (&a.pacman, &b.pacman),
        ),
        flatpak: compare_list(a, b, (true, true), (&a.flatpak, &b.flatpak)),
    }
}

fn compare_list(
    a: &Snapshot,
    b: &Snapshot,
    has: (bool, bool),
    items: (&[String], &[String]),
) -> ListComparison {
    let mut lacks = Vec::new();
    if !has.0 {
        lacks.push(a.name.clone());
    }
    if !has.1 {
        lacks.push(b.name.clone());
    }
    if !lacks.is_empty() {
        return ListComparison::NotComparable { lacks };
    }
    let set_a: BTreeSet<&String> = items.0.iter().collect();
    let set_b: BTreeSet<&String> = items.1.iter().collect();
    ListComparison::Compared {
        only_a: set_b.difference(&set_a).map(|s| (*s).clone()).collect(),
        only_b: set_b.difference(&set_a).map(|s| (*s).clone()).collect(),
        common: set_a.intersection(&set_b).count(),
    }
}

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
