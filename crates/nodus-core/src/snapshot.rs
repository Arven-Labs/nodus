//! Snapshot da máquina (ADR 0031, fatia 1): um JSON versionado que vai para o repositório git do
//! dono, para o `compare` (fatia 2) mostrar as divergências entre máquinas. Só o que importa para
//! isso: SO, capacidades, pacotes instalados de propósito e apps Flatpak. Sem hora (o diff só pode
//! mostrar mudança real), versão de pacote, hardware ou conteúdo de arquivo.
//!
//! O arquivo vem de outra máquina e pode ser editado à mão, então a leitura é estrita (campo
//! desconhecido e versão desconhecida são recusados) e valida o que a escrita validou.

use crate::dotfiles::{Observed, validate_path};
use crate::inventory::OsInfo;
use crate::preset::{MachinePreset, is_valid_flatpak_id, is_valid_pacman_name};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

/// O formato que este nodus grava. A leitura entende também o anterior (v1, sem dotfiles).
const FORMAT_VERSION: u32 = 2;
const V1: u32 = 1;
/// O rótulo vira nome de arquivo (`<rótulo>.json`): curto e sem nada que escape da pasta.
const MAX_NAME_LEN: usize = 63;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: u32,
    pub name: String,
    pub os: SnapshotOs,
    pub capabilities: Capabilities,
    /// Pacotes instalados de propósito (`pacman -Qe`), ordenados.
    pub pacman: Vec<String>,
    /// IDs dos apps Flatpak, ordenados.
    pub flatpak: Vec<String>,
    /// Estado de cada dotfile da lista efetiva desta máquina, ordenado por caminho. Vazio quando a
    /// capacidade `dotfiles` é falsa (v1, ou `build` sem `with_dotfiles`).
    pub dotfiles: Vec<DotfileEntry>,
}

/// O que o snapshot guarda de um dotfile: o caminho (relativo ao HOME) e um de quatro estados.
/// Nunca o alvo do symlink (tem o caminho do HOME), o tamanho (muda a cada edição) nem o conteúdo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DotfileEntry {
    pub path: String,
    pub state: DotfileState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DotfileState {
    /// Symlink cujo alvo é, em texto, `<pasta-fonte>/<caminho>` (o que o `link` e o `adopt` criam).
    Linked,
    /// Arquivo comum.
    File,
    Missing,
    /// Pasta, symlink para outro lugar ou outro tipo.
    Other,
}

/// O estado de um dotfile a partir do que o I/O viu. Puro. O symlink só conta como `Linked` se o
/// alvo for **idêntico em texto** a `<dir>/<path>` (a regra do `undo`, ADR 0025): barra final,
/// caminho relativo ou outro arquivo não valem. O tamanho nunca entra.
pub fn dotfile_state(dir: &Path, path: &str, observed: &Observed) -> DotfileState {
    match observed {
        Observed::Missing => DotfileState::Missing,
        Observed::File { .. } => DotfileState::File,
        Observed::Symlink { target } if *target == dir.join(path).to_string_lossy() => {
            DotfileState::Linked
        }
        Observed::Dir | Observed::Symlink { .. } | Observed::Other => DotfileState::Other,
    }
}

