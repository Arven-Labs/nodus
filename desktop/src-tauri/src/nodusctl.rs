//! Ponte entre a GUI e o `nodusctl` (ADR 0026).
//!
//! A fronteira de confiança é aqui, no Rust, e não no JavaScript: a webview é código de página web. O frontend só
//! manda o **nome** de um comando; este módulo recusa o que não está na lista, monta os argumentos sozinho, sem shell,
//! e só roda depois de conferir que o `nodusctl --version` tem o mesmo `major.minor` da GUI.

use serde::Serialize;
use std::ffi::OsString;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Limite da saída aceita de um comando (16 MiB). O `inventory` de uma máquina com milhares de pacotes fica bem abaixo.
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

/// Prazo padrão de uma chamada ao `nodusctl`: os comandos de leitura locais levam de milissegundos a poucos segundos.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// De quanto em quanto tempo a espera olha se o processo terminou, se a saída passou do limite ou se o prazo acabou.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// O `kill` do sistema (util-linux ou procps), por caminho fixo: não depende do PATH.
const KILL_BINARY: &str = "/usr/bin/kill";

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
    /// Só o relatório (`checkupdates` e `flatpak remote-ls`, que usam a rede); nada é instalado nem atualizado.
    Updates,
    /// Só o relatório. `workspace set` e `workspace unset` gravam no `config.toml` e ficam de fora (ADR 0026).
    Workspace,
    /// Só a listagem. `history undo <n>` roda `sudo pacman -Rs` ou escreve no HOME e fica de fora (ADR 0026).
    History,
    /// Só o plano (`preset plan`; nada é instalado). O arquivo do preset é o único argumento que não é fixo, e vem de
    /// `NODUS_PRESET` lido aqui no Rust, nunca do frontend. `preset apply` e `preset adopt` instalam ou escrevem e ficam de fora.
    PresetPlan,
    /// Só a sugestão (`preset suggest`, sem `--all`; só lê o pacman, o Flatpak e o `config.toml`). Mesmo arquivo de `NODUS_PRESET`.
    /// `preset adopt`, `ignored` e `unignore` escrevem e ficam de fora.
    PresetSuggest,
}

/// Prazo do `updates`: consulta a rede (espelhos do pacman, Flathub), então demora mais que os comandos locais.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);

