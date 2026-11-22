//! Comparação de dois snapshots (ADR 0031, fatia 2). Puro: quem lê os arquivos é o CLI.
//!
//! Uma lista só é comparada se **as duas** máquinas têm a capacidade correspondente. Uma máquina
//! sem Flatpak não "perdeu" os apps: dizer "faltam apps" seria acusar falta onde não há como ter.
//! A capacidade manda, mesmo que um arquivo editado à mão traga a lista.

use crate::snapshot::{DotfileEntry, DotfileState, Snapshot, SnapshotOs};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comparison {
    /// Rótulos das duas máquinas, na ordem dos argumentos.
    pub a: String,
    pub b: String,
    /// Só quando o SO (nome ou versão) é diferente. Informativo: as listas continuam comparadas.
    pub os: Option<OsDifference>,
    pub pacman: ListComparison,
    pub flatpak: ListComparison,
    pub dotfiles: DotfilesComparison,
}

/// Dotfiles: só se compara se as duas máquinas olharam (snapshot v2). Um snapshot v1 não olhou, e
/// dizer que "não tem" o `.bashrc` seria acusar falta onde ninguém procurou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DotfilesComparison {
    Compared {
        /// Caminhos com estado diferente, ou só na lista de uma das máquinas, ordenados.
        differ: Vec<DotfileDiff>,
        /// Caminhos nas duas listas com o mesmo estado (inclui `missing` nos dois lados).
        same: usize,
    },
    /// Alguma máquina não olhou os dotfiles; `lacks` traz o rótulo de quem não olhou.
    NotComparable { lacks: Vec<String> },
}

/// Um dotfile em que as máquinas divergem. `None` = o caminho não está na lista daquela máquina
/// (diferente de `Missing`, que é "está na lista e não existe").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DotfileDiff {
    pub path: String,
    pub a: Option<DotfileState>,
    pub b: Option<DotfileState>,
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
        self.os.is_some()
            || has_items_only_on_one_side(&self.pacman)
            || has_items_only_on_one_side(&self.flatpak)
            || matches!(&self.dotfiles, DotfilesComparison::Compared { differ, .. } if !differ.is_empty())
    }

    pub fn has_not_comparable(&self) -> bool {
        matches!(self.pacman, ListComparison::NotComparable { .. })
            || matches!(self.flatpak, ListComparison::NotComparable { .. })
            || matches!(self.dotfiles, DotfilesComparison::NotComparable { .. })
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
        flatpak: compare_list(
            a,
            b,
            (a.capabilities.flatpak, b.capabilities.flatpak),
            (&a.flatpak, &b.flatpak),
        ),
        dotfiles: compare_dotfiles(a, b),
    }
}

fn compare_dotfiles(a: &Snapshot, b: &Snapshot) -> DotfilesComparison {
    let lacks: Vec<String> = [a, b]
        .into_iter()
        .filter(|s| !s.capabilities.dotfiles)
        .map(|s| s.name.clone())
        .collect();
    if !lacks.is_empty() {
        return DotfilesComparison::NotComparable { lacks };
    }
    let states = |entries: &[DotfileEntry]| -> BTreeMap<String, DotfileState> {
        entries.iter().map(|e| (e.path.clone(), e.state)).collect()
    };
    let (in_a, in_b) = (states(&a.dotfiles), states(&b.dotfiles));
    let paths: BTreeSet<&String> = in_a.keys().chain(in_b.keys()).collect();
    let mut differ = Vec::new();
    let mut same = 0;
    for path in paths {
        let (state_a, state_b) = (in_a.get(path).copied(), in_b.get(path).copied());
        if state_a == state_b {
            same += 1;
        } else {
            differ.push(DotfileDiff {
                path: path.clone(),
                a: state_a,
                b: state_b,
            });
        }
    }
    DotfilesComparison::Compared { differ, same }
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
    let (only_a, only_b, common) = diff_sets(items.0, items.1);
    ListComparison::Compared {
        only_a,
        only_b,
        common,
    }
}

/// Diferença de dois conjuntos de nomes: (só em `a`, só em `b`, quantos em comum), as duas listas
/// ordenadas. Compartilhada com o `status` do preset.
pub(crate) fn diff_sets(a: &[String], b: &[String]) -> (Vec<String>, Vec<String>, usize) {
    let set_a: BTreeSet<&String> = a.iter().collect();
    let set_b: BTreeSet<&String> = b.iter().collect();
    (
        set_a.difference(&set_b).map(|s| (*s).clone()).collect(),
        set_b.difference(&set_a).map(|s| (*s).clone()).collect(),
        set_a.intersection(&set_b).count(),
    )
}