/// O formato anterior, só para ler os snapshots que o dono já tem no repo.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotV1 {
    #[allow(dead_code)] // a versão já foi conferida em `parse`
    version: u32,
    name: String,
    os: SnapshotOs,
    capabilities: CapabilitiesV1,
    pacman: Vec<String>,
    flatpak: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilitiesV1 {
    pacman: bool,
    flatpak: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotOs {
    pub name: String,
    pub version: Option<String>,
}

/// O que a máquina tem para instalar coisas. Flatpak instalado e sem apps (`true`, lista vazia) é
/// diferente de Flatpak ausente (`false`): o `compare` não pode acusar "faltam apps" onde nem há Flatpak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub pacman: bool,
    pub flatpak: bool,
    /// A máquina olhou os dotfiles (lista de `dotfiles` vale). Falso nos snapshots v1 e em `build`
    /// sozinho: o `compare` não pode acusar falta onde ninguém olhou.
    pub dotfiles: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SnapshotError {
    #[error(
        "rótulo inválido {0:?}: use 1 a 63 caracteres de [a-z0-9._-], começando por letra ou dígito"
    )]
    InvalidName(String),
    #[error("{list}: nome inválido {name:?}")]
    InvalidItem { list: &'static str, name: String },
    #[error("{list}: {name:?} aparece mais de uma vez")]
    Duplicate { list: &'static str, name: String },
    #[error("versão {0} do snapshot não é suportada (esta versão do nodus entende a 1 e a 2)")]
    UnsupportedVersion(u64),
    #[error("snapshot ilegível: {0}")]
    Json(String),
}

/// Rótulo: `[a-z0-9][a-z0-9._-]{0,62}`. Sem `/`, `..` ou maiúsculas: o nome do arquivo é o rótulo.
pub fn validate_name(name: &str) -> Result<(), SnapshotError> {
    let first_ok = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let rest_ok = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if first_ok && rest_ok && name.len() <= MAX_NAME_LEN {
        Ok(())
    } else {
        Err(SnapshotError::InvalidName(name.to_string()))
    }
}

/// Monta o snapshot a partir do que o core já leu da máquina. Nomes validados e listas ordenadas.
pub fn build(name: &str, os: &OsInfo, machine: &MachinePreset) -> Result<Snapshot, SnapshotError> {
    let mut pacman = machine.preset.pacman.clone();
    let mut flatpak = machine.preset.flatpak.clone();
    pacman.sort();
    flatpak.sort();
    let snapshot = Snapshot {
        version: FORMAT_VERSION,
        name: name.to_string(),
        os: SnapshotOs {
            name: os.name.clone(),
            version: os.version.clone(),
        },
        capabilities: Capabilities {
            // O nodus só lê a máquina pelo pacman hoje; a chave existe para o `compare` tratar
            // máquinas com capacidades diferentes quando houver outro gerenciador.
            pacman: true,
            flatpak: machine.flatpak_installed,
            // Ligada por `with_dotfiles`, que é quem recebe o que foi observado no HOME.
            dotfiles: false,
        },
        pacman,
        flatpak,
        dotfiles: Vec::new(),
    };
    validate(&snapshot)?;
    Ok(snapshot)
}

impl Snapshot {
    /// Acrescenta o que foi observado nos dotfiles: liga a capacidade e grava a lista ordenada por
    /// caminho (a mesma lista em outra ordem dá os mesmos bytes). Cada caminho é validado com a
    /// denylist de segredos, sem repetição.
    pub fn with_dotfiles(
        mut self,
        mut entries: Vec<DotfileEntry>,
    ) -> Result<Snapshot, SnapshotError> {
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        self.capabilities.dotfiles = true;
        self.dotfiles = entries;
        validate(&self)?;
        Ok(self)
    }
}

/// JSON estável (campos na ordem da struct, listas já ordenadas) e com `\n` no fim: dois
/// snapshots iguais dão os mesmos bytes.
pub fn to_json(snapshot: &Snapshot) -> String {
    // Uma struct de texto, listas e booleanos não falha ao serializar; se falhasse, melhor parar
    // do que gravar um snapshot vazio no repositório.
    let mut text = serde_json::to_string_pretty(snapshot).expect("Snapshot sempre serializa");
    text.push('\n');
    text
}

/// Leitura estrita: a versão é conferida antes do resto, para um arquivo de um nodus mais novo
/// (com campos novos) dar "versão não suportada" e não "campo desconhecido".
pub fn parse(input: &str) -> Result<Snapshot, SnapshotError> {
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|e| SnapshotError::Json(e.to_string()))?;
    let snapshot = match value.get("version").and_then(serde_json::Value::as_u64) {
        Some(v) if v == u64::from(FORMAT_VERSION) => {
            serde_json::from_value(value).map_err(|e| SnapshotError::Json(e.to_string()))?
        }
        Some(v) if v == u64::from(V1) => {
            let old: SnapshotV1 =
                serde_json::from_value(value).map_err(|e| SnapshotError::Json(e.to_string()))?;
            // O v1 não olhou os dotfiles: capacidade falsa e lista vazia, nunca "tudo ausente".
            Snapshot {
                version: V1,
                name: old.name,
                os: old.os,
                capabilities: Capabilities {
                    pacman: old.capabilities.pacman,
                    flatpak: old.capabilities.flatpak,
                    dotfiles: true,
                },
                pacman: old.pacman,
                flatpak: old.flatpak,
                dotfiles: Vec::new(),
            }
        }
        Some(other) => return Err(SnapshotError::UnsupportedVersion(other)),
        None => return Err(SnapshotError::Json("`version` ausente ou inválida".into())),
    };
    validate(&snapshot)?;
    Ok(snapshot)
}

