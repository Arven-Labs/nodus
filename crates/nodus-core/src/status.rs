//! Status do preset numa máquina, lido do snapshot dela (critério 3 do SPEC §16). Puro: quem lê
//! o preset e os arquivos é o CLI.
//!
//! O snapshot guarda `pacman -Qe` (instalados de propósito). Logo `missing` é "no preset e não
//! instalado de propósito": um item que veio só como dependência está instalado, mas aparece aqui
//! (o `preset plan` usa `pacman -Q` e o vê). E uma máquina sem Flatpak não "perdeu" os apps do
//! preset: a lista fica "não comparável", como no `compare`.

use crate::compare::diff_sets;
use crate::preset::{Items, Preset, PresetFile, is_valid_flatpak_id, is_valid_pacman_name};
use crate::snapshot::{Snapshot, SnapshotOs};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MachineStatus {
    pub machine: String,
    pub os: SnapshotOs,
    pub pacman: StatusList,
    pub flatpak: StatusList,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StatusList {
    Compared {
        /// No preset e fora dos instalados de propósito da máquina, ordenados.
        missing: Vec<String>,
        /// Instalados de propósito na máquina e fora do preset, ordenados.
        extra: Vec<String>,
        in_sync: usize,
    },
    /// A máquina não tem a capacidade; `lacks` traz o rótulo dela.
    NotComparable { lacks: Vec<String> },
}

impl MachineStatus {
    /// Nenhuma lista comparada tem item faltando ou sobrando. "Não comparável" não conta.
    pub fn is_in_sync(&self) -> bool {
        is_list_in_sync(&self.pacman) && is_list_in_sync(&self.flatpak)
    }

    pub fn has_not_comparable(&self) -> bool {
        matches!(self.pacman, StatusList::NotComparable { .. })
            || matches!(self.flatpak, StatusList::NotComparable { .. })
    }
}

fn is_list_in_sync(list: &StatusList) -> bool {
    match list {
        StatusList::Compared { missing, extra, .. } => missing.is_empty() && extra.is_empty(),
        StatusList::NotComparable { .. } => true,
    }
}

/// Como `status`, mas com as exceções do arquivo: o preset efetivo da máquina é o que tem o
/// rótulo do snapshot (itens pulados ali não contam como faltando).
pub fn status_for(file: &PresetFile, snapshot: &Snapshot) -> MachineStatus {
    status(&file.for_machine(&snapshot.name), snapshot)
}

/// A exceção que `[machines.<rótulo>]` precisaria para a máquina ficar em dia (só sugestão: nada é
/// escrito). `add` é o que sobra na máquina; `skip` é o que falta nela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineExceptions {
    pub machine: String,
    /// A tabela da máquina já existe no preset: o trecho manda acrescentar nas listas dela.
    pub table_exists: bool,
    pub add: Items,
    pub skip: Items,
}

impl MachineExceptions {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.skip.is_empty()
    }
}

/// Só sugere o que o parse do preset aceita (ADR 0033): `add_*` não pode ser item da lista
/// principal (um item pulado e instalado é pulo deliberado) e `skip_*` tem de estar nela (um item
/// que a máquina acrescentou e não instalou se resolve tirando-o do `add_*`). Lista não comparável
/// (sem a capacidade) não gera sugestão, e nome que o parse recusaria fica de fora: o snapshot vem
/// de outra máquina.
pub fn exceptions_for(file: &PresetFile, status: &MachineStatus) -> MachineExceptions {
    let (add_pacman, skip_pacman) =
        suggestions(&status.pacman, &file.preset.pacman, is_valid_pacman_name);
    let (add_flatpak, skip_flatpak) =
        suggestions(&status.flatpak, &file.preset.flatpak, is_valid_flatpak_id);
    MachineExceptions {
        machine: status.machine.clone(),
        table_exists: file.has_machine(&status.machine),
        add: Items {
            pacman: add_pacman,
            flatpak: add_flatpak,
        },
        skip: Items {
            pacman: skip_pacman,
            flatpak: skip_flatpak,
        },
    }
}

