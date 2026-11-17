//! Ponte entre a GUI e o `nodusctl` (ADR 0026).
//!
//! A fronteira de confiança é aqui, no Rust, e não no JavaScript: a webview é código de página web. O frontend só
//! manda o **nome** de um comando; este módulo recusa o que não está na lista, monta os argumentos sozinho, sem shell,
//! e só roda depois de conferir que o `nodusctl --version` tem o mesmo `major.minor` da GUI.

use serde::Serialize;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Limite da saída aceita de um comando (16 MiB). O `inventory` de uma máquina com milhares de pacotes fica bem abaixo.
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

/// Erro devolvido ao frontend: `kind` é um código estável (a tela decide o que mostrar), `message` é para pessoas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CmdError {
    pub kind: &'static str,
    pub message: String,
}

impl CmdError {
    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }
}

/// Comandos de **leitura** que a GUI pode pedir. Cada um tem argumentos fixos, terminados em `--json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowed {
    Inventory,
    /// Sem `--fetch`: a GUI não usa a rede nem escreve em `.git`. A pasta é a que a CLI resolve (workspace).
    Repos,
    /// Só o relatório. `dotfiles adopt` e `dotfiles link` escrevem no HOME e ficam de fora (ADR 0026).
    Dotfiles,
}

impl Allowed {
    /// Aceita só o nome exato; qualquer outra coisa (maiúscula, espaço, argumento embutido) é recusada.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "inventory" => Some(Self::Inventory),
            "repos" => Some(Self::Repos),
            n if n.starts_with("dotfiles") => Some(Self::Dotfiles),
            _ => None,
        }
    }

    pub fn args(self) -> [&'static str; 2] {
        match self {
            Self::Inventory => ["inventory", "--json"],
            Self::Repos => ["repos", "--json"],
            Self::Dotfiles => ["dotfiles", "--json"],
        }
    }
}

/// Escolhe o binário a partir do valor de `NODUSCTL`. Sem a variável, `nodusctl` no PATH. Com ela, só caminho **absoluto**:
/// o app roda com o diretório atual em `src-tauri/`, então um caminho relativo apontaria para outro lugar que o do terminal.
pub fn resolve_binary(env_value: Option<OsString>) -> Result<PathBuf, CmdError> {
    let Some(value) = env_value else {
        return Ok(PathBuf::from("nodusctl"));
    };
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(CmdError::new(
            "invalid_binary",
            format!("NODUSCTL precisa ser um caminho absoluto (recebi {:?})", path.as_os_str()),
        ))
    }
}

/// Lê `nodusctl X.Y.Z` (a saída de `nodusctl --version`). Estrito: sem sufixo, sem prefixo, sem número fora de `u64`.
pub fn parse_version(output: &str) -> Option<(u64, u64, u64)> {
    let version = output.trim_end_matches('\n').strip_prefix("nodusctl ")?;
    let mut parts = version.split('.');
    let mut number = || -> Option<u64> {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    let (major, minor, patch) = (number()?, number()?, number()?);
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// A GUI aceita a CLI do mesmo `major.minor` (em 0.x, uma mudança de minor pode quebrar o JSON). O patch pode diferir.
pub fn compatible(gui: (u64, u64, u64), cli: (u64, u64, u64)) -> bool {
    gui.0 == cli.0 && gui.1 == cli.1
}

/// Roda o binário com `args`, com um limite de saída. Sem shell; entrada vazia; o erro padrão volta junto para as mensagens.
fn run(binary: &Path, args: &[&str]) -> Result<(std::process::ExitStatus, String, String), CmdError> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => CmdError::new("not_found", format!("nodusctl não encontrado ({})", binary.display())),
            _ => CmdError::new("failed", format!("não foi possível executar o nodusctl: {e}")),
        })?;

    // O erro padrão é lido em outra thread: se o processo encher o cano do stderr enquanto esperamos o stdout, os dois travariam.
    // Guarda só os primeiros 64 KiB e descarta o resto.
    let stderr_reader = child.stderr.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut kept = Vec::new();
            let mut buf = [0u8; 8192];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if kept.len() < 64 * 1024 {
                    kept.extend_from_slice(&buf[..n]);
                }
            }
            String::from_utf8_lossy(&kept).into_owned()
        })
    });

    let mut stdout = String::new();
    let mut too_large = false;
    if let Some(pipe) = child.stdout.take() {
        let mut limited = pipe.take(MAX_OUTPUT_BYTES as u64 + 1);
        let mut bytes = Vec::new();
        limited.read_to_end(&mut bytes).map_err(|e| CmdError::new("failed", format!("lendo a saída do nodusctl: {e}")))?;
        if bytes.len() > MAX_OUTPUT_BYTES {
            too_large = true;
            // Fecha o cano para o processo não ficar bloqueado escrevendo.
            drop(limited);
            let _ = child.kill();
        } else {
            stdout = String::from_utf8_lossy(&bytes).into_owned();
        }
    }
    let status = child.wait().map_err(|e| CmdError::new("failed", format!("esperando o nodusctl: {e}")))?;
    let stderr = stderr_reader.and_then(|handle| handle.join().ok()).unwrap_or_default();
    if too_large {
        return Err(CmdError::new("output_too_large", "a saída do nodusctl passou do limite aceito"));
    }
    Ok((status, stdout, stderr))
}