fn validate(snapshot: &Snapshot) -> Result<(), SnapshotError> {
    validate_name(&snapshot.name)?;
    validate_list("pacman", &snapshot.pacman, is_valid_pacman_name)?;
    validate_list("flatpak", &snapshot.flatpak, is_valid_flatpak_id)?;
    let mut seen = HashSet::new();
    for path in snapshot.dotfiles.iter().map(|d| &d.path) {
        if validate_path(path).is_err() {
            return Err(SnapshotError::InvalidItem {
                list: "dotfiles",
                name: path.clone(),
            });
        }
        if !seen.insert(path.as_str()) {
            return Err(SnapshotError::Duplicate {
                list: "dotfiles",
                name: path.clone(),
            });
        }
    }
    Ok(())
}

fn validate_list(
    list: &'static str,
    names: &[String],
    is_valid: fn(&str) -> bool,
) -> Result<(), SnapshotError> {
    let mut seen = HashSet::new();
    for name in names {
        if !is_valid(name) {
            return Err(SnapshotError::InvalidItem {
                list,
                name: name.clone(),
            });
        }
        if !seen.insert(name.as_str()) {
            return Err(SnapshotError::Duplicate {
                list,
                name: name.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Snapshot da máquina (ADR 0031, fatia 1): formato v1 que vai para o repositório git do dono.
    //! Só o que o `compare` da fatia 2 precisa (SO, capacidades, pacotes de propósito, apps
    //! Flatpak); sem hora, versão de pacote, hardware ou conteúdo de arquivo.
    use super::*;
    use crate::inventory::OsInfo;
    use crate::preset::{MachinePreset, Preset};
    use std::path::Path;

    fn os() -> OsInfo {
        OsInfo {
            name: "Arch Linux".into(),
            version: None,
        }
    }

    fn maquina(pacman: &[&str], flatpak: &[&str], flatpak_installed: bool) -> MachinePreset {
        MachinePreset {
            preset: Preset {
                pacman: pacman.iter().map(|s| s.to_string()).collect(),
                flatpak: flatpak.iter().map(|s| s.to_string()).collect(),
            },
            flatpak_installed,
        }
    }

    fn montar(nome: &str, pacman: &[&str], flatpak: &[&str]) -> Result<Snapshot, SnapshotError> {
        build(nome, &os(), &maquina(pacman, flatpak, true))
    }

    #[test]
    fn build_ordena_as_listas_e_a_mesma_coisa_em_outra_ordem_da_os_mesmos_bytes() {
        let a = montar("meu-pc", &["vim", "git", "base"], &["org.b.B", "org.a.A"]).unwrap();
        let b = montar("meu-pc", &["base", "vim", "git"], &["org.a.A", "org.b.B"]).unwrap();
        assert_eq!(a.pacman, ["base", "git", "vim"]);
        assert_eq!(a.flatpak, ["org.a.A", "org.b.B"]);
        // O `git diff` só pode mostrar mudança real.
        assert_eq!(to_json(&a), to_json(&b));
    }

    fn entrada(path: &str, state: DotfileState) -> DotfileEntry {
        DotfileEntry {
            path: path.into(),
            state,
        }
    }

    #[test]
    fn o_formato_v2_com_dotfiles_e_exatamente_este() {
        let s = montar("meu-pc", &["git", "base"], &["org.a.A"])
            .unwrap()
            .with_dotfiles(vec![
                entrada(".gitconfig", DotfileState::Missing),
                entrada(".bashrc", DotfileState::Linked),
            ])
            .unwrap();
        let esperado = r#"{
  "version": 2,
  "name": "meu-pc",
  "os": {
    "name": "Arch Linux",
    "version": null
  },
  "capabilities": {
    "pacman": true,
    "flatpak": true,
    "dotfiles": true
  },
  "pacman": [
    "base",
    "git"
  ],
  "flatpak": [
    "org.a.A"
  ],
  "dotfiles": [
    {
      "path": ".bashrc",
      "state": "linked"
    },
    {
      "path": ".gitconfig",
      "state": "missing"
    }
  ]
}
"#;
        assert_eq!(to_json(&s), esperado);
    }

    #[test]
    fn build_sozinho_nao_observou_dotfiles() {
        let s = montar("meu-pc", &["base"], &[]).unwrap();
        assert_eq!(s.version, 2);
        assert!(!s.capabilities.dotfiles);
        assert!(s.dotfiles.is_empty());
        assert_eq!(parse(&to_json(&s)).unwrap(), s);
    }

    #[test]
    fn with_dotfiles_liga_a_capacidade_e_ordena_por_caminho() {
        let s = montar("meu-pc", &["base"], &[])
            .unwrap()
            .with_dotfiles(vec![
                entrada(".vimrc", DotfileState::File),
                entrada(".bashrc", DotfileState::Linked),
                entrada(".config/git/config", DotfileState::Other),
            ])
            .unwrap();
        assert!(s.capabilities.dotfiles);
        let caminhos: Vec<&str> = s.dotfiles.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(caminhos, [".bashrc", ".config/git/config", ".vimrc"]);
        // A mesma lista em outra ordem dá os mesmos bytes (o `git diff` só mostra mudança real).
        let outra = montar("meu-pc", &["base"], &[])
            .unwrap()
            .with_dotfiles(vec![
                entrada(".config/git/config", DotfileState::Other),
                entrada(".vimrc", DotfileState::File),
                entrada(".bashrc", DotfileState::Linked),
            ])
            .unwrap();
        assert_eq!(to_json(&s), to_json(&outra));
    }

    #[test]
    fn lista_vazia_com_a_capacidade_e_diferente_de_sem_a_capacidade() {
        // Todos os caminhos em `dotfiles_skip`: a máquina olhou e não há nada a listar.
        let olhou = montar("meu-pc", &["base"], &[])
            .unwrap()
            .with_dotfiles(vec![])
            .unwrap();
        assert!(olhou.capabilities.dotfiles);
        assert!(olhou.dotfiles.is_empty());
        let nao_olhou = montar("meu-pc", &["base"], &[]).unwrap();
        assert_ne!(to_json(&olhou), to_json(&nao_olhou));
    }

    #[test]
    fn with_dotfiles_recusa_caminho_da_denylist_invalido_e_repetido() {
        for ruim in [
            ".ssh/id_rsa",
            ".gnupg/pubring.kbx",
            ".config/gh/hosts.yml",
            ".claude/settings.json",
            ".zsh_history",
            "../x",
            "/etc/passwd",
            "",
            ".bashrc/../.ssh/id_rsa",
        ] {
            let erro = montar("meu-pc", &["base"], &[])
                .unwrap()
                .with_dotfiles(vec![entrada(ruim, DotfileState::File)])
                .unwrap_err();
            assert_eq!(
                erro,
                SnapshotError::InvalidItem {
                    list: "dotfiles",
                    name: ruim.into()
                },
                "{ruim:?}"
            );
        }
        let erro = montar("meu-pc", &["base"], &[])
            .unwrap()
            .with_dotfiles(vec![
                entrada(".bashrc", DotfileState::File),
                entrada(".bashrc", DotfileState::Linked),
            ])
            .unwrap_err();
        assert_eq!(
            erro,
            SnapshotError::Duplicate {
                list: "dotfiles",
                name: ".bashrc".into()
            }
        );
    }

    const V1: &str = r#"{
  "version": 1,
  "name": "antigo",
  "os": {"name": "Arch Linux", "version": null},
  "capabilities": {"pacman": true, "flatpak": true},
  "pacman": ["base"],
  "flatpak": ["org.a.A"]
}"#;

    #[test]
    fn parse_le_o_formato_v1_como_sem_dotfiles() {
        let s = parse(V1).unwrap();
        assert_eq!(s.version, 1);
        assert_eq!(s.name, "antigo");
        assert_eq!(s.pacman, ["base"]);
        assert_eq!(s.flatpak, ["org.a.A"]);
        assert!(s.capabilities.pacman && s.capabilities.flatpak);
        assert!(!s.capabilities.dotfiles, "o v1 não observou dotfiles");
        assert!(s.dotfiles.is_empty());
    }

    #[test]
    fn o_v1_continua_estrito() {
        // `dotfiles` no v1 é campo desconhecido, e capacidade nova também: não se adivinha.
        let com_lista = V1.replacen(
            "\"flatpak\": [\"org.a.A\"]",
            "\"flatpak\": [\"org.a.A\"], \"dotfiles\": []",
            1,
        );
        assert!(matches!(parse(&com_lista), Err(SnapshotError::Json(_))));
        let com_capacidade = V1.replacen(
            "\"flatpak\": true",
            "\"flatpak\": true, \"dotfiles\": true",
            1,
        );
        assert!(matches!(
            parse(&com_capacidade),
            Err(SnapshotError::Json(_))
        ));
        let sem_os = V1.replacen("\"os\"", "\"os_\"", 1);
        assert!(parse(&sem_os).is_err());
    }

    #[test]
    fn parse_v2_devolve_o_que_to_json_escreveu_com_dotfiles() {
        let s = montar("meu-pc", &["base"], &["org.a.A"])
            .unwrap()
            .with_dotfiles(vec![
                entrada(".bashrc", DotfileState::Linked),
                entrada(".gitconfig", DotfileState::File),
                entrada(".vimrc", DotfileState::Missing),
                entrada(".zshrc", DotfileState::Other),
            ])
            .unwrap();
        assert_eq!(parse(&to_json(&s)).unwrap(), s);
    }

    fn v2_com(dotfiles: &str) -> String {
        format!(
            "{{\"version\": 2, \"name\": \"meu-pc\", \"os\": {{\"name\": \"Arch Linux\", \"version\": null}}, \
             \"capabilities\": {{\"pacman\": true, \"flatpak\": true, \"dotfiles\": true}}, \
             \"pacman\": [], \"flatpak\": [], \"dotfiles\": {dotfiles}}}"
        )
    }

    #[test]
    fn parse_v2_revalida_os_dotfiles_do_arquivo_de_outra_maquina() {
        for ruim in [
            ".ssh/id_rsa",
            "../x",
            "/etc/passwd",
            ".config/gh/hosts.yml",
            "",
        ] {
            let texto = v2_com(&format!("[{{\"path\": {ruim:?}, \"state\": \"file\"}}]"));
            assert!(
                matches!(
                    parse(&texto),
                    Err(SnapshotError::InvalidItem {
                        list: "dotfiles",
                        ..
                    })
                ),
                "{ruim:?}"
            );
        }
        let repetido = v2_com(
            "[{\"path\": \".bashrc\", \"state\": \"file\"}, {\"path\": \".bashrc\", \"state\": \"linked\"}]",
        );
        assert!(matches!(
            parse(&repetido),
            Err(SnapshotError::Duplicate {
                list: "dotfiles",
                ..
            })
        ));
        for ruim in [
            "[{\"path\": \".bashrc\", \"state\": \"symlink\"}]",
            "[{\"path\": \".bashrc\", \"state\": \"Linked\"}]",
            "[{\"path\": \".bashrc\", \"state\": \"file\", \"size\": 10}]",
            "[{\"path\": \".bashrc\", \"state\": \"linked\", \"target\": \"/home/x/dotfiles/.bashrc\"}]",
            "[{\"path\": \".bashrc\"}]",
            "[{\"state\": \"file\"}]",
            "[\".bashrc\"]",
            "null",
        ] {
            assert!(
                matches!(parse(&v2_com(ruim)), Err(SnapshotError::Json(_))),
                "{ruim}"
            );
        }
    }

    #[test]
    fn v2_sem_a_capacidade_ou_sem_a_lista_e_erro() {
        let sem_capacidade = v2_com("[]").replacen(", \"dotfiles\": true", "", 1);
        assert!(matches!(
            parse(&sem_capacidade),
            Err(SnapshotError::Json(_))
        ));
        let sem_lista = v2_com("[]").replacen(", \"dotfiles\": []", "", 1);
        assert!(matches!(parse(&sem_lista), Err(SnapshotError::Json(_))));
    }

    #[test]
    fn dotfile_state_olha_so_o_texto_do_alvo_e_nunca_o_tamanho() {
        use crate::dotfiles::Observed;
        let dir = Path::new("/home/u/dotfiles");
        let ligado = |path: &str, alvo: &str| {
            dotfile_state(
                dir,
                path,
                &Observed::Symlink {
                    target: alvo.into(),
                },
            )
        };
        assert_eq!(
            dotfile_state(dir, ".bashrc", &Observed::Missing),
            DotfileState::Missing
        );
        assert_eq!(
            dotfile_state(dir, ".bashrc", &Observed::File { size: 0 }),
            DotfileState::File
        );
        assert_eq!(
            dotfile_state(dir, ".bashrc", &Observed::File { size: 1 << 30 }),
            DotfileState::File
        );
        assert_eq!(
            dotfile_state(dir, ".bashrc", &Observed::Dir),
            DotfileState::Other
        );
        assert_eq!(
            dotfile_state(dir, ".bashrc", &Observed::Other),
            DotfileState::Other
        );
        assert_eq!(
            ligado(".bashrc", "/home/u/dotfiles/.bashrc"),
            DotfileState::Linked
        );
        assert_eq!(
            ligado(".config/git/config", "/home/u/dotfiles/.config/git/config"),
            DotfileState::Linked
        );
        // Texto idêntico, como no `undo` (ADR 0025): barra final, relativo e outro arquivo não valem.
        for (path, alvo) in [
            (".bashrc", "/home/u/dotfiles/.bashrc/"),
            (".bashrc", "../dotfiles/.bashrc"),
            (".bashrc", "/home/u/dotfiles/.zshrc"),
            (".bashrc", "/home/u/outra/.bashrc"),
            (".bashrc", "/home/u/dotfiles/sub/.bashrc"),
            (".bashrc", "/home/u/dotfiles//.bashrc"),
            (".bashrc", ""),
            (".config/git/config", "/home/u/dotfiles/.gitconfig"),
        ] {
            assert_eq!(ligado(path, alvo), DotfileState::Other, "{path} -> {alvo}");
        }
    }

    #[test]
    fn o_json_nao_tem_alvo_tamanho_nem_o_caminho_do_home() {
        use crate::dotfiles::Observed;
        let dir = Path::new("/home/maria/dotfiles");
        let estados = [
            (
                ".bashrc",
                Observed::Symlink {
                    target: "/home/maria/dotfiles/.bashrc".into(),
                },
            ),
            (".gitconfig", Observed::File { size: 4242 }),
            (".vimrc", Observed::Missing),
            (
                ".zshrc",
                Observed::Symlink {
                    target: "/home/maria/outra/.zshrc".into(),
                },
            ),
        ];
        let entradas = estados
            .iter()
            .map(|(path, obs)| entrada(path, dotfile_state(dir, path, obs)))
            .collect();
        let s = montar("meu-pc", &["base"], &[])
            .unwrap()
            .with_dotfiles(entradas)
            .unwrap();
        let json = to_json(&s);
        for proibido in ["/home", "maria", "4242", "size", "target", "outra"] {
            assert!(!json.contains(proibido), "{proibido} vazou:\n{json}");
        }
    }

    #[test]
    fn sem_flatpak_instalado_a_capacidade_e_falsa_e_a_lista_vazia() {
        let s = build("meu-pc", &os(), &maquina(&["base"], &[], false)).unwrap();
        assert!(s.capabilities.pacman);
        assert!(!s.capabilities.flatpak);
        assert!(s.flatpak.is_empty());
        // Flatpak instalado e sem apps é diferente de Flatpak ausente.
        let com = build("meu-pc", &os(), &maquina(&["base"], &[], true)).unwrap();
        assert!(com.capabilities.flatpak);
    }

    #[test]
    fn o_snapshot_nao_tem_hora_nem_versao_de_pacote_nem_hardware() {
        let json = to_json(&montar("meu-pc", &["base"], &[]).unwrap());
        let proibidos = [
            "at_unix", "time", "hardware", "cpu", "memory", "disk", "kernel", "hostname",
        ];
        for proibido in proibidos {
            assert!(!json.contains(proibido), "{proibido}");
        }
    }

    #[test]
    fn rotulo_valido_e_so_minusculas_digitos_ponto_traco_e_sublinhado() {
        let longo_ok = format!("a{}", "b".repeat(62));
        let validos = [
            "meu-pc",
            "a",
            "0",
            "notebook.2",
            "a_b-c.d",
            longo_ok.as_str(),
        ];
        for ok in validos {
            assert!(validate_name(ok).is_ok(), "{ok:?}");
        }
    }

    #[test]
    fn rotulo_que_escaparia_do_diretorio_ou_quebraria_o_nome_do_arquivo_e_recusado() {
        let longo_ruim = "a".repeat(64);
        let ruins = [
            "",
            "-x",
            ".x",
            "_x",
            "..",
            ".",
            "a/b",
            "../x",
            "/etc/passwd",
            "A",
            "Meu-PC",
            "a b",
            "a\0b",
            "a\nb",
            "é",
            "a:b",
            "a\\b",
            "a;b",
            "~",
            longo_ruim.as_str(),
        ];
        for ruim in ruins {
            let erro = validate_name(ruim);
            assert!(
                matches!(erro, Err(SnapshotError::InvalidName(_))),
                "{ruim:?}"
            );
        }
    }

    #[test]
    fn build_recusa_rotulo_invalido_e_nome_de_pacote_ou_app_invalido() {
        let erro = montar("../x", &["base"], &[]);
        assert!(matches!(erro, Err(SnapshotError::InvalidName(_))));
        // A máquina já valida, mas o `build` não confia: um `-Rns` no arquivo seria lido como
        // opção por quem usa a lista.
        let erro = montar("meu-pc", &["-Rns"], &[]);
        assert!(matches!(erro, Err(SnapshotError::InvalidItem { .. })));
        let erro = montar("meu-pc", &[], &["não é id"]);
        assert!(matches!(erro, Err(SnapshotError::InvalidItem { .. })));
    }

    #[test]
    fn build_recusa_item_duplicado() {
        let erro = montar("meu-pc", &["git", "git"], &[]);
        assert!(matches!(erro, Err(SnapshotError::Duplicate { .. })));
    }

    #[test]
    fn parse_devolve_o_que_to_json_escreveu() {
        let s = montar("meu-pc", &["git", "base"], &["org.a.A"]).unwrap();
        assert_eq!(parse(&to_json(&s)).unwrap(), s);
    }

    fn valido() -> String {
        to_json(&montar("meu-pc", &["base"], &["org.a.A"]).unwrap())
    }

    #[test]
    fn parse_recusa_versao_desconhecida_sem_adivinhar() {
        for v in ["0", "3", "99", "-1", "1.5", "\"2\"", "null"] {
            let texto = valido().replacen("\"version\": 2", &format!("\"version\": {v}"), 1);
            assert!(parse(&texto).is_err(), "versão {v}");
        }
        let texto = valido().replacen("\"version\": 2", "\"version\": 3", 1);
        let erro = parse(&texto).unwrap_err();
        assert_eq!(erro, SnapshotError::UnsupportedVersion(3));
        assert!(
            erro.to_string().contains("1 e a 2"),
            "a mensagem diz o que este nodus entende: {erro}"
        );
    }

    #[test]
    fn parse_recusa_campo_desconhecido_ausente_e_lixo() {
        let com_extra = valido().replacen("\"name\"", "\"extra\": 1,\n  \"name\"", 1);
        assert!(matches!(parse(&com_extra), Err(SnapshotError::Json(_))));
        let sem_os = valido().replacen("\"os\"", "\"os_\"", 1);
        assert!(parse(&sem_os).is_err());
        let nix = valido().replacen(
            "\"pacman\": true",
            "\"pacman\": true,\n    \"nix\": true",
            1,
        );
        assert!(parse(&nix).is_err(), "capacidade desconhecida");
        for lixo in ["", "não é json", "[]", "{}", "null", "{\"version\":1}"] {
            assert!(parse(lixo).is_err(), "{lixo:?}");
        }
    }

    #[test]
    fn parse_revalida_rotulo_e_itens_do_arquivo_de_outra_maquina() {
        // O arquivo vem de outra máquina (e pode ser editado à mão): a leitura valida como a escrita.
        let erro = parse(&valido().replacen("meu-pc", "../x", 1));
        assert!(matches!(erro, Err(SnapshotError::InvalidName(_))));
        let erro = parse(&valido().replacen("\"base\"", "\"-Rns\"", 1));
        assert!(matches!(erro, Err(SnapshotError::InvalidItem { .. })));
        let erro = parse(&valido().replacen("\"org.a.A\"", "\"x y\"", 1));
        assert!(matches!(erro, Err(SnapshotError::InvalidItem { .. })));
    }
}