impl Allowed {
    /// Aceita só o nome exato; qualquer outra coisa (maiúscula, espaço, argumento embutido) é recusada.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "inventory" => Some(Self::Inventory),
            "repos" => Some(Self::Repos),
            "dotfiles" => Some(Self::Dotfiles),
            "updates" => Some(Self::Updates),
            "workspace" => Some(Self::Workspace),
            "history" => Some(Self::History),
            "preset_plan" => Some(Self::PresetPlan),
            "preset_suggest" => Some(Self::PresetSuggest),
            _ => None,
        }
    }

    /// Os argumentos fixos. No `PresetPlan`, o caminho do arquivo é acrescentado depois, por `read_with`.
    pub fn args(self) -> Vec<&'static str> {
        match self {
            Self::Inventory => vec!["inventory", "--json"],
            Self::Repos => vec!["repos", "--json"],
            Self::Dotfiles => vec!["dotfiles", "--json"],
            Self::Updates => vec!["updates", "--json"],
            Self::Workspace => vec!["workspace", "--json"],
            Self::History => vec!["history", "--json"],
            Self::PresetPlan => vec!["preset", "plan", "--json"],
            Self::PresetSuggest => vec!["preset", "suggest", "--json"],
        }
    }

    /// Quanto a GUI espera por este comando antes de matá-lo. Cada comando tem o seu: os de rede (a tela de atualizações) precisam de mais.
    pub fn timeout(self) -> Duration {
        match self {
            Self::Inventory | Self::Repos | Self::Dotfiles | Self::Workspace | Self::History | Self::PresetPlan | Self::PresetSuggest => {
                DEFAULT_TIMEOUT
            }
            Self::Updates => NETWORK_TIMEOUT,
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

/// O arquivo do preset vem do valor de `NODUS_PRESET`, só caminho **absoluto** (o app roda com o diretório atual em `src-tauri/`).
/// Só o `preset_plan` chama isto: uma `NODUS_PRESET` ruim não pode derrubar as outras abas.
pub fn resolve_preset(env_value: Option<OsString>) -> Result<PathBuf, CmdError> {
    let Some(value) = env_value else {
        return Err(CmdError::new(
            "preset_not_set",
            "defina NODUS_PRESET com o caminho absoluto do arquivo de preset (o mesmo que você passaria a `nodusctl preset plan`)",
        ));
    };
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(CmdError::new(
            "invalid_preset",
            format!("NODUS_PRESET precisa ser um caminho absoluto (recebi {:?})", path.as_os_str()),
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

/// Mata o grupo de processos do filho (o `nodusctl` e o que ele lançou: `checkupdates`, `flatpak`...). Sem isso, um neto vivo
/// segura o cano de saída aberto e a leitura nunca termina. O número vem do `child.id()`, nunca de entrada externa; sem shell.
/// Se o `kill` do sistema falhar (não existe, ou o grupo já acabou), cai para matar só o filho.
fn kill_group(child: &mut Child) {
    let group = format!("-{}", child.id());
    let killed = Command::new(KILL_BINARY)
        .args(["-KILL", "--", &group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !killed {
        let _ = child.kill();
    }
}

/// "30 s" ou "300 ms", para a mensagem de tempo esgotado.
fn describe(timeout: Duration) -> String {
    if timeout.as_secs() >= 1 { format!("{} s", timeout.as_secs()) } else { format!("{} ms", timeout.as_millis()) }
}

/// Por que a espera foi interrompida antes de o processo terminar sozinho.
enum Stop {
    TimedOut,
    TooLarge,
}

/// Roda o binário com `args`, com um limite de saída e um prazo. Sem shell; entrada vazia; o erro padrão volta junto para as
/// mensagens. O filho nasce em um grupo de processos próprio (`process_group(0)`), para o prazo poder matar o grupo inteiro.
fn run<S: AsRef<std::ffi::OsStr>>(binary: &Path, args: &[S], timeout: Duration) -> Result<(ExitStatus, String, String), CmdError> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
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

    // A saída é lida em outra thread (como o erro padrão), para a thread principal poder vigiar o prazo. Passou de 16 MiB: levanta
    // a bandeira e para de ler; a thread principal mata o grupo.
    let too_large = Arc::new(AtomicBool::new(false));
    let stdout_reader = child.stdout.take().map(|pipe| {
        let flag = Arc::clone(&too_large);
        std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut bytes = Vec::new();
            pipe.take(MAX_OUTPUT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
            if bytes.len() > MAX_OUTPUT_BYTES {
                flag.store(true, Ordering::SeqCst);
            }
            Ok(bytes)
        })
    });

    let deadline = Instant::now() + timeout;
    let mut stop = None;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| CmdError::new("failed", format!("esperando o nodusctl: {e}")))? {
            break status;
        }
        if too_large.load(Ordering::SeqCst) {
            stop = Some(Stop::TooLarge);
        } else if Instant::now() >= deadline {
            stop = Some(Stop::TimedOut);
        }
        if stop.is_some() {
            kill_group(&mut child);
            break child.wait().map_err(|e| CmdError::new("failed", format!("esperando o nodusctl: {e}")))?;
        }
        std::thread::sleep(POLL_INTERVAL);
    };

    // O grupo morreu (ou o processo terminou): os canos fecham e as threads acabam.
    let bytes = stdout_reader
        .map(|handle| handle.join().unwrap_or_else(|_| Err(std::io::Error::other("a thread de leitura falhou"))))
        .unwrap_or_else(|| Ok(Vec::new()));
    let stderr = stderr_reader.and_then(|handle| handle.join().ok()).unwrap_or_default();
    match stop {
        Some(Stop::TimedOut) => {
            return Err(CmdError::new(
                "timeout",
                format!("o nodusctl demorou mais de {} e foi interrompido", describe(timeout)),
            ));
        }
        Some(Stop::TooLarge) => {
            return Err(CmdError::new("output_too_large", "a saída do nodusctl passou do limite aceito"));
        }
        None => {}
    }
    let bytes = bytes.map_err(|e| CmdError::new("failed", format!("lendo a saída do nodusctl: {e}")))?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(CmdError::new("output_too_large", "a saída do nodusctl passou do limite aceito"));
    }
    Ok((status, String::from_utf8_lossy(&bytes).into_owned(), stderr))
}

/// Roda um comando de leitura e devolve o JSON bruto (texto). Ordem: lista, versão, comando. O prazo é o do comando.
/// `preset` é o valor cru de `NODUS_PRESET`; só o `preset_plan` o usa.
pub fn read(binary: &Path, name: &str, gui_version: (u64, u64, u64), preset: Option<OsString>) -> Result<String, CmdError> {
    let timeout = Allowed::parse(name).map_or(DEFAULT_TIMEOUT, Allowed::timeout);
    read_with(binary, name, gui_version, preset, timeout)
}

/// Como `read`, com o prazo escolhido por quem chama (os testes usam um prazo curto). O prazo vale para cada processo: a
/// conferência de `--version` e o comando.
pub fn read_with(
    binary: &Path,
    name: &str,
    gui_version: (u64, u64, u64),
    preset: Option<OsString>,
    timeout: Duration,
) -> Result<String, CmdError> {
    let allowed = Allowed::parse(name)
        .ok_or_else(|| CmdError::new("not_allowed", format!("comando não permitido: {name:?}")))?;
    // O caminho é conferido antes de executar qualquer coisa. Um argumento só (sem shell), então espaço não separa nada.
    let mut argv: Vec<OsString> = allowed.args().into_iter().map(OsString::from).collect();
    if matches!(allowed, Allowed::PresetPlan | Allowed::PresetSuggest) {
        argv.push(resolve_preset(preset)?.into_os_string());
    }

    let (status, out, _) = run(binary, &["--version"], timeout)?;
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

    let (status, out, err) = run(binary, argv.as_slice(), timeout)?;
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
pub(crate) mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// Serializa "criar o script" e "executá-lo". Sem isso, o `fork` de um teste herda o descritor de escrita ainda aberto do
    /// script de outro teste, e o `exec` desse outro recebe ETXTBSY ("text file busy"): falha intermitente, que já derrubou o CI da main.
    pub(crate) static EXEC: Mutex<()> = Mutex::new(());

    fn ler(binary: &Path, name: &str, gui: (u64, u64, u64)) -> Result<String, CmdError> {
        let _guard = EXEC.lock().unwrap_or_else(|e| e.into_inner());
        read(binary, name, gui, None)
    }

    /// Como `ler`, com o valor que `NODUS_PRESET` teria (o backend lê a variável; o frontend nunca fornece caminho).
    fn ler_preset(binary: &Path, name: &str, gui: (u64, u64, u64), preset: Option<&str>) -> Result<String, CmdError> {
        let _guard = EXEC.lock().unwrap_or_else(|e| e.into_inner());
        read(binary, name, gui, preset.map(std::ffi::OsString::from))
    }

    /// Como `ler`, mas com um prazo curto escolhido pelo teste (sem esperar 30 s no CI).
    fn ler_com_prazo(binary: &Path, name: &str, gui: (u64, u64, u64), prazo: Duration) -> Result<String, CmdError> {
        let _guard = EXEC.lock().unwrap_or_else(|e| e.into_inner());
        read_with(binary, name, gui, None, prazo)
    }

    /// O processo existe e não é zumbi? (Um filho morto cujo pai não o recolheu ainda aparece em `/proc` como `Z`.)
    fn viva(pid: u32) -> bool {
        match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(stat) => stat.rsplit_once(") ").is_some_and(|(_, resto)| !resto.starts_with('Z')),
            Err(_) => false,
        }
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
            "inventory\n", "inventory\0", "apply", "undo", "adopt", "link", "preset",
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

    // --- tempo limite: o `nodusctl` que não termina é morto, junto com os filhos (ADR 0026) ---

    #[test]
    fn prazo_estourado_vira_timeout_e_nao_trava() {
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.0"; exit 0; fi
sleep 5
echo '{"ok":true}'
"#;
        let (_dir, bin) = falso(corpo);
        let inicio = Instant::now();
        let erro = ler_com_prazo(&bin, "inventory", (0, 1, 0), Duration::from_millis(300)).unwrap_err();
        assert_eq!(erro.kind, "timeout", "{}", erro.message);
        assert!(erro.message.contains("demorou"), "{}", erro.message);
        assert!(inicio.elapsed() < Duration::from_secs(3), "voltou em {:?}: esperou o script inteiro", inicio.elapsed());
    }

    #[test]
    fn estouro_na_versao_nao_roda_o_comando() {
        let corpo = r#"
if [ "$1" = "--version" ]; then sleep 5; echo "nodusctl 0.1.0"; exit 0; fi
echo '{"ok":true}'
"#;
        let (dir, bin) = falso(corpo);
        let erro = ler_com_prazo(&bin, "inventory", (0, 1, 0), Duration::from_millis(300)).unwrap_err();
        assert_eq!(erro.kind, "timeout", "{}", erro.message);
        assert_eq!(chamadas(&dir), ["--version"], "o comando nunca pode rodar depois de a versão estourar");
    }

    #[test]
    fn prazo_estourado_mata_tambem_o_neto_que_segura_o_cano() {
        // O neto (`sleep`) herda o cano de saída: matar só o filho deixaria o neto vivo e a leitura sem fim.
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.0"; exit 0; fi
sleep 30 &
echo $! > "$(dirname "$0")/neto.pid"
wait
"#;
        let (dir, bin) = falso(corpo);
        let inicio = Instant::now();
        let erro = ler_com_prazo(&bin, "inventory", (0, 1, 0), Duration::from_secs(1)).unwrap_err();
        assert_eq!(erro.kind, "timeout", "{}", erro.message);
        assert!(inicio.elapsed() < Duration::from_secs(10), "voltou em {:?}: ficou preso no cano", inicio.elapsed());
        let pid: u32 = fs::read_to_string(dir.join("neto.pid")).unwrap().trim().parse().unwrap();
        let limite = Instant::now() + Duration::from_secs(2);
        while viva(pid) && Instant::now() < limite {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!viva(pid), "o neto {pid} continua vivo depois do tempo limite");
    }

    #[test]
    fn dentro_do_prazo_o_resultado_e_o_de_sempre() {
        let (_dir, bin) = falso(BOM);
        let saida = ler_com_prazo(&bin, "inventory", (0, 1, 0), Duration::from_secs(30)).unwrap();
        assert_eq!(saida.trim(), r#"{"ok":true}"#);
    }

    #[test]
    fn o_prazo_dos_comandos_locais_e_de_30_segundos() {
        assert_eq!(Allowed::Inventory.timeout(), Duration::from_secs(30));
        assert_eq!(Allowed::Repos.timeout(), Duration::from_secs(30));
        assert_eq!(Allowed::Dotfiles.timeout(), Duration::from_secs(30));
        assert_eq!(Allowed::Workspace.timeout(), Duration::from_secs(30));
        assert_eq!(Allowed::History.timeout(), Duration::from_secs(30));
    }

    #[test]
    fn history_so_a_listagem_sem_undo() {
        assert_eq!(Allowed::parse("history"), Some(Allowed::History));
        assert_eq!(Allowed::History.args(), ["history", "--json"]);
        for nome in [
            "history undo", "history undo 1", "history --json", "history 1", "History", "HISTORY", "history ", " history",
            "history\n", "history;ls", "histor", "historys",
        ] {
            assert_eq!(Allowed::parse(nome), None, "{nome:?} não pode passar: `undo` roda sudo pacman -Rs e escreve no HOME");
        }
    }

    #[test]
    fn workspace_so_o_relatorio_sem_set_nem_unset() {
        assert_eq!(Allowed::parse("workspace"), Some(Allowed::Workspace));
        assert_eq!(Allowed::Workspace.args(), ["workspace", "--json"]);
        for nome in [
            "workspace set", "workspace set /tmp", "workspace unset", "workspace --json", "workspace /tmp", "Workspace", "WORKSPACE",
            "workspace ", " workspace", "workspace\n", "workspace;ls", "workspaces", "workspac",
        ] {
            assert_eq!(Allowed::parse(nome), None, "{nome:?} não pode passar: set e unset gravam no config.toml");
        }
    }

    #[test]
    fn o_updates_usa_a_rede_e_tem_120_segundos() {
        assert_eq!(Allowed::Updates.timeout(), Duration::from_secs(120));
    }

    #[test]
    fn updates_so_o_relatorio_sem_argumento() {
        assert_eq!(Allowed::parse("updates"), Some(Allowed::Updates));
        assert_eq!(Allowed::Updates.args(), ["updates", "--json"]);
        for nome in [
            "updates --json", "updates --fetch", "updates /tmp", "Updates", "UPDATES", "updates ", " updates", "updates\n",
            "updates;ls", "update", "updatesx",
        ] {
            assert_eq!(Allowed::parse(nome), None, "{nome:?} não pode passar");
        }
    }

    #[test]
    fn le_o_json_de_updates() {
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.4"; exit 0; fi
if [ "$1" = "updates" ] && [ "$2" = "--json" ]; then echo '{"pacman":[]}'; exit 0; fi
echo "argumentos inesperados: $*" >&2; exit 64
"#;
        let (dir, bin) = falso(corpo);
        let saida = ler(&bin, "updates", (0, 1, 0)).unwrap();
        assert_eq!(saida.trim(), r#"{"pacman":[]}"#);
        assert_eq!(chamadas(&dir), ["--version", "updates --json"]);
    }

    // --- preset plan: o único comando de leitura com um argumento, e ele vem de NODUS_PRESET, nunca do frontend ---

    const PLANO_FALSO: &str = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.4"; exit 0; fi
if [ "$1" = "preset" ] && [ "$2" = "plan" ] && [ "$3" = "--json" ] && [ -n "$4" ] && [ -z "$5" ]; then echo '{"plano":true}'; exit 0; fi
echo "argumentos inesperados: $*" >&2; exit 64
"#;

    #[test]
    fn preset_plan_so_o_plano_sem_apply_nem_adopt() {
        assert_eq!(Allowed::parse("preset_plan"), Some(Allowed::PresetPlan));
        assert_eq!(Allowed::PresetPlan.timeout(), Duration::from_secs(30));
        for nome in [
            "preset plan", "preset_plan /tmp/p.toml", "preset_plan --json", "preset apply", "preset_apply", "preset adopt",
            "preset_adopt", "preset init", "preset_init", "preset suggest", "preset ignored", "preset unignore", "preset_ignored",
            "preset_unignore", "preset_suggest /tmp/p.toml", "preset_suggest --all", "Preset_suggest", "preset_suggest ", "preset-suggest",
            "Preset_plan", "PRESET_PLAN", "preset_plan ", " preset_plan", "preset_plan\n", "preset_plan;ls", "preset-plan", "preset_plans",
        ] {
            assert_eq!(Allowed::parse(nome), None, "{nome:?} não pode passar: apply instala e adopt escreve no preset");
        }
    }

    #[test]
    fn preset_plan_roda_com_o_arquivo_da_variavel() {
        let (dir, bin) = falso(PLANO_FALSO);
        let saida = ler_preset(&bin, "preset_plan", (0, 1, 0), Some("/tmp/nodus-teste/p.toml")).unwrap();
        assert_eq!(saida.trim(), r#"{"plano":true}"#);
        assert_eq!(chamadas(&dir), ["--version", "preset plan --json /tmp/nodus-teste/p.toml"]);
    }

    #[test]
    fn o_caminho_com_espaco_vai_como_um_argumento_so() {
        // Sem shell: o espaço não separa argumentos (o falso exige exatamente 4 argumentos).
        let (_dir, bin) = falso(PLANO_FALSO);
        let saida = ler_preset(&bin, "preset_plan", (0, 1, 0), Some("/tmp/minha pasta/p.toml")).unwrap();
        assert_eq!(saida.trim(), r#"{"plano":true}"#);
    }

    #[test]
    fn sem_a_variavel_o_erro_diz_qual_definir_e_nada_executa() {
        let (dir, bin) = falso(PLANO_FALSO);
        let erro = ler_preset(&bin, "preset_plan", (0, 1, 0), None).unwrap_err();
        assert_eq!(erro.kind, "preset_not_set");
        assert!(erro.message.contains("NODUS_PRESET"), "a mensagem precisa nomear a variável: {}", erro.message);
        assert!(chamadas(&dir).is_empty(), "o binário não pode nem ser executado");
    }

    #[test]
    fn caminho_relativo_ou_vazio_e_recusado_e_nada_executa() {
        // O app roda com o diretório atual em `src-tauri/`: um caminho relativo apontaria para outro lugar que o do terminal.
        let (dir, bin) = falso(PLANO_FALSO);
        for valor in ["", "p.toml", "./p.toml", "../p.toml", "~/p.toml", "-p.toml", "--help"] {
            let erro = ler_preset(&bin, "preset_plan", (0, 1, 0), Some(valor)).unwrap_err();
            assert_eq!(erro.kind, "invalid_preset", "{valor:?}");
        }
        assert!(chamadas(&dir).is_empty(), "o binário não pode nem ser executado");
    }

    #[test]
    fn resolve_preset_so_aceita_caminho_absoluto() {
        assert_eq!(resolve_preset(Some("/home/x/preset.toml".into())).unwrap(), PathBuf::from("/home/x/preset.toml"));
        assert_eq!(resolve_preset(None).unwrap_err().kind, "preset_not_set");
        assert_eq!(resolve_preset(Some("preset.toml".into())).unwrap_err().kind, "invalid_preset");
    }

    // --- preset suggest: mesmo mecanismo do plan (arquivo vindo de NODUS_PRESET); sem `--all`, sem adopt/ignore ---

    const SUGESTAO_FALSA: &str = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.4"; exit 0; fi
if [ "$1" = "preset" ] && [ "$2" = "suggest" ] && [ "$3" = "--json" ] && [ -n "$4" ] && [ -z "$5" ]; then echo '{"sugestao":true}'; exit 0; fi
echo "argumentos inesperados: $*" >&2; exit 64
"#;

    #[test]
    fn preset_suggest_so_a_sugestao_sem_all_adopt_nem_ignore() {
        assert_eq!(Allowed::parse("preset_suggest"), Some(Allowed::PresetSuggest));
        assert_eq!(Allowed::PresetSuggest.timeout(), Duration::from_secs(30));
        assert_eq!(Allowed::PresetSuggest.args(), vec!["preset", "suggest", "--json"]);
    }

    #[test]
    fn preset_suggest_roda_com_o_arquivo_da_variavel() {
        let (dir, bin) = falso(SUGESTAO_FALSA);
        let saida = ler_preset(&bin, "preset_suggest", (0, 1, 0), Some("/tmp/minha pasta/p.toml")).unwrap();
        assert_eq!(saida.trim(), r#"{"sugestao":true}"#);
        assert_eq!(chamadas(&dir), ["--version", "preset suggest --json /tmp/minha pasta/p.toml"]);
    }

    #[test]
    fn preset_suggest_sem_caminho_valido_nada_executa() {
        let (dir, bin) = falso(SUGESTAO_FALSA);
        let erro = ler_preset(&bin, "preset_suggest", (0, 1, 0), None).unwrap_err();
        assert_eq!(erro.kind, "preset_not_set");
        assert!(erro.message.contains("NODUS_PRESET"), "{}", erro.message);
        for valor in ["", "p.toml", "./p.toml", "../p.toml", "~/p.toml", "-p.toml", "--all"] {
            let erro = ler_preset(&bin, "preset_suggest", (0, 1, 0), Some(valor)).unwrap_err();
            assert_eq!(erro.kind, "invalid_preset", "{valor:?}");
        }
        assert!(chamadas(&dir).is_empty(), "o binário não pode nem ser executado");
    }

    #[test]
    fn a_variavel_nao_vira_argumento_do_inventario_nem_do_plan_por_causa_do_suggest() {
        // O plan continua com o seu argv próprio: o suggest não pode ter vazado para ele.
        let (dir, bin) = falso(PLANO_FALSO);
        ler_preset(&bin, "preset_plan", (0, 1, 0), Some("/tmp/p.toml")).unwrap();
        assert_eq!(chamadas(&dir), ["--version", "preset plan --json /tmp/p.toml"]);
    }

    #[test]
    fn a_variavel_so_importa_para_o_preset_plan() {
        // Uma NODUS_PRESET relativa não pode derrubar as outras abas, e uma absoluta não vira argumento delas.
        let (dir, bin) = falso(BOM);
        ler_preset(&bin, "inventory", (0, 1, 0), Some("relativo.toml")).unwrap();
        ler_preset(&bin, "inventory", (0, 1, 0), Some("/tmp/p.toml")).unwrap();
        assert_eq!(chamadas(&dir), ["--version", "inventory --json", "--version", "inventory --json"]);
    }

    #[test]
    fn falha_do_nodusctl_no_preset_chega_com_a_mensagem_dele() {
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.4"; exit 0; fi
echo "Error: lendo /tmp/p.toml: No such file or directory" >&2; exit 1
"#;
        let (_dir, bin) = falso(corpo);
        let erro = ler_preset(&bin, "preset_plan", (0, 1, 0), Some("/tmp/p.toml")).unwrap_err();
        assert_eq!(erro.kind, "failed");
        assert!(erro.message.contains("No such file or directory"), "{}", erro.message);
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
