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
pub struct Disk {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_bytes: u64,
}

/// Hardware lido por um adaptador (o core não toca a máquina).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HardwareInfo {
    pub cpu_model: String,
    pub cpu_threads: usize,
    pub memory_total_bytes: u64,
    pub disks: Vec<Disk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlatpakApp {
    pub application: String,
    pub version: Option<String>,
    pub branch: String,
    pub origin: String,
    pub installation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inventory {
    pub system: SystemInfo,
    pub hardware: HardwareInfo,
    pub packages: Vec<Package>,
    /// `None`: Flatpak não instalado. `Some(vec![])`: instalado, sem apps.
    pub flatpak_apps: Option<Vec<FlatpakApp>>,
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
            number: index + 1,
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

/// Colunas pedidas ao Flatpak (ADR 0008): o parser depende desta ordem.
const FLATPAK_COLUMNS: &str = "--columns=application,version,branch,origin,installation";

/// Interpreta `flatpak list --app` com as colunas de `FLATPAK_COLUMNS`, separadas por tabulação.
/// Campo vazio é dado (a versão pode faltar), por isso não se descartam campos vazios ao dividir.
pub fn parse_flatpak_list(output: &str) -> Result<Vec<FlatpakApp>, ParseError> {
    let mut apps = Vec::new();
    for (index, line) in output.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let invalid = || ParseError::InvalidLine {
            number: index + 1,
            content: line.to_string(),
        };
        if line.chars().any(|c| c.is_control() && c != '\t') {
            return Err(invalid());
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [application, version, branch, origin, installation] = fields[..] else {
            return Err(invalid());
        };
        if application.is_empty() || branch.is_empty() || installation.is_empty() {
            return Err(invalid());
        }
        apps.push(FlatpakApp {
            application: application.to_string(),
            version: Some(version.to_string()),
            branch: branch.to_string(),
            origin: origin.to_string(),
            installation: installation.to_string(),
        });
    }
    Ok(apps)
}

/// Monta o inventário. O conteúdo de `os-release` e do kernel vem de quem chama (o core não lê arquivos).
pub fn collect(
    runner: &dyn CommandRunner,
    os_release: &str,
    kernel: &str,
    hardware: HardwareInfo,
) -> Result<Inventory, InventoryError> {
    let os = parse_os_release(os_release)?;
    let packages = parse_pacman_q(&runner.run("pacman", &["-Q"])?)?;
    // Flatpak é opcional: só "programa não encontrado" vira `None`; outras falhas continuam erro.
    let flatpak_apps = match runner.run("flatpak", &["list", "--app", FLATPAK_COLUMNS]) {
        Ok(output) => Some(parse_flatpak_list(&output)?),
        Err(RunnerError::NotFound { .. }) => None,
        Err(erro) => return Err(erro.into()),
    };
    Ok(Inventory {
        system: SystemInfo {
            os_name: os.name,
            os_version: os.version,
            kernel: kernel.trim().to_string(),
        },
        hardware,
        packages,
        flatpak_apps,
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

    const FLATPAK_APP: &str = include_str!("../tests/fixtures/flatpak_list_app.tsv");
    const FLATPAK_COLUNAS: &str = "--columns=application,version,branch,origin,installation";

    fn flatpak_ausente() -> RunnerError {
        RunnerError::NotFound {
            program: "flatpak".into(),
        }
    }

    /// Runner falso: responde por programa e guarda como foi chamado. Sem Flatpak por padrão.
    struct Falso {
        pacman: Result<String, RunnerError>,
        flatpak: Result<String, RunnerError>,
        chamadas: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Falso {
        fn ok(saida: &str) -> Self {
            Self {
                pacman: Ok(saida.to_string()),
                flatpak: Err(flatpak_ausente()),
                chamadas: RefCell::new(vec![]),
            }
        }
        fn erro(e: RunnerError) -> Self {
            Self {
                pacman: Err(e),
                flatpak: Err(flatpak_ausente()),
                chamadas: RefCell::new(vec![]),
            }
        }
        fn com_flatpak(mut self, resposta: Result<String, RunnerError>) -> Self {
            self.flatpak = resposta;
            self
        }
    }

    impl CommandRunner for Falso {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError> {
            self.chamadas.borrow_mut().push((
                program.to_string(),
                args.iter().map(|a| a.to_string()).collect(),
            ));
            match program {
                "pacman" => self.pacman.clone(),
                "flatpak" => self.flatpak.clone(),
                outro => panic!("programa inesperado no teste: {outro}"),
            }
        }
    }

    fn app(application: &str, version: Option<&str>) -> FlatpakApp {
        FlatpakApp {
            application: application.into(),
            version: version.map(String::from),
            branch: "stable".into(),
            origin: "flathub".into(),
            installation: "user".into(),
        }
    }

    fn hardware() -> HardwareInfo {
        HardwareInfo {
            cpu_model: "AMD Ryzen 5 5600".into(),
            cpu_threads: 12,
            memory_total_bytes: 33_554_432_000,
            disks: vec![Disk {
                name: "/dev/sda3".into(),
                mount_point: "/home".into(),
                file_system: "ext4".into(),
                total_bytes: 200_000_000_000,
            }],
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
    fn collect_chama_pacman_e_flatpak_com_lista_de_argumentos() {
        let runner = Falso::ok(PACMAN_Q).com_flatpak(Ok(FLATPAK_APP.to_string()));
        collect(&runner, OS_RELEASE_ARCH, "6.12.1-arch1-1\n", hardware()).unwrap();
        assert_eq!(
            *runner.chamadas.borrow(),
            vec![
                ("pacman".to_string(), vec!["-Q".to_string()]),
                (
                    "flatpak".to_string(),
                    vec![
                        "list".to_string(),
                        "--app".to_string(),
                        FLATPAK_COLUNAS.to_string()
                    ]
                ),
            ]
        );
    }

    #[test]
    fn collect_com_flatpak_lista_os_apps() {
        let runner = Falso::ok("bash 5.2.037-2\n").com_flatpak(Ok(FLATPAK_APP.to_string()));
        let inv = collect(&runner, OS_RELEASE_ARCH, "6.12.1", hardware()).unwrap();
        assert_eq!(
            inv.flatpak_apps,
            Some(vec![app("org.nongnu.lingot", Some("1.1.1"))])
        );
    }

    #[test]
    fn collect_flatpak_sem_apps_e_lista_vazia() {
        let runner = Falso::ok("bash 5.2.037-2\n").com_flatpak(Ok(String::new()));
        let inv = collect(&runner, OS_RELEASE_ARCH, "6.12.1", hardware()).unwrap();
        assert_eq!(inv.flatpak_apps, Some(vec![]));
    }

    #[test]
    fn collect_flatpak_nao_instalado_e_none_e_nao_e_erro() {
        let runner = Falso::ok("bash 5.2.037-2\n");
        let inv = collect(&runner, OS_RELEASE_ARCH, "6.12.1", hardware()).unwrap();
        assert_eq!(inv.flatpak_apps, None);
    }

    #[test]
    fn collect_flatpak_com_falha_real_e_erro() {
        let falha = RunnerError::Failed {
            program: "flatpak".into(),
            code: Some(1),
        };
        let runner = Falso::ok("bash 5.2.037-2\n").com_flatpak(Err(falha.clone()));
        let erro = collect(&runner, OS_RELEASE_ARCH, "6.12.1", hardware()).unwrap_err();
        assert_eq!(erro, InventoryError::Runner(falha));
    }

    #[test]
    fn collect_flatpak_sem_permissao_nao_vira_nao_instalado() {
        let falha = RunnerError::Spawn {
            program: "flatpak".into(),
            reason: "permission denied".into(),
        };
        let runner = Falso::ok("bash 5.2.037-2\n").com_flatpak(Err(falha.clone()));
        let erro = collect(&runner, OS_RELEASE_ARCH, "6.12.1", hardware()).unwrap_err();
        assert_eq!(erro, InventoryError::Runner(falha));
    }

    #[test]
    fn collect_propaga_erro_de_parsing_do_flatpak() {
        let runner = Falso::ok("bash 5.2.037-2\n").com_flatpak(Ok("so-uma-coluna\n".to_string()));
        let erro = collect(&runner, OS_RELEASE_ARCH, "6.12.1", hardware()).unwrap_err();
        assert!(matches!(
            erro,
            InventoryError::Parse(ParseError::InvalidLine { number: 1, .. })
        ));
    }

    #[test]
    fn collect_pacman_ausente_continua_sendo_erro() {
        let ausente = RunnerError::NotFound {
            program: "pacman".into(),
        };
        let runner = Falso::erro(ausente.clone());
        let erro = collect(&runner, OS_RELEASE_ARCH, "6.12.1", hardware()).unwrap_err();
        assert_eq!(erro, InventoryError::Runner(ausente));
    }

    #[test]
    fn collect_monta_o_inventario_e_tira_a_quebra_de_linha_do_kernel() {
        let runner = Falso::ok("bash 5.2.037-2\n");
        let inv = collect(&runner, OS_RELEASE_ARCH, "6.12.1-arch1-1\n", hardware()).unwrap();
        assert_eq!(
            inv,
            Inventory {
                system: SystemInfo {
                    os_name: "Arch Linux".into(),
                    os_version: None,
                    kernel: "6.12.1-arch1-1".into(),
                },
                hardware: hardware(),
                packages: vec![pacote("bash", "5.2.037-2")],
                flatpak_apps: None,
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
        let erro = collect(&runner, OS_RELEASE_ARCH, "6.12.1", hardware()).unwrap_err();
        assert_eq!(erro, InventoryError::Runner(falha));
    }

    #[test]
    fn collect_propaga_erro_de_parsing_do_pacman() {
        let runner = Falso::ok("linux\n");
        let erro = collect(&runner, OS_RELEASE_ARCH, "6.12.1", hardware()).unwrap_err();
        assert!(matches!(
            erro,
            InventoryError::Parse(ParseError::InvalidLine { number: 1, .. })
        ));
    }

    #[test]
    fn collect_propaga_os_release_invalido() {
        let runner = Falso::ok("");
        let erro = collect(&runner, "ID=arch\n", "6.12.1", hardware()).unwrap_err();
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
            hardware: hardware(),
            packages: vec![pacote("bash", "5.2.037-2")],
            flatpak_apps: Some(vec![app("org.nongnu.lingot", Some("1.1.1")), {
                let mut sem_versao = app("org.example.SemVersao", None);
                sem_versao.installation = "system".into();
                sem_versao
            }]),
        };
        let json = serde_json::to_value(&inv).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "system": { "os_name": "Arch Linux", "os_version": null, "kernel": "6.12.1-arch1-1" },
                "hardware": {
                    "cpu_model": "AMD Ryzen 5 5600",
                    "cpu_threads": 12,
                    "memory_total_bytes": 33554432000u64,
                    "disks": [
                        { "name": "/dev/sda3", "mount_point": "/home", "file_system": "ext4", "total_bytes": 200000000000u64 }
                    ]
                },
                "packages": [ { "name": "bash", "version": "5.2.037-2" } ],
                "flatpak_apps": [
                    { "application": "org.nongnu.lingot", "version": "1.1.1", "branch": "stable", "origin": "flathub", "installation": "user" },
                    { "application": "org.example.SemVersao", "version": null, "branch": "stable", "origin": "flathub", "installation": "system" }
                ]
            })
        );
    }

    #[test]
    fn json_flatpak_nao_instalado_e_null_e_sem_apps_e_lista_vazia() {
        let mut inv = Inventory {
            system: SystemInfo {
                os_name: "Arch Linux".into(),
                os_version: None,
                kernel: "6.12.1-arch1-1".into(),
            },
            hardware: hardware(),
            packages: vec![],
            flatpak_apps: None,
        };
        let nao_instalado = serde_json::to_value(&inv).unwrap();
        assert_eq!(nao_instalado["flatpak_apps"], serde_json::Value::Null);
        inv.flatpak_apps = Some(vec![]);
        let sem_apps = serde_json::to_value(&inv).unwrap();
        assert_eq!(sem_apps["flatpak_apps"], serde_json::json!([]));
    }

    // ---- flatpak list ----

    #[test]
    fn flatpak_list_da_amostra_real() {
        let apps = parse_flatpak_list(FLATPAK_APP).unwrap();
        assert_eq!(apps, vec![app("org.nongnu.lingot", Some("1.1.1"))]);
    }

    #[test]
    fn flatpak_list_vazio_e_lista_vazia() {
        assert_eq!(parse_flatpak_list("").unwrap(), vec![]);
    }

    #[test]
    fn flatpak_list_versao_vazia_vira_none_sem_deslocar_as_colunas() {
        // Mesmo formato do campo vazio medido nos runtimes (ADR 0008): `id\t\tramo\torigem\tinstalação`.
        let apps = parse_flatpak_list("org.example.App\t\tstable\tflathub\tsystem\n").unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].version, None);
        assert_eq!(apps[0].branch, "stable");
        assert_eq!(apps[0].origin, "flathub");
        assert_eq!(apps[0].installation, "system");
    }

    #[test]
    fn flatpak_list_ignora_linhas_em_branco_e_aceita_crlf() {
        let entrada =
            "org.a.A\t1\tstable\tflathub\tuser\r\n\r\norg.b.B\t2\tstable\tflathub\tsystem\r\n";
        let apps = parse_flatpak_list(entrada).unwrap();
        let ids: Vec<&str> = apps.iter().map(|a| a.application.as_str()).collect();
        assert_eq!(ids, vec!["org.a.A", "org.b.B"]);
    }

    #[test]
    fn flatpak_list_com_quatro_colunas_e_erro() {
        let erro = parse_flatpak_list("org.a.A\t1\tstable\tflathub\n").unwrap_err();
        assert_eq!(
            erro,
            ParseError::InvalidLine {
                number: 1,
                content: "org.a.A\t1\tstable\tflathub".into()
            }
        );
    }

    #[test]
    fn flatpak_list_com_seis_colunas_e_erro() {
        let erro = parse_flatpak_list("org.a.A\t1\tstable\tflathub\tuser\textra\n").unwrap_err();
        assert!(matches!(erro, ParseError::InvalidLine { number: 1, .. }));
    }

    #[test]
    fn flatpak_list_com_id_ou_ramo_vazio_e_erro() {
        assert!(parse_flatpak_list("\t1\tstable\tflathub\tuser\n").is_err());
        assert!(parse_flatpak_list("org.a.A\t1\t\tflathub\tuser\n").is_err());
    }

    #[test]
    fn flatpak_list_erro_informa_o_numero_da_linha() {
        let erro = parse_flatpak_list("org.a.A\t1\tstable\tflathub\tuser\nlixo\n").unwrap_err();
        assert!(matches!(erro, ParseError::InvalidLine { number: 2, .. }));
    }

    #[test]
    fn flatpak_list_com_caractere_de_controle_e_erro_sem_panic() {
        let erro = parse_flatpak_list("org.a.\u{1b}A\t1\tstable\tflathub\tuser\n").unwrap_err();
        assert!(matches!(erro, ParseError::InvalidLine { number: 1, .. }));
    }
}