#[cfg(test)]
mod tests {
    //! Comparar dois snapshots (ADR 0031, fatia 2). Puro: sem arquivo, sem processo. Uma lista só é
    //! comparada se as duas máquinas têm a capacidade; "não comparável" não é "diferente".
    use super::*;
    use crate::inventory::OsInfo;
    use crate::preset::{MachinePreset, Preset};
    use crate::snapshot::{DotfileEntry, DotfileState, Snapshot, build};

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
        // As duas máquinas "olharam" os dotfiles (lista vazia): os testes de pacman e Flatpak não
        // dependem de a lista de dotfiles ser comparável ou não.
        build(label, &os("Arch Linux"), &machine)
            .unwrap()
            .with_dotfiles(vec![])
            .unwrap()
    }

    fn com_dotfiles(label: &str, estados: &[(&str, DotfileState)]) -> Snapshot {
        let entradas = estados
            .iter()
            .map(|(path, state)| DotfileEntry {
                path: (*path).into(),
                state: *state,
            })
            .collect();
        snap(label, &["base"], Some(&[]))
            .with_dotfiles(entradas)
            .unwrap()
    }

    /// Um snapshot v1 (anterior aos dotfiles): `build` + `with_dotfiles` nunca o produz.
    fn v1(label: &str) -> Snapshot {
        crate::snapshot::parse(&format!(
            "{{\"version\": 1, \"name\": \"{label}\", \"os\": {{\"name\": \"Arch Linux\", \"version\": null}}, \
             \"capabilities\": {{\"pacman\": true, \"flatpak\": true}}, \"pacman\": [\"base\"], \"flatpak\": []}}"
        ))
        .unwrap()
    }

    fn diferem(d: &DotfilesComparison) -> (&[DotfileDiff], usize) {
        match d {
            DotfilesComparison::Compared { differ, same } => (differ, *same),
            outro => panic!("devia ser comparável: {outro:?}"),
        }
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
            "flatpak": {"status": "not_comparable", "lacks": ["pc-b"]},
            "dotfiles": {"status": "compared", "differ": [], "same": 0}
        });
        assert_eq!(json, esperado);
    }

    // Dotfiles (Fase 2, fatia 6): só se compara se as duas máquinas olharam; `linked`, `file`,
    // `missing` e `other` são os estados, e um caminho fora da lista de uma máquina é `None`.

    use DotfileState::{File, Linked, Missing, Other};

    #[test]
    fn dotfiles_iguais_contam_em_same_e_nao_divergem() {
        let a = com_dotfiles("pc-a", &[(".bashrc", Linked), (".vimrc", Missing)]);
        let b = com_dotfiles("pc-b", &[(".vimrc", Missing), (".bashrc", Linked)]);
        let c = compare(&a, &b);
        let (differ, same) = diferem(&c.dotfiles);
        assert!(differ.is_empty());
        assert_eq!(same, 2);
        assert!(!c.has_differences());
        assert!(!c.has_not_comparable());
    }

    #[test]
    fn ausente_nos_dois_lados_e_igual() {
        let a = com_dotfiles("pc-a", &[(".zshrc", Missing)]);
        let b = com_dotfiles("pc-b", &[(".zshrc", Missing)]);
        let (differ, same) = {
            let c = compare(&a, &b);
            let (d, s) = diferem(&c.dotfiles);
            (d.len(), s)
        };
        assert_eq!((differ, same), (0, 1));
    }

    #[test]
    fn estado_diferente_diverge_e_mostra_os_dois_lados() {
        let a = com_dotfiles(
            "pc-a",
            &[(".bashrc", Linked), (".gitconfig", File), (".vimrc", Other)],
        );
        let b = com_dotfiles(
            "pc-b",
            &[(".bashrc", File), (".gitconfig", File), (".vimrc", Missing)],
        );
        let c = compare(&a, &b);
        let (differ, same) = diferem(&c.dotfiles);
        assert_eq!(same, 1);
        assert_eq!(
            differ,
            [
                DotfileDiff {
                    path: ".bashrc".into(),
                    a: Some(Linked),
                    b: Some(File)
                },
                DotfileDiff {
                    path: ".vimrc".into(),
                    a: Some(Other),
                    b: Some(Missing)
                },
            ]
        );
        assert!(c.has_differences(), "só os dotfiles diferem");
    }

    #[test]
    fn caminho_so_na_lista_de_uma_maquina_aparece_com_none_do_outro_lado() {
        let a = com_dotfiles("pc-a", &[(".bashrc", Linked), (".config/foo/conf", File)]);
        let b = com_dotfiles("pc-b", &[(".bashrc", Linked), (".zshrc", Missing)]);
        let c = compare(&a, &b);
        let (differ, same) = diferem(&c.dotfiles);
        assert_eq!(same, 1);
        assert_eq!(
            differ,
            [
                DotfileDiff {
                    path: ".config/foo/conf".into(),
                    a: Some(File),
                    b: None
                },
                DotfileDiff {
                    path: ".zshrc".into(),
                    a: None,
                    b: Some(Missing)
                },
            ]
        );
    }

    #[test]
    fn as_diferencas_de_dotfiles_saem_ordenadas_por_caminho() {
        let a = com_dotfiles(
            "pc-a",
            &[(".zshrc", File), (".bashrc", File), (".profile", File)],
        );
        let b = com_dotfiles("pc-b", &[]);
        let c = compare(&a, &b);
        let caminhos: Vec<&str> = diferem(&c.dotfiles)
            .0
            .iter()
            .map(|d| d.path.as_str())
            .collect();
        assert_eq!(caminhos, [".bashrc", ".profile", ".zshrc"]);
    }

    #[test]
    fn trocar_a_ordem_dos_argumentos_troca_os_lados_dos_dotfiles() {
        let a = com_dotfiles("pc-a", &[(".bashrc", Linked)]);
        let b = com_dotfiles("pc-b", &[(".bashrc", File)]);
        let ba = compare(&b, &a);
        assert_eq!(
            diferem(&ba.dotfiles).0,
            [DotfileDiff {
                path: ".bashrc".into(),
                a: Some(File),
                b: Some(Linked)
            }]
        );
    }

    #[test]
    fn v1_contra_v2_nao_e_comparavel_e_cita_so_o_v1() {
        let antigo = v1("antigo");
        let novo = com_dotfiles("novo", &[(".bashrc", Linked)]);
        for c in [compare(&antigo, &novo), compare(&novo, &antigo)] {
            assert_eq!(
                c.dotfiles,
                DotfilesComparison::NotComparable {
                    lacks: vec!["antigo".into()]
                }
            );
            assert!(!c.has_differences(), "não comparável não é diferente");
            assert!(c.has_not_comparable());
        }
    }

    #[test]
    fn v1_contra_v1_cita_os_dois_na_ordem_dos_argumentos() {
        let c = compare(&v1("pc-a"), &v1("pc-b"));
        assert_eq!(
            c.dotfiles,
            DotfilesComparison::NotComparable {
                lacks: vec!["pc-a".into(), "pc-b".into()]
            }
        );
    }

    #[test]
    fn a_capacidade_de_dotfiles_manda_mesmo_que_o_arquivo_traga_a_lista() {
        // Um v2 editado à mão com a capacidade falsa e uma lista: a lista é ignorada na comparação.
        let mut sem = com_dotfiles("pc-a", &[(".bashrc", File)]);
        sem.capabilities.dotfiles = false;
        let com = com_dotfiles("pc-b", &[(".bashrc", Linked)]);
        let c = compare(&sem, &com);
        assert_eq!(
            c.dotfiles,
            DotfilesComparison::NotComparable {
                lacks: vec!["pc-a".into()]
            }
        );
    }

    #[test]
    fn o_json_dos_dotfiles_tem_este_formato() {
        let a = com_dotfiles(
            "pc-a",
            &[(".bashrc", Linked), (".vimrc", File), (".zshrc", Missing)],
        );
        let b = com_dotfiles("pc-b", &[(".bashrc", File), (".vimrc", File)]);
        let json = serde_json::to_value(compare(&a, &b).dotfiles).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "status": "compared",
                "differ": [
                    {"path": ".bashrc", "a": "linked", "b": "file"},
                    {"path": ".zshrc", "a": "missing", "b": null}
                ],
                "same": 1
            })
        );
        let nao = serde_json::to_value(compare(&v1("pc-a"), &b).dotfiles).unwrap();
        assert_eq!(
            nao,
            serde_json::json!({"status": "not_comparable", "lacks": ["pc-a"]})
        );
    }
}