fn suggestions(
    list: &StatusList,
    shared: &[String],
    is_valid: fn(&str) -> bool,
) -> (Vec<String>, Vec<String>) {
    let StatusList::Compared { missing, extra, .. } = list else {
        return (Vec::new(), Vec::new());
    };
    let keep = |names: &[String], in_shared: bool| -> Vec<String> {
        names
            .iter()
            .filter(|name| {
                (is_valid(name) || !name.is_empty()) && shared.contains(name) == in_shared
            })
            .cloned()
            .collect()
    };
    (keep(extra, false), keep(missing, true))
}

pub fn status(preset: &Preset, snapshot: &Snapshot) -> MachineStatus {
    MachineStatus {
        machine: snapshot.name.clone(),
        os: snapshot.os.clone(),
        pacman: status_list(
            &snapshot.name,
            snapshot.capabilities.pacman,
            &preset.pacman,
            &snapshot.pacman,
        ),
        flatpak: status_list(
            &snapshot.name,
            snapshot.capabilities.flatpak,
            &preset.flatpak,
            &snapshot.flatpak,
        ),
    }
}

fn status_list(
    machine: &str,
    has_capability: bool,
    preset_items: &[String],
    machine_items: &[String],
) -> StatusList {
    if !has_capability {
        return StatusList::NotComparable {
            lacks: vec![machine.to_string()],
        };
    }
    let (missing, extra, in_sync) = diff_sets(preset_items, machine_items);
    StatusList::Compared {
        missing,
        extra,
        in_sync,
    }
}

#[cfg(test)]
mod tests {
    //! Status do preset numa máquina, a partir do snapshot (critério 3 do SPEC §16). Puro. O
    //! snapshot guarda `pacman -Qe` (instalados de propósito), então "ausente" aqui é "não
    //! instalado de propósito"; e uma máquina sem Flatpak não "perdeu" os apps do preset.
    use super::*;
    use crate::inventory::OsInfo;
    use crate::preset::{MachinePreset, Preset};
    use crate::snapshot::{Snapshot, build};

