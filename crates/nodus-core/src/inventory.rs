//! Inventário do sistema e dos pacotes instalados (somente leitura).

use crate::runner::{CommandRunner, RunnerError};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Package {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: Option<String>,
    pub kernel: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inventory {
    pub system: SystemInfo,
    pub packages: Vec<Package>,
}

/// Campos que usamos do `/etc/os-release`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsInfo {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("campo obrigatório ausente: {0}")]
    MissingField(&'static str),
    #[error("linha {number} inválida: {content:?}")]
    InvalidLine { number: usize, content: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InventoryError {
    #[error(transparent)]
    Runner(#[from] RunnerError),
    #[error(transparent)]
    Parse(#[from] ParseError),
}

/// Lê `NAME` e `VERSION_ID` do conteúdo de um `os-release`. Valores entre aspas duplas ou sem aspas.
pub fn parse_os_release(content: &str) -> Result<OsInfo, ParseError> {
    let mut name = None;
    let mut version = None;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim_matches('"').to_string();
        match key {
            "NAME" => name = Some(value),
            "VERSION_ID" => version = Some(value),
            _ => {}
        }
    }
    Ok(OsInfo {
        name: name.ok_or(ParseError::MissingField("NAME"))?,
        version,
    })
}

/// Interpreta a saída de `pacman -Q`: uma linha `nome versão` por pacote.
pub fn parse_pacman_q(output: &str) -> Result<Vec<Package>, ParseError> {
    let mut packages = Vec::new();
    for (index, line) in output.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let invalid = || ParseError::InvalidLine {
            number: index,
            content: line.trim().to_string(),
        };
        if line.chars().any(char::is_control) {
            return Err(invalid());
        }
        let mut fields = line.split_whitespace();
        match (fields.next(), fields.next(), fields.next()) {
            (Some(name), Some(version), None) => packages.push(Package {
                name: name.to_string(),
                version: version.to_string(),
            }),
            _ => return Err(invalid()),
        }
    }
    Ok(packages)
}

/// Monta o inventário. O conteúdo de `os-release` e do kernel vem de quem chama (o core não lê arquivos).
pub fn collect(
    runner: &dyn CommandRunner,
    os_release: &str,
    kernel: &str,
) -> Result<Inventory, InventoryError> {
    let os = parse_os_release(os_release)?;
    let packages = parse_pacman_q(&runner.run("pacman", &["-Q"])?)?;
    Ok(Inventory {
        system: SystemInfo {
            os_name: os.name,
            os_version: os.version,
            kernel: kernel.trim().to_string(),
        },
        packages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandRunner, RunnerError};
    use std::cell::RefCell;

    const OS_RELEASE_ARCH: &str = "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\nBUILD_ID=rolling\nANSI_COLOR=\"38;2;23;147;209\"\nHOME_URL=\"https://archlinux.org/\"\n";
    const OS_RELEASE_FEDORA: &str =
        "# comentário\nNAME=\"Fedora Linux\"\n\nVERSION_ID=41\nID=fedora\n";
    const PACMAN_Q: &str =
        "bash 5.2.037-2\nlinux 6.12.1.arch1-1\nlib32-glibc 2.40+r16+gaa533d58ff-2\n";

    /// Runner falso: devolve uma resposta fixa e guarda como foi chamado.
    struct Falso {
        resposta: Result<String, RunnerError>,
        chamadas: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Falso {
        fn ok(saida: &str) -> Self {
            Self {
                resposta: Ok(saida.to_string()),
                chamadas: RefCell::new(vec![]),
            }
        }
        fn erro(e: RunnerError) -> Self {
            Self {
                resposta: Err(e),
                chamadas: RefCell::new(vec![]),
            }
        }
    }

    impl CommandRunner for Falso {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError> {
            self.chamadas.borrow_mut().push((
                program.to_string(),
                args.iter().map(|a| a.to_string()).collect(),
            ));
            self.resposta.clone()
        }
    }

    fn pacote(nome: &str, versao: &str) -> Package {
        Package {
            name: nome.to_string(),
            version: versao.to_string(),
        }
    }

    // ---- os-release ----

    #[test]
    fn os_release_do_arch_nao_tem_versao() {
        let os = parse_os_release(OS_RELEASE_ARCH).unwrap();
        assert_eq!(
            os,
            OsInfo {
                name: "Arch Linux".into(),
                version: None
            }
        );
    }

    #[test]
    fn os_release_ignora_comentario_e_linha_vazia_e_le_versao() {
        let os = parse_os_release(OS_RELEASE_FEDORA).unwrap();
        assert_eq!(
            os,
            OsInfo {
                name: "Fedora Linux".into(),
                version: Some("41".into())
            }
        );
    }

    #[test]
    fn os_release_sem_name_e_erro() {
        let erro = parse_os_release("ID=arch\n").unwrap_err();
        assert_eq!(erro, ParseError::MissingField("NAME"));
    }

    #[test]
    fn os_release_vazio_e_erro() {
        assert_eq!(
            parse_os_release("").unwrap_err(),
            ParseError::MissingField("NAME")
        );
    }

    // ---- pacman -Q ----

    #[test]
    fn pacman_q_vira_lista_de_pacotes() {
        let pacotes = parse_pacman_q(PACMAN_Q).unwrap();
        assert_eq!(
            pacotes,
            vec![
                pacote("bash", "5.2.037-2"),
                pacote("linux", "6.12.1.arch1-1"),
                pacote("lib32-glibc", "2.40+r16+gaa533d58ff-2"),
            ]
        );
    }

    #[test]
    fn pacman_q_vazio_e_lista_vazia() {
        assert_eq!(parse_pacman_q("").unwrap(), vec![]);
    }

    #[test]
    fn pacman_q_ignora_linhas_em_branco_e_aceita_crlf() {
        let pacotes = parse_pacman_q("bash 5.2.037-2\r\n\r\nlinux 6.12.1.arch1-1\r\n").unwrap();
        assert_eq!(
            pacotes,
            vec![
                pacote("bash", "5.2.037-2"),
                pacote("linux", "6.12.1.arch1-1")
            ]
        );
    }

    #[test]
    fn pacman_q_linha_sem_versao_e_erro_com_numero_da_linha() {
        let erro = parse_pacman_q("bash 5.2.037-2\nlinux\n").unwrap_err();
        assert_eq!(
            erro,
            ParseError::InvalidLine {
                number: 2,
                content: "linux".into()
            }
        );
    }

    #[test]
    fn pacman_q_linha_com_campo_a_mais_e_erro() {
        let erro = parse_pacman_q("bash 5.2.037-2 extra\n").unwrap_err();
        assert_eq!(
            erro,
            ParseError::InvalidLine {
                number: 1,
                content: "bash 5.2.037-2 extra".into()
            }
        );
    }

    #[test]
    fn pacman_q_lixo_binario_nao_causa_panic() {
        let entrada = "\u{0}\u{1}\u{7f} \u{fffd}\n\u{0}\n";
        assert!(parse_pacman_q(entrada).is_err());
    }

    // ---- collect ----

    #[test]
    fn collect_chama_pacman_q_com_lista_de_argumentos() {
        let runner = Falso::ok(PACMAN_Q);
        collect(&runner, OS_RELEASE_ARCH, "6.12.1-arch1-1\n").unwrap();
        assert_eq!(
            *runner.chamadas.borrow(),
            vec![("pacman".to_string(), vec!["-Q".to_string()])]
        );
    }

    #[test]
    fn collect_monta_o_inventario_e_tira_a_quebra_de_linha_do_kernel() {
        let runner = Falso::ok("bash 5.2.037-2\n");
        let inv = collect(&runner, OS_RELEASE_ARCH, "6.12.1-arch1-1\n").unwrap();
        assert_eq!(
            inv,
            Inventory {
                system: SystemInfo {
                    os_name: "Arch Linux".into(),
                    os_version: None,
                    kernel: "6.12.1-arch1-1".into(),
                },
                packages: vec![pacote("bash", "5.2.037-2")],
            }
        );
    }

    #[test]
    fn collect_propaga_falha_do_runner() {
        let falha = RunnerError::Failed {
            program: "pacman".into(),
            code: Some(1),
        };
        let runner = Falso::erro(falha.clone());
        let erro = collect(&runner, OS_RELEASE_ARCH, "6.12.1").unwrap_err();
        assert_eq!(erro, InventoryError::Runner(falha));
    }

    #[test]
    fn collect_propaga_erro_de_parsing_do_pacman() {
        let runner = Falso::ok("linux\n");
        let erro = collect(&runner, OS_RELEASE_ARCH, "6.12.1").unwrap_err();
        assert!(matches!(
            erro,
            InventoryError::Parse(ParseError::InvalidLine { number: 1, .. })
        ));
    }

    #[test]
    fn collect_propaga_os_release_invalido() {
        let runner = Falso::ok("");
        let erro = collect(&runner, "ID=arch\n", "6.12.1").unwrap_err();
        assert_eq!(
            erro,
            InventoryError::Parse(ParseError::MissingField("NAME"))
        );
    }

    // ---- contrato JSON ----

    #[test]
    fn json_tem_os_campos_do_contrato() {
        let inv = Inventory {
            system: SystemInfo {
                os_name: "Arch Linux".into(),
                os_version: None,
                kernel: "6.12.1-arch1-1".into(),
            },
            packages: vec![pacote("bash", "5.2.037-2")],
        };
        let json = serde_json::to_value(&inv).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "system": { "os_name": "Arch Linux", "os_version": null, "kernel": "6.12.1-arch1-1" },
                "packages": [ { "name": "bash", "version": "5.2.037-2" } ]
            })
        );
    }
}