/// Roda um comando de leitura e devolve o JSON bruto (texto). Ordem: lista, versão, comando.
pub fn read(binary: &Path, name: &str, gui_version: (u64, u64, u64)) -> Result<String, CmdError> {
    let allowed = Allowed::parse(name)
        .ok_or_else(|| CmdError::new("not_allowed", format!("comando não permitido: {name:?}")))?;

    let (status, out, _) = run(binary, &["--version"])?;
    let cli_version = parse_version(&out).filter(|_| status.success()).ok_or_else(|| {
        CmdError::new("version_unknown", "não consegui ler a versão do nodusctl (saída de --version inesperada)")
    })?;
    if !compatible(gui_version, cli_version) {
        return Err(CmdError::new(
            "version_mismatch",
            format!(
                "nodusctl {}.{}.{} não é compatível com esta interface ({}.{}.x)",
                cli_version.0, cli_version.1, cli_version.2, gui_version.0, gui_version.1
            ),
        ));
    }

    let (status, out, err) = run(binary, &allowed.args())?;
    if !status.success() {
        let detail = err.trim();
        return Err(CmdError::new(
            "failed",
            if detail.is_empty() { format!("o nodusctl terminou com {status}") } else { detail.to_string() },
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    /// Serializa "criar o script" e "executá-lo". Sem isso, o `fork` de um teste herda o descritor de escrita ainda aberto do
    /// script de outro teste, e o `exec` desse outro recebe ETXTBSY ("text file busy"): falha intermitente, que já derrubou o CI da main.
    static EXEC: Mutex<()> = Mutex::new(());

    fn ler(binary: &Path, name: &str, gui: (u64, u64, u64)) -> Result<String, CmdError> {
        let _guard = EXEC.lock().unwrap_or_else(|e| e.into_inner());
        read(binary, name, gui)
    }

    // --- allowlist: a GUI só roda comandos de leitura, com argumentos fixos (ADR 0026) ---

    #[test]
    fn allowlist_aceita_so_inventory_repos_e_dotfiles() {
        assert_eq!(Allowed::parse("inventory"), Some(Allowed::Inventory));
        assert_eq!(Allowed::parse("repos"), Some(Allowed::Repos));
        assert_eq!(Allowed::parse("dotfiles"), Some(Allowed::Dotfiles));
    }

    #[test]
    fn dotfiles_so_o_relatorio_nunca_adopt_nem_link() {
        // `dotfiles adopt` e `dotfiles link` escrevem no HOME: a GUI não pode pedi-los (ADR 0026).
        for nome in [
            "dotfiles adopt", "dotfiles link", "dotfiles --json", "dotfiles adopt .bashrc", "dotfiles link .bashrc",
            "Dotfiles", "DOTFILES", "dotfiles ", " dotfiles", "dotfiles\n", "dotfiles;ls", "dotfilesadopt",
        ] {
            assert_eq!(Allowed::parse(nome), None, "{nome:?} não pode passar");
        }
    }

    #[test]
    fn repos_nao_aceita_argumento_nem_fetch() {
        for nome in ["repos --fetch", "repos --json", "repos /tmp", "Repos", "REPOS", "repos ", "repos\n", "repos;ls"] {
            assert_eq!(Allowed::parse(nome), None, "{nome:?} não pode passar");
        }
    }

    #[test]
    fn allowlist_recusa_tudo_o_que_nao_esta_na_lista() {
        for nome in [
            "", " ", "Inventory", "INVENTORY", "inventory ", " inventory", "inventory --json", "inventory;ls",
            "inventory\n", "inventory\0", "apply", "undo", "adopt", "link", "preset", "history", "workspace",
            "--version", "--help", "../inventory", "/bin/sh", "-c", "sudo",
        ] {
            assert_eq!(Allowed::parse(nome), None, "{nome:?} não pode passar");
        }
    }

    #[test]
    fn argumentos_sao_fixos_e_terminam_em_json() {
        assert_eq!(Allowed::Inventory.args(), ["inventory", "--json"]);
        assert_eq!(Allowed::Repos.args(), ["repos", "--json"], "sem --fetch: a GUI não usa a rede nem escreve em .git");
    }

    #[test]
    fn argumentos_de_dotfiles_sao_so_o_relatorio() {
        assert_eq!(Allowed::Dotfiles.args(), ["dotfiles", "--json"]);
    }

    #[test]
    fn le_o_json_de_dotfiles() {
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.0"; exit 0; fi
if [ "$1" = "dotfiles" ] && [ "$2" = "--json" ] && [ -z "$3" ]; then echo '[]'; exit 0; fi
echo "argumentos inesperados: $*" >&2; exit 64
"#;
        let (dir, bin) = falso(corpo);
        assert_eq!(ler(&bin, "dotfiles", (0, 1, 0)).unwrap().trim(), "[]");
        assert_eq!(chamadas(&dir), ["--version", "dotfiles --json"]);
    }

    #[test]
    fn adopt_e_link_nao_executam_nada() {
        let (dir, bin) = falso(BOM);
        for nome in ["dotfiles adopt .bashrc", "dotfiles link .bashrc"] {
            assert_eq!(ler(&bin, nome, (0, 1, 0)).unwrap_err().kind, "not_allowed", "{nome:?}");
        }
        assert!(chamadas(&dir).is_empty(), "o binário não pode nem ser executado");
    }

    #[test]
    fn le_o_json_de_repos() {
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.0"; exit 0; fi
if [ "$1" = "repos" ] && [ "$2" = "--json" ]; then echo '[]'; exit 0; fi
echo "argumentos inesperados: $*" >&2; exit 64
"#;
        let (dir, bin) = falso(corpo);
        assert_eq!(ler(&bin, "repos", (0, 1, 0)).unwrap().trim(), "[]");
        assert_eq!(chamadas(&dir), ["--version", "repos --json"]);
    }

    // --- binário: `NODUSCTL` só vale como caminho absoluto (o app roda com o diretório atual em `src-tauri/`) ---

    #[test]
    fn sem_variavel_usa_o_nodusctl_do_path() {
        assert_eq!(resolve_binary(None).unwrap(), PathBuf::from("nodusctl"));
    }

    #[test]
    fn caminho_absoluto_e_aceito() {
        for caminho in ["/usr/bin/nodusctl", "/home/x/nodus/target/release/nodusctl", "/a/../b/nodusctl"] {
            assert_eq!(resolve_binary(Some(caminho.into())).unwrap(), PathBuf::from(caminho));
        }
    }

    #[test]
    fn caminho_relativo_ou_vazio_e_recusado() {
        for caminho in ["../target/release/nodusctl", "./nodusctl", "target/release/nodusctl", "nodusctl", "ls", "", " ", "~/nodusctl"] {
            let erro = resolve_binary(Some(caminho.into())).unwrap_err();
            assert_eq!(erro.kind, "invalid_binary", "{caminho:?}");
            assert!(erro.message.contains("caminho absoluto"), "{caminho:?}: {}", erro.message);
        }
    }

    // --- versão: `nodusctl --version` tem que ser o mesmo major.minor da GUI ---

    #[test]
    fn le_a_versao_do_nodusctl() {
        assert_eq!(parse_version("nodusctl 0.1.0\n"), Some((0, 1, 0)));
        assert_eq!(parse_version("nodusctl 12.34.56"), Some((12, 34, 56)));
    }

    #[test]
    fn versao_malformada_nao_e_lida() {
        for texto in [
            "", "nodusctl", "nodusctl 0.1", "nodusctl 0.1.0.0", "0.1.0", "nodus 0.1.0", "NODUSCTL 0.1.0",
            "nodusctl 0.1.0-dev", "nodusctl v0.1.0", "nodusctl 0.1.x", "nodusctl -1.0.0",
            "nodusctl 99999999999999999999.0.0", "nodusctl 0.1.0 extra", "hello",
        ] {
            assert_eq!(parse_version(texto), None, "{texto:?} não é uma versão válida");
        }
    }

    #[test]
    fn compativel_so_com_mesmo_major_e_minor() {
        assert!(compatible((0, 1, 0), (0, 1, 0)));
        assert!(compatible((0, 1, 0), (0, 1, 9)), "patch pode diferir");
        assert!(compatible((1, 2, 7), (1, 2, 0)));
        assert!(!compatible((0, 1, 0), (0, 2, 0)), "minor diferente quebra em 0.x");
        assert!(!compatible((0, 1, 0), (1, 1, 0)), "major diferente");
        assert!(!compatible((1, 2, 0), (1, 3, 0)));
        assert!(!compatible((1, 2, 0), (2, 2, 0)));
    }

    // --- `read`: roda o binário de verdade (um script de mentira) ---

    static CONTADOR: AtomicU32 = AtomicU32::new(0);

    /// Cria uma pasta temporária com um `nodusctl` de mentira. O script deixa um arquivo `chamadas` com uma linha por execução.
    fn falso(corpo: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "nodus-desktop-teste-{}-{}",
            std::process::id(),
            CONTADOR.fetch_add(1, Ordering::SeqCst)
        ));
        let _guard = EXEC.lock().unwrap_or_else(|e| e.into_inner());
        fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("nodusctl");
        fs::write(&bin, format!("#!/bin/sh\necho \"$*\" >> '{}/chamadas'\n{corpo}\n", dir.display())).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        (dir, bin)
    }

    fn chamadas(dir: &Path) -> Vec<String> {
        fs::read_to_string(dir.join("chamadas")).map(|t| t.lines().map(String::from).collect()).unwrap_or_default()
    }

    const BOM: &str = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.4"; exit 0; fi
if [ "$1" = "inventory" ] && [ "$2" = "--json" ]; then echo '{"ok":true}'; exit 0; fi
echo "argumentos inesperados: $*" >&2; exit 64
"#;

    #[test]
    fn le_o_json_do_inventario() {
        let (dir, bin) = falso(BOM);
        let saida = ler(&bin, "inventory", (0, 1, 0)).unwrap();
        assert_eq!(saida.trim(), r#"{"ok":true}"#);
        assert_eq!(chamadas(&dir), ["--version", "inventory --json"], "versão primeiro, depois o comando, nada mais");
    }

    #[test]
    fn comando_fora_da_lista_nao_executa_nada() {
        let (dir, bin) = falso(BOM);
        for nome in ["apply", "inventory --json", "", "sudo", "undo 1"] {
            let erro = ler(&bin, nome, (0, 1, 0)).unwrap_err();
            assert_eq!(erro.kind, "not_allowed", "{nome:?}");
        }
        assert!(chamadas(&dir).is_empty(), "o binário não pode nem ser executado");
    }

    #[test]
    fn versao_incompativel_bloqueia_e_nao_roda_o_comando() {
        let (dir, bin) = falso(&BOM.replace("0.1.4", "0.2.0"));
        let erro = ler(&bin, "inventory", (0, 1, 0)).unwrap_err();
        assert_eq!(erro.kind, "version_mismatch", "{}", erro.message);
        assert!(erro.message.contains("0.2.0"), "a mensagem diz qual versão veio: {}", erro.message);
        assert_eq!(chamadas(&dir), ["--version"], "só a versão foi pedida; o inventário nunca rodou");
    }

    #[test]
    fn major_diferente_tambem_bloqueia() {
        let (_dir, bin) = falso(&BOM.replace("0.1.4", "1.1.0"));
        assert_eq!(ler(&bin, "inventory", (0, 1, 0)).unwrap_err().kind, "version_mismatch");
    }

    #[test]
    fn saida_de_versao_ilegivel_e_erro_proprio() {
        let (dir, bin) = falso("echo hello; exit 0");
        let erro = ler(&bin, "inventory", (0, 1, 0)).unwrap_err();
        assert_eq!(erro.kind, "version_unknown");
        assert_eq!(chamadas(&dir), ["--version"]);
    }

    #[test]
    fn binario_inexistente_e_not_found() {
        let erro = ler(Path::new("/nao/existe/nodusctl"), "inventory", (0, 1, 0)).unwrap_err();
        assert_eq!(erro.kind, "not_found");
    }

    #[test]
    fn comando_que_falha_devolve_o_erro_da_cli() {
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.0"; exit 0; fi
echo "falhou de propósito" >&2; exit 3
"#;
        let (_dir, bin) = falso(corpo);
        let erro = ler(&bin, "inventory", (0, 1, 0)).unwrap_err();
        assert_eq!(erro.kind, "failed");
        assert!(erro.message.contains("falhou de propósito"), "{}", erro.message);
    }

    #[test]
    fn saida_gigante_e_recusada() {
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.0"; exit 0; fi
head -c 20000000 /dev/zero | tr '\0' 'a'
"#;
        let (_dir, bin) = falso(corpo);
        assert_eq!(ler(&bin, "inventory", (0, 1, 0)).unwrap_err().kind, "output_too_large");
    }

    #[test]
    fn nenhum_argumento_vem_do_chamador() {
        // O que o frontend manda é só o nome; qualquer tentativa de embutir argumento é recusada (teste acima) e
        // o conjunto de argumentos executados é exatamente o da allowlist.
        let (dir, bin) = falso(BOM);
        ler(&bin, "inventory", (0, 1, 0)).unwrap();
        for linha in chamadas(&dir) {
            assert!(linha == "--version" || linha == "inventory --json", "chamada fora do esperado: {linha}");
        }
    }
}