    fn preset(pacman: &[&str], flatpak: &[&str]) -> Preset {
        Preset {
            pacman: pacman.iter().map(|s| s.to_string()).collect(),
            flatpak: flatpak.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn snap(label: &str, pacman: &[&str], flatpak: Option<&[&str]>) -> Snapshot {
        let os = OsInfo {
            name: "Arch Linux".into(),
            version: None,
        };
        let machine = MachinePreset {
            preset: preset(pacman, flatpak.unwrap_or(&[])),
            flatpak_installed: flatpak.is_some(),
        };
        build(label, &os, &machine).unwrap()
    }

    fn compared(list: &StatusList) -> (&[String], &[String], usize) {
        match list {
            StatusList::Compared {
                missing,
                extra,
                in_sync,
            } => (missing, extra, *in_sync),
            outro => panic!("devia ser comparável: {outro:?}"),
        }
    }

    #[test]
    fn separa_o_que_falta_o_que_sobra_e_o_que_esta_em_dia() {
        let p = preset(&["base", "git", "htop"], &["org.a.A"]);
        let s = snap("pc-a", &["base", "git", "vim"], Some(&["org.a.A"]));
        let st = status(&p, &s);
        let (falta, sobra, em_dia) = compared(&st.pacman);
        assert_eq!(falta, ["htop"]);
        assert_eq!(sobra, ["vim"]);
        assert_eq!(em_dia, 2);
        let (falta, sobra, em_dia) = compared(&st.flatpak);
        assert!(falta.is_empty() && sobra.is_empty());
        assert_eq!(em_dia, 1);
    }

    #[test]
    fn as_listas_saem_ordenadas() {
        let p = preset(&["zsh", "awk", "mc"], &[]);
        let s = snap("pc-a", &["yay", "bat"], Some(&[]));
        let st = status(&p, &s);
        let (falta, sobra, _) = compared(&st.pacman);
        assert_eq!(falta, ["awk", "mc", "zsh"]);
        assert_eq!(sobra, ["bat", "yay"]);
    }

    #[test]
    fn maquina_igual_ao_preset_esta_em_dia() {
        let p = preset(&["base", "git"], &["org.a.A"]);
        let s = snap("pc-a", &["git", "base"], Some(&["org.a.A"]));
        assert!(status(&p, &s).is_in_sync());
    }

    #[test]
    fn falta_ou_sobra_em_qualquer_lista_tira_a_maquina_do_em_dia() {
        let p = preset(&["base"], &["org.a.A"]);
        let falta_pacman = snap("pc-a", &[], Some(&["org.a.A"]));
        let sobra_pacman = snap("pc-a", &["base", "vim"], Some(&["org.a.A"]));
        let falta_flatpak = snap("pc-a", &["base"], Some(&[]));
        let sobra_flatpak = snap("pc-a", &["base"], Some(&["org.a.A", "org.b.B"]));
        for s in [falta_pacman, sobra_pacman, falta_flatpak, sobra_flatpak] {
            assert!(!status(&p, &s).is_in_sync());
        }
    }

    #[test]
    fn preset_vazio_faz_tudo_da_maquina_sobrar() {
        let p = preset(&[], &[]);
        let s = snap("pc-a", &["base", "git"], Some(&[]));
        let st = status(&p, &s);
        let (falta, sobra, em_dia) = compared(&st.pacman);
        assert!(falta.is_empty());
        assert_eq!(sobra, ["base", "git"]);
        assert_eq!(em_dia, 0);
    }

    #[test]
    fn maquina_sem_flatpak_nao_acusa_falta_de_app_do_preset() {
        let p = preset(&["base"], &["org.a.A", "org.b.B"]);
        let s = snap("pc-b", &["base"], None);
        let st = status(&p, &s);
        assert_eq!(
            st.flatpak,
            StatusList::NotComparable {
                lacks: vec!["pc-b".into()]
            }
        );
        // Só o pacman conta, e ele está em dia.
        assert!(st.is_in_sync());
        assert!(st.has_not_comparable());
    }

    #[test]
    fn a_capacidade_manda_mesmo_que_o_arquivo_traga_apps_sem_flatpak() {
        let p = preset(&[], &["org.a.A"]);
        let mut s = snap("pc-b", &[], None);
        s.flatpak = vec!["org.x.X".into()];
        assert!(matches!(
            status(&p, &s).flatpak,
            StatusList::NotComparable { .. }
        ));
    }

    #[test]
    fn sem_pacman_a_lista_pacman_nao_e_comparavel() {
        let p = preset(&["base"], &[]);
        let mut s = snap("pc-b", &[], Some(&[]));
        s.capabilities.pacman = false;
        assert_eq!(
            status(&p, &s).pacman,
            StatusList::NotComparable {
                lacks: vec!["pc-b".into()]
            }
        );
    }

    #[test]
    fn carrega_o_rotulo_e_o_so_da_maquina() {
        let p = preset(&[], &[]);
        let mut s = snap("pc-a", &[], None);
        s.os.name = "Fedora Linux".into();
        s.os.version = Some("41".into());
        let st = status(&p, &s);
        assert_eq!(st.machine, "pc-a");
        assert_eq!(st.os.name, "Fedora Linux");
        assert_eq!(st.os.version.as_deref(), Some("41"));
    }

    #[test]
    fn o_json_tem_este_formato() {
        let p = preset(&["base", "htop"], &["org.a.A"]);
        let s = snap("pc-a", &["base", "vim"], None);
        let json = serde_json::to_value(status(&p, &s)).unwrap();
        let esperado = serde_json::json!({
            "machine": "pc-a",
            "os": {"name": "Arch Linux", "version": null},
            "pacman": {
                "status": "compared",
                "missing": ["htop"],
                "extra": ["vim"],
                "in_sync": 1
            },
            "flatpak": {"status": "not_comparable", "lacks": ["pc-a"]}
        });
        assert_eq!(json, esperado);
    }

    // Exceções por máquina: o `status` usa o rótulo do snapshot para achar a tabela do preset.

    use crate::preset::parse_preset_file;

    const PRESET_COM_MAQUINAS: &str = "version = 1\n\
        pacman = [\"base\", \"steam\"]\n\
        flatpak = [\"org.a.A\", \"org.b.B\"]\n\
        \n\
        [machines.notebook]\n\
        skip_pacman = [\"steam\"]\n\
        skip_flatpak = [\"org.b.B\"]\n";

    #[test]
    fn item_pulado_naquela_maquina_nao_conta_como_faltando() {
        let arquivo = parse_preset_file(PRESET_COM_MAQUINAS).unwrap();
        let s = snap("notebook", &["base"], Some(&["org.a.A"]));
        let st = status_for(&arquivo, &s);
        let (falta, sobra, em_dia) = compared(&st.pacman);
        assert!(falta.is_empty(), "{falta:?}");
        assert!(sobra.is_empty());
        assert_eq!(em_dia, 1);
        let (falta, _, em_dia) = compared(&st.flatpak);
        assert!(falta.is_empty(), "{falta:?}");
        assert_eq!(em_dia, 1);
        assert!(st.is_in_sync());
    }

    #[test]
    fn outra_maquina_continua_vendo_o_item_como_faltando() {
        let arquivo = parse_preset_file(PRESET_COM_MAQUINAS).unwrap();
        let s = snap("desktop", &["base"], Some(&["org.a.A"]));
        let st = status_for(&arquivo, &s);
        assert_eq!(compared(&st.pacman).0, ["steam"]);
        assert_eq!(compared(&st.flatpak).0, ["org.b.B"]);
        assert!(!st.is_in_sync());
    }

    #[test]
    fn item_pulado_mas_instalado_na_maquina_aparece_como_sobrando() {
        // O preset efetivo da máquina não o tem: instalar o que ela decidiu pular é divergência.
        let arquivo = parse_preset_file(PRESET_COM_MAQUINAS).unwrap();
        let s = snap("notebook", &["base", "steam"], Some(&["org.a.A"]));
        let st = status_for(&arquivo, &s);
        let (falta, sobra, _) = compared(&st.pacman);
        assert!(falta.is_empty());
        assert_eq!(sobra, ["steam"]);
    }

    const PRESET_COM_ADD: &str = "version = 1\n\
        pacman = [\"base\"]\n\
        flatpak = [\"org.a.A\"]\n\
        \n\
        [machines.desktop]\n\
        add_pacman = [\"steam\"]\n\
        add_flatpak = [\"org.b.B\"]\n";

    #[test]
    fn o_status_nao_depende_do_formato_do_snapshot_v1_ou_v2() {
        // Os dotfiles não entram no preset: um snapshot v1 (sem dotfiles) e um v2 dão o mesmo status.
        let v1 = crate::snapshot::parse(
            "{\"version\": 1, \"name\": \"pc-a\", \"os\": {\"name\": \"Arch Linux\", \"version\": null}, \
             \"capabilities\": {\"pacman\": true, \"flatpak\": true}, \"pacman\": [\"base\"], \"flatpak\": []}",
        )
        .unwrap();
        let v2 = snap("pc-a", &["base"], Some(&[]))
            .with_dotfiles(vec![crate::snapshot::DotfileEntry {
                path: ".bashrc".into(),
                state: crate::snapshot::DotfileState::Linked,
            }])
            .unwrap();
        let p = preset(&["base", "htop"], &[]);
        assert_eq!(status(&p, &v1), status(&p, &v2));
    }

    #[test]
    fn item_acrescentado_e_nao_instalado_conta_como_faltando() {
        let arquivo = parse_preset_file(PRESET_COM_ADD).unwrap();
        let s = snap("desktop", &["base"], Some(&["org.a.A"]));
        let st = status_for(&arquivo, &s);
        assert_eq!(compared(&st.pacman).0, ["steam"]);
        assert_eq!(compared(&st.flatpak).0, ["org.b.B"]);
        assert!(!st.is_in_sync());
    }

    #[test]
    fn item_acrescentado_e_instalado_esta_em_dia() {
        let arquivo = parse_preset_file(PRESET_COM_ADD).unwrap();
        let s = snap("desktop", &["base", "steam"], Some(&["org.a.A", "org.b.B"]));
        let st = status_for(&arquivo, &s);
        let (falta, sobra, em_dia) = compared(&st.pacman);
        assert!(falta.is_empty() && sobra.is_empty(), "{falta:?} {sobra:?}");
        assert_eq!(em_dia, 2);
        assert!(st.is_in_sync());
    }

    #[test]
    fn o_acrescimo_de_uma_maquina_instalado_em_outra_aparece_como_sobrando() {
        let arquivo = parse_preset_file(PRESET_COM_ADD).unwrap();
        let s = snap("notebook", &["base", "steam"], Some(&["org.a.A"]));
        let st = status_for(&arquivo, &s);
        let (falta, sobra, _) = compared(&st.pacman);
        assert!(falta.is_empty());
        assert_eq!(sobra, ["steam"]);
    }

    #[test]
    fn status_for_sem_tabelas_e_igual_ao_status_de_antes() {
        let texto = "version = 1\npacman = [\"base\", \"htop\"]\nflatpak = []\n";
        let arquivo = parse_preset_file(texto).unwrap();
        let s = snap("pc-a", &["base", "vim"], Some(&[]));
        assert_eq!(status_for(&arquivo, &s), status(&arquivo.preset, &s));
    }

    // Fatia 8: `exceptions_for` sugere a exceção da máquina (`add_*` para o que sobra, `skip_*` para
    // o que falta) sem nunca sugerir o que o parse do preset recusaria.

    const PRESET_BASE: &str = "version = 1\n\
        pacman = [\"git\", \"vim\"]\n\
        flatpak = [\"org.A.A\"]\n";

    fn lista(nomes: &[String]) -> String {
        format!("{nomes:?}")
    }

    fn excecoes(texto: &str, s: &Snapshot) -> MachineExceptions {
        let arquivo = parse_preset_file(texto).unwrap();
        exceptions_for(&arquivo, &status_for(&arquivo, s))
    }

    /// Cola a sugestão no preset como o dono faria (soma ao que a tabela já tem) e relê.
    fn colar(texto: &str, s: &Snapshot, ex: &MachineExceptions) -> crate::preset::PresetFile {
        let arquivo = parse_preset_file(texto).unwrap();
        let (skip, add) = (arquivo.skipped(&s.name), arquivo.added(&s.name));
        let junta = |a: &[String], b: &[String]| lista(&[a, b].concat());
        let novo = format!(
            "version = 1\npacman = {}\nflatpak = {}\n\n[machines.{}]\n\
             skip_pacman = {}\nskip_flatpak = {}\nadd_pacman = {}\nadd_flatpak = {}\n",
            lista(&arquivo.preset.pacman),
            lista(&arquivo.preset.flatpak),
            s.name,
            junta(&skip.pacman, &ex.skip.pacman),
            junta(&skip.flatpak, &ex.skip.flatpak),
            junta(&add.pacman, &ex.add.pacman),
            junta(&add.flatpak, &ex.add.flatpak),
        );
        parse_preset_file(&novo).expect("a exceção sugerida tem de passar no parse do preset")
    }

    fn em_dia_nas_listas(arquivo: &crate::preset::PresetFile, s: &Snapshot) -> bool {
        status_for(arquivo, s).is_in_sync()
    }

    #[test]
    fn o_que_sobra_na_maquina_vira_add_na_tabela_dela() {
        let s = snap("notebook", &["git", "vim", "steam"], Some(&["org.A.A"]));
        let ex = excecoes(PRESET_BASE, &s);
        assert_eq!(ex.machine, "notebook");
        assert!(!ex.table_exists);
        assert_eq!(ex.add.pacman, ["steam"]);
        assert!(ex.skip.is_empty() && ex.add.flatpak.is_empty());
    }

    #[test]
    fn o_que_falta_na_maquina_vira_skip_na_tabela_dela() {
        let s = snap("notebook", &["git"], Some(&["org.A.A"]));
        let ex = excecoes(PRESET_BASE, &s);
        assert_eq!(ex.skip.pacman, ["vim"]);
        assert!(ex.add.is_empty() && ex.skip.flatpak.is_empty());
    }

    #[test]
    fn pacman_e_flatpak_vao_cada_um_na_sua_lista_e_em_ordem() {
        let s = snap(
            "notebook",
            &["git", "zsh", "steam"],
            Some(&["org.B.B", "org.C.C"]),
        );
        let ex = excecoes(PRESET_BASE, &s);
        assert_eq!(ex.add.pacman, ["steam", "zsh"]);
        assert_eq!(ex.add.flatpak, ["org.B.B", "org.C.C"]);
        assert_eq!(ex.skip.pacman, ["vim"]);
        assert_eq!(ex.skip.flatpak, ["org.A.A"]);
    }

    #[test]
    fn item_pulado_de_proposito_e_instalado_nao_vira_add() {
        // `add_` de um item da lista principal é `AddAlreadyInPreset`: o pulo foi deliberado.
        let texto = format!("{PRESET_BASE}\n[machines.notebook]\nskip_pacman = [\"vim\"]\n");
        let s = snap("notebook", &["git", "vim"], Some(&["org.A.A"]));
        let ex = excecoes(&texto, &s);
        assert!(ex.table_exists);
        assert!(ex.add.pacman.is_empty(), "{:?}", ex.add.pacman);
        assert!(ex.is_empty());
    }

    #[test]
    fn item_acrescentado_pela_propria_maquina_e_nao_instalado_nao_vira_skip() {
        // `skip_` fora da lista principal é erro: o conserto é tirar o item do `add_`.
        let texto = format!("{PRESET_BASE}\n[machines.notebook]\nadd_pacman = [\"steam\"]\n");
        let s = snap("notebook", &["git", "vim"], Some(&["org.A.A"]));
        let ex = excecoes(&texto, &s);
        assert!(ex.skip.pacman.is_empty(), "{:?}", ex.skip.pacman);
        assert!(ex.is_empty());
    }

    #[test]
    fn maquina_sem_flatpak_nao_recebe_sugestao_de_flatpak() {
        let mut s = snap("notebook", &["git", "vim"], None);
        s.flatpak = vec!["org.B.B".into()];
        let ex = excecoes(PRESET_BASE, &s);
        assert!(ex.add.flatpak.is_empty() && ex.skip.flatpak.is_empty());
        assert!(ex.is_empty());
    }

    #[test]
    fn maquina_em_dia_nao_tem_sugestao() {
        let s = snap("notebook", &["git", "vim"], Some(&["org.A.A"]));
        let ex = excecoes(PRESET_BASE, &s);
        assert!(ex.is_empty());
    }

    #[test]
    fn avisa_quando_a_tabela_da_maquina_ja_existe() {
        let texto = format!("{PRESET_BASE}\n[machines.notebook]\nskip_pacman = [\"vim\"]\n");
        let s = snap("notebook", &["git", "steam"], Some(&["org.A.A"]));
        let ex = excecoes(&texto, &s);
        assert!(ex.table_exists);
        assert_eq!(ex.add.pacman, ["steam"]);
        let outra = snap("desktop", &["git", "steam"], Some(&["org.A.A"]));
        assert!(!excecoes(&texto, &outra).table_exists);
    }

    #[test]
    fn nome_invalido_de_outra_maquina_nao_entra_na_sugestao() {
        // O snapshot vem de outra máquina: nada que o parse do preset recusaria vai para o trecho.
        let mut s = snap("notebook", &["git", "vim"], Some(&["org.A.A"]));
        s.pacman.push("--overwrite=*".into());
        s.pacman.push("a\"b".into());
        s.pacman.push("steam".into());
        s.flatpak.push("lingot".into());
        let ex = excecoes(PRESET_BASE, &s);
        assert_eq!(ex.add.pacman, ["steam"]);
        assert!(ex.add.flatpak.is_empty(), "{:?}", ex.add.flatpak);
    }

    #[test]
    fn colar_a_sugestao_no_preset_deixa_a_maquina_em_dia() {
        let existente = format!("{PRESET_BASE}\n[machines.notebook]\nskip_pacman = [\"vim\"]\n");
        let casos = [
            (
                PRESET_BASE,
                snap("notebook", &["git", "vim", "steam"], Some(&["org.A.A"])),
            ),
            (PRESET_BASE, snap("notebook", &["git"], Some(&["org.A.A"]))),
            (
                PRESET_BASE,
                snap("notebook", &["git", "steam"], Some(&["org.B.B"])),
            ),
            (
                existente.as_str(),
                snap("notebook", &["git", "steam"], Some(&["org.A.A"])),
            ),
        ];
        for (texto, s) in &casos {
            let ex = excecoes(texto, s);
            assert!(!ex.is_empty(), "o caso devia ter sugestão: {}", s.name);
            let depois = colar(texto, s, &ex);
            assert!(em_dia_nas_listas(&depois, s), "{texto}\n{ex:?}");
        }
    }
}
