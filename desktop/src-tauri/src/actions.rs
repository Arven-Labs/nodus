// Ações que ESCREVEM, lançadas pela GUI (ADR 0027): nenhuma roda no processo da GUI. O botão abre um terminal já rodando
// `nodusctl <comando>`, e a CLI segue dona do plano, do `[s/N]` e do `sudo`. Lista fechada e separada da de leitura (`Allowed`).

use crate::nodusctl::{check_version, resolve_preset, CmdError, DEFAULT_TIMEOUT};
use serde::Serialize;
use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// O script do `sh -c`: roda o comando (`"$@"`), mostra o código de saída e espera o Enter, para a janela não fechar e levar o
/// resultado junto. É uma **constante de uma linha**: os dados (o `nodusctl`, o preset) entram só como parâmetros posicionais,
/// nunca no texto do script. Um caminho como `x'; rm -rf ~; '` é, para o `sh`, um valor de `$1`, não código.
pub const WRAPPER: &str = r#""$@"; code=$?; printf '\nFim (código %s). Enter para fechar.' "$code"; read _"#;

/// As ações que escrevem, em lista fechada. Só o nome exato entra; cada uma tem os argumentos fixos. O único argumento que a tela
/// pode mandar é **numérico** (`u32`, o mesmo tipo da CLI): um inteiro não tem sintaxe para escapar de nada. Texto e caminho vindos da
/// tela não existem aqui; o arquivo do preset vem de `NODUS_PRESET`, lido no Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `nodusctl preset apply <NODUS_PRESET>`: roda `sudo pacman -S` e `flatpak install` dentro do terminal, com a confirmação da CLI.
    PresetApply,
    /// `nodusctl history undo <n>`, `n >= 1`: o `sudo pacman -Rs` (ou a reversão de dotfile) é da CLI, que simula, mostra o plano e
    /// confirma dentro do terminal. O histórico só acrescenta, então o número de uma entrada não muda.
    HistoryUndo(u32),
}

impl Action {
    /// Nome exato mais o argumento numérico: `history_undo` exige `n >= 1`, e `preset_apply` recusa qualquer `n` (argumento que a
    /// ação não definiu é recusado, não ignorado).
    pub fn parse(name: &str, n: Option<u32>) -> Result<Self, CmdError> {
        match (name, n) {
            ("preset_apply", _) => Ok(Self::PresetApply),
            ("history_undo", Some(n)) => Ok(Self::HistoryUndo(n)),
            ("history_undo", _) => {
                Err(CmdError::new("invalid_argument", format!("argumento inválido para a ação {name:?}: {n:?}")))
            }
            _ => Err(CmdError::new("not_allowed", format!("ação não permitida: {name:?}"))),
        }
    }

    /// Os argumentos fixos. No `PresetApply`, o caminho do arquivo é acrescentado depois, por `run`.
    pub fn args(self) -> Vec<String> {
        match self {
            Self::PresetApply => vec!["preset".into(), "apply".into()],
            Self::HistoryUndo(n) => vec!["history".into(), "undo".into(), n.to_string()],
        }
    }
}

/// Os terminais que a GUI sabe abrir: só o que foi **medido** (ADR 0027). As flags de `-e` diferem entre emuladores, então cada
/// um novo entra aqui depois de medido; qualquer outro cai no fallback (mostrar o comando).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Terminal {
    /// Medido em 2026-11-19 (konsole 26.08.3): `-e <cmd> <args...>` entrega os argumentos intactos, com tty e o ambiente herdado;
    /// o processo do `konsole` só termina quando a janela fecha.
    Konsole,
}

impl Terminal {
    const ALL: [Terminal; 1] = [Terminal::Konsole];

    fn name(self) -> &'static str {
        match self {
            Self::Konsole => "konsole",
        }
    }

    fn from_file_name(name: Option<&OsStr>) -> Option<Self> {
        let name = name?.to_str()?;
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    /// O que vem antes do comando a executar.
    fn before_command(self) -> &'static [&'static str] {
        match self {
            Self::Konsole => &["-e"],
        }
    }
}

/// Arquivo comum com bit de execução (segue symlink).
fn is_executable_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// `NODUS_TERMINAL` (caminho absoluto, de um arquivo executável cujo nome está na tabela) ou, sem ela, a busca no `PATH` só dos
/// nomes da tabela (entradas relativas do `PATH` são ignoradas: apontariam para o diretório atual). `Ok(None)` é o fallback.
fn find_terminal(env_value: Option<OsString>, path: Option<OsString>) -> Result<Option<(PathBuf, Terminal)>, CmdError> {
    if let Some(value) = env_value {
        let candidate = PathBuf::from(value);
        let known = Terminal::from_file_name(candidate.file_name());
        return match known {
            Some(terminal) if candidate.is_absolute() && is_executable_file(&candidate) => Ok(Some((candidate, terminal))),
            _ => Err(CmdError::new(
                "invalid_terminal",
                format!(
                    "NODUS_TERMINAL precisa ser o caminho absoluto de um terminal conhecido ({}) que exista e seja executável (recebi {:?})",
                    Terminal::ALL.map(Terminal::name).join(", "),
                    candidate.as_os_str()
                ),
            )),
        };
    }
    let Some(path) = path else { return Ok(None) };
    for dir in std::env::split_paths(&path).filter(|d| d.is_absolute()) {
        for terminal in Terminal::ALL {
            let candidate = dir.join(terminal.name());
            if is_executable_file(&candidate) {
                return Ok(Some((candidate, terminal)));
            }
        }
    }
    Ok(None)
}

/// O que o backend lê do ambiente (só aqui, no Rust; o frontend nunca fornece nada disto).
#[derive(Debug)]
pub struct Env {
    pub binary: PathBuf,
    /// Valor cru de `NODUS_PRESET`.
    pub preset: Option<OsString>,
    /// Valor cru de `NODUS_TERMINAL`.
    pub terminal: Option<OsString>,
    /// Valor cru de `PATH`.
    pub path: Option<OsString>,
}

/// O resultado de uma ação. `launched: false` não é erro: não há terminal conhecido, e `command` é o que o dono cola num terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outcome {
    pub launched: bool,
    pub command: String,
}

/// Escreve o comando para **mostrar** ao dono, com aspas simples quando preciso. Nunca é executado: o que roda é o `argv`.
pub fn display_command(parts: &[OsString]) -> String {
    fn quote(part: &OsStr) -> String {
        let text = part.to_string_lossy();
        let plain = !text.is_empty()
            && text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | ':' | '=' | '@' | '%' | '+' | ',' | '-'));
        if plain { text.into_owned() } else { format!("'{}'", text.replace('\'', "'\\''")) }
    }
    parts.iter().map(|p| quote(p)).collect::<Vec<_>>().join(" ")
}

/// Executa uma ação da lista: abre o terminal com o comando da CLI e **não espera** (o processo do terminal vive enquanto a janela
/// existir). Ordem: lista, arquivo do preset, terminal, versão da CLI, e só então o processo. Falta de terminal devolve o comando.
pub fn run(name: &str, n: Option<u32>, env: Env, gui_version: (u64, u64, u64)) -> Result<Outcome, CmdError> {
    let action = Action::parse(name, n)?;
    let mut args: Vec<OsString> = action.args().into_iter().map(OsString::from).collect();
    match action {
        Action::PresetApply => args.push(resolve_preset(env.preset)?.into_os_string()),
        // Não usa preset: `NODUS_PRESET` ausente ou ruim não pode atrapalhar.
        Action::HistoryUndo(_) => {}
    }
    let terminal = find_terminal(env.terminal, env.path)?;
    check_version(&env.binary, gui_version, DEFAULT_TIMEOUT)?;

    let mut parts = vec![env.binary.clone().into_os_string()];
    parts.extend(args.iter().cloned());
    let command = display_command(&parts);
    let Some((terminal_path, terminal)) = terminal else {
        return Ok(Outcome { launched: false, command });
    };

    // Sem shell na nossa mão: lista de argumentos. O `sh -c` é do invólucro constante e recebe os dados como parâmetros.
    let mut child = Command::new(&terminal_path)
        .args(terminal.before_command())
        .args(["sh", "-c", WRAPPER, "sh"])
        .args(&parts)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| CmdError::new("launch_failed", format!("não consegui abrir o terminal {}: {e}", terminal_path.display())))?;
    // Recolhe o processo quando a janela fechar (sem zumbi), sem segurar a GUI.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(Outcome { launched: true, command })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nodusctl::tests::EXEC;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, Instant};

    const GUI: (u64, u64, u64) = (0, 1, 0);
    static CONTADOR: AtomicU32 = AtomicU32::new(0);

    fn pasta() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nodus-acao-teste-{}-{}",
            std::process::id(),
            CONTADOR.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Cria um executável. Serializado com o `EXEC` dos testes de leitura: sem isso o `fork` de outro teste herda o descritor de
    /// escrita ainda aberto e o `exec` recebe ETXTBSY ("text file busy").
    fn executavel(path: &Path, corpo: &str) {
        let _guard = EXEC.lock().unwrap_or_else(|e| e.into_inner());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("#!/bin/sh\n{corpo}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// `nodusctl` de mentira: responde `--version`, grava toda chamada em `chamadas` e os demais argumentos (um por linha) em
    /// `nodusctl.args`.
    fn nodusctl_falso(dir: &Path, versao: &str) -> PathBuf {
        let bin = dir.join("nodusctl");
        executavel(
            &bin,
            &format!(
                "echo \"$*\" >> '{d}/chamadas'\nif [ \"$1\" = \"--version\" ]; then echo \"nodusctl {versao}\"; exit 0; fi\n\
                 for a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{d}/nodusctl.args'\nexit 0",
                d = dir.display()
            ),
        );
        bin
    }

    /// Terminal de mentira que só grava os argumentos (um por linha, e `FIM` no fim) em `term.args`.
    fn terminal_que_grava(dir: &Path) -> PathBuf {
        let term = dir.join("term").join("konsole");
        executavel(
            &term,
            &format!("for a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{d}/term.args'\necho FIM >> '{d}/term.args'", d = dir.display()),
        );
        term
    }

    /// Terminal de mentira que se comporta como um de verdade: executa o que vem depois do `-e` (o invólucro real roda num `sh`).
    fn terminal_que_executa(dir: &Path) -> PathBuf {
        let term = dir.join("term").join("konsole");
        executavel(&term, &format!("if [ \"$1\" = \"-e\" ]; then shift; \"$@\"; fi\necho FIM > '{d}/term.fim'", d = dir.display()));
        term
    }

    fn chamadas(dir: &Path) -> Vec<String> {
        fs::read_to_string(dir.join("chamadas")).map(|t| t.lines().map(String::from).collect()).unwrap_or_default()
    }

    /// Espera (até 5 s) o arquivo existir e satisfazer `pronto`; devolve o conteúdo.
    fn esperar(path: &Path, pronto: impl Fn(&str) -> bool) -> String {
        let limite = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(texto) = fs::read_to_string(path)
                && pronto(&texto)
            {
                return texto;
            }
            assert!(Instant::now() < limite, "{} não ficou pronto em 5 s", path.display());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn argumentos_do_terminal(dir: &Path) -> Vec<String> {
        let texto = esperar(&dir.join("term.args"), |t| t.ends_with("FIM\n"));
        let mut linhas: Vec<String> = texto.lines().map(String::from).collect();
        linhas.pop();
        linhas
    }

    fn nada_lancado(dir: &Path) {
        std::thread::sleep(Duration::from_millis(300));
        assert!(!dir.join("term.args").exists() && !dir.join("term.fim").exists(), "o terminal não podia ser lançado");
    }

    fn env(bin: &Path, preset: Option<&str>, terminal: Option<&Path>, path: Option<&Path>) -> Env {
        Env {
            binary: bin.to_path_buf(),
            preset: preset.map(OsString::from),
            terminal: terminal.map(|t| t.as_os_str().to_owned()),
            path: path.map(|p| p.as_os_str().to_owned()),
        }
    }

    /// Ação sem argumento vindo da tela (`preset_apply`).
    fn chamar(name: &str, env: Env) -> Result<Outcome, crate::nodusctl::CmdError> {
        chamar_n(name, None, env)
    }

    /// Ação com o argumento numérico da tela (`history_undo`).
    fn chamar_n(name: &str, n: Option<u32>, env: Env) -> Result<Outcome, crate::nodusctl::CmdError> {
        let _guard = EXEC.lock().unwrap_or_else(|e| e.into_inner());
        run(name, n, env, GUI)
    }

    // --- a lista de ações ---

    #[test]
    fn so_o_nome_exato_da_lista_fechada() {
        assert_eq!(Action::parse("preset_apply", None), Ok(Action::PresetApply));
        assert_eq!(Action::PresetApply.args(), vec!["preset", "apply"]);
        for nome in [
            "preset apply", "preset_apply /tmp/p.toml", "preset_apply --yes", "preset_apply --noconfirm", "Preset_apply", "PRESET_APPLY",
            "preset_apply ", " preset_apply", "preset_apply\n", "preset_apply;ls", "preset-apply", "preset_applyx", "preset_applys",
            "history undo 1", "history_undo 1", "history_undo;ls", "History_undo", "history-undo", "history_undos", "history_undo ",
            "dotfiles_adopt", "workspace_set", "preset_adopt", "preset_plan", "history", "inventory", "",
        ] {
            let erro = Action::parse(nome, None).unwrap_err();
            assert_eq!(erro.kind, "not_allowed", "{nome:?} não pode passar: a lista de ações é fechada");
        }
    }

    #[test]
    fn a_lista_de_leitura_e_a_de_acoes_nao_se_misturam() {
        assert_eq!(crate::nodusctl::Allowed::parse("preset_apply"), None, "ação não pode entrar pela porta da leitura");
        assert_eq!(crate::nodusctl::Allowed::parse("history_undo"), None, "ação não pode entrar pela porta da leitura");
        assert_eq!(Action::parse("preset_plan", None).unwrap_err().kind, "not_allowed", "leitura não pode entrar pela porta das ações");
        assert_eq!(Action::parse("history", None).unwrap_err().kind, "not_allowed", "leitura não pode entrar pela porta das ações");
    }

    // --- o argumento numérico vindo da tela (a única forma de argumento que a tela pode mandar) ---

    #[test]
    fn history_undo_so_com_n_inteiro_a_partir_de_1() {
        assert_eq!(Action::parse("history_undo", Some(1)), Ok(Action::HistoryUndo(1)));
        assert_eq!(Action::parse("history_undo", Some(u32::MAX)), Ok(Action::HistoryUndo(u32::MAX)));
        assert_eq!(Action::HistoryUndo(7).args(), vec!["history", "undo", "7"]);
        for n in [None, Some(0)] {
            let erro = Action::parse("history_undo", n).unwrap_err();
            assert_eq!(erro.kind, "invalid_argument", "{n:?}: o `undo` sem número válido nem chega à CLI");
        }
    }

    #[test]
    fn so_o_history_undo_aceita_argumento() {
        // Um `n` solto num `preset_apply` seria argumento que a ação não definiu: recusado, não ignorado.
        for n in [0, 1, 3] {
            let erro = Action::parse("preset_apply", Some(n)).unwrap_err();
            assert_eq!(erro.kind, "invalid_argument", "{n}");
        }
    }

    // --- o que vira processo ---

    #[test]
    fn lanca_o_terminal_com_o_argv_exato() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = terminal_que_grava(&dir);
        let saida = chamar("preset_apply", env(&bin, Some("/tmp/p.toml"), Some(&term), None)).unwrap();
        assert!(saida.launched);
        assert!(!WRAPPER.contains('\n'), "uma linha só, para o teste enxergar o argumento inteiro");
        assert_eq!(
            argumentos_do_terminal(&dir),
            ["-e", "sh", "-c", WRAPPER, "sh", bin.to_str().unwrap(), "preset", "apply", "/tmp/p.toml"]
        );
        assert_eq!(saida.command, format!("{} preset apply /tmp/p.toml", bin.display()));
        assert_eq!(chamadas(&dir), ["--version"], "a GUI só confere a versão; o `apply` roda dentro do terminal");
    }

    #[test]
    fn caminho_com_espaco_vai_como_um_argumento_so() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = terminal_que_grava(&dir);
        let saida = chamar("preset_apply", env(&bin, Some("/tmp/minha pasta/p.toml"), Some(&term), None)).unwrap();
        let args = argumentos_do_terminal(&dir);
        assert_eq!(args.len(), 9);
        assert_eq!(args[8], "/tmp/minha pasta/p.toml");
        assert!(saida.command.ends_with("preset apply '/tmp/minha pasta/p.toml'"), "{}", saida.command);
    }

    #[test]
    fn o_involucro_e_constante_e_nao_recebe_dado_no_texto() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = terminal_que_grava(&dir);
        chamar("preset_apply", env(&bin, Some("/tmp/um.toml"), Some(&term), None)).unwrap();
        let primeiro = argumentos_do_terminal(&dir);
        fs::remove_file(dir.join("term.args")).unwrap();
        chamar("preset_apply", env(&bin, Some("/tmp/outro caminho/dois.toml"), Some(&term), None)).unwrap();
        let segundo = argumentos_do_terminal(&dir);
        assert_eq!(primeiro[3], WRAPPER);
        assert_eq!(segundo[3], WRAPPER, "o texto do script não pode mudar com o dado");
        assert!(WRAPPER.contains("\"$@\""), "os dados entram só por parâmetro posicional");
    }

    #[test]
    fn caminho_hostil_chega_ao_nodusctl_como_dado_pelo_involucro_real() {
        // O terminal falso executa o comando de verdade, então o `sh -c` constante roda. Um caminho com aspas e `;` precisa chegar
        // ao `nodusctl` falso, byte a byte, como um argumento, e o `touch` embutido nele não pode acontecer.
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = terminal_que_executa(&dir);
        let hostil = format!("{d}/a'; touch {d}/PWNED; echo '.toml", d = dir.display());
        chamar("preset_apply", env(&bin, Some(&hostil), Some(&term), None)).unwrap();
        esperar(&dir.join("term.fim"), |t| t.contains("FIM"));
        let recebido = esperar(&dir.join("nodusctl.args"), |t| t.ends_with('\n'));
        assert_eq!(recebido.lines().collect::<Vec<_>>(), ["preset", "apply", hostil.as_str()]);
        assert!(!dir.join("PWNED").exists(), "o texto do caminho virou comando");
    }

    #[test]
    fn a_gui_nao_espera_o_terminal_acabar() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = dir.join("term").join("konsole");
        executavel(&term, &format!("sleep 2\necho FIM > '{d}/term.fim'", d = dir.display()));
        let inicio = Instant::now();
        let saida = chamar("preset_apply", env(&bin, Some("/tmp/p.toml"), Some(&term), None)).unwrap();
        assert!(saida.launched);
        assert!(inicio.elapsed() < Duration::from_millis(1500), "o konsole fica vivo enquanto a janela existe: não dá para esperar");
        esperar(&dir.join("term.fim"), |t| t.contains("FIM"));
    }

    // --- o que NÃO vira processo ---

    #[test]
    fn sem_preset_valido_nada_executa() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = terminal_que_grava(&dir);
        let erro = chamar("preset_apply", env(&bin, None, Some(&term), None)).unwrap_err();
        assert_eq!(erro.kind, "preset_not_set");
        assert!(erro.message.contains("NODUS_PRESET"), "{}", erro.message);
        for valor in ["", "p.toml", "./p.toml", "../p.toml", "~/p.toml", "-p.toml", "--yes"] {
            let erro = chamar("preset_apply", env(&bin, Some(valor), Some(&term), None)).unwrap_err();
            assert_eq!(erro.kind, "invalid_preset", "{valor:?}");
        }
        assert!(chamadas(&dir).is_empty(), "nem o `--version` pode rodar");
        nada_lancado(&dir);
    }

    #[test]
    fn nome_fora_da_lista_nada_executa() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = terminal_que_grava(&dir);
        for nome in ["preset_apply --yes", "history_undo 3", "history undo", "inventory", "Preset_apply", ""] {
            let erro = chamar(nome, env(&bin, Some("/tmp/p.toml"), Some(&term), None)).unwrap_err();
            assert_eq!(erro.kind, "not_allowed", "{nome:?}");
        }
        assert!(chamadas(&dir).is_empty());
        nada_lancado(&dir);
    }

    #[test]
    fn terminal_fora_da_tabela_ou_relativo_e_recusado_e_nada_executa() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let intruso = dir.join("xterm");
        executavel(&intruso, &format!("echo RODOU > '{d}/term.fim'", d = dir.display()));
        for valor in ["", "konsole", "./konsole", "/bin/sh", intruso.to_str().unwrap(), "/tmp/konsole; ls", "/tmp/konsole2", "/nao/existe/konsole"] {
            let e = Env {
                binary: bin.clone(),
                preset: Some("/tmp/p.toml".into()),
                terminal: Some(valor.into()),
                path: None,
            };
            let erro = chamar("preset_apply", e).unwrap_err();
            assert_eq!(erro.kind, "invalid_terminal", "{valor:?}");
        }
        assert!(chamadas(&dir).is_empty(), "nem o `--version` pode rodar com NODUS_TERMINAL ruim");
        nada_lancado(&dir);
    }

    #[test]
    fn versao_incompativel_nada_executa() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.2.0");
        let term = terminal_que_grava(&dir);
        let erro = chamar("preset_apply", env(&bin, Some("/tmp/p.toml"), Some(&term), None)).unwrap_err();
        assert_eq!(erro.kind, "version_mismatch");
        nada_lancado(&dir);
    }

    // --- achar o terminal, e o fallback ---

    #[test]
    fn sem_terminal_o_resultado_e_o_comando_para_colar_e_nada_e_lancado() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let vazio = pasta();
        for path in [Some(vazio.as_path()), None] {
            let saida = chamar("preset_apply", env(&bin, Some("/tmp/p.toml"), None, path)).unwrap();
            assert!(!saida.launched);
            assert_eq!(saida.command, format!("{} preset apply /tmp/p.toml", bin.display()));
        }
        assert_eq!(chamadas(&dir), ["--version", "--version"], "a versão é conferida mesmo no fallback");
        nada_lancado(&dir);
    }

    #[test]
    fn acha_o_konsole_no_path_e_ignora_o_que_nao_e_executavel() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let sem_permissao = pasta();
        fs::write(sem_permissao.join("konsole"), "#!/bin/sh\n").unwrap();
        fs::set_permissions(sem_permissao.join("konsole"), fs::Permissions::from_mode(0o644)).unwrap();
        let term = terminal_que_grava(&dir);
        let path = std::env::join_paths([sem_permissao.as_path(), term.parent().unwrap()]).unwrap();
        let saida = chamar(
            "preset_apply",
            Env { binary: bin, preset: Some("/tmp/p.toml".into()), terminal: None, path: Some(path) },
        )
        .unwrap();
        assert!(saida.launched);
        assert_eq!(argumentos_do_terminal(&dir)[0], "-e");
    }

    #[test]
    fn falha_ao_lancar_e_um_erro_com_nome() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = dir.join("term").join("konsole");
        {
            let _guard = EXEC.lock().unwrap_or_else(|e| e.into_inner());
            fs::create_dir_all(term.parent().unwrap()).unwrap();
            fs::write(&term, "#!/nao/existe/interprete\n").unwrap();
            fs::set_permissions(&term, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let erro = chamar("preset_apply", env(&bin, Some("/tmp/p.toml"), Some(&term), None)).unwrap_err();
        assert_eq!(erro.kind, "launch_failed");
    }

    // --- o comando mostrado ao dono (só texto: nunca é executado) ---

    #[test]
    fn comando_exibido_com_aspas_so_quando_precisa() {
        let partes = |v: &[&str]| v.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(display_command(&partes(&["/usr/bin/nodusctl", "preset", "apply", "/tmp/p.toml"])), "/usr/bin/nodusctl preset apply /tmp/p.toml");
        assert_eq!(display_command(&partes(&["nodusctl", "/tmp/minha pasta/p.toml"])), "nodusctl '/tmp/minha pasta/p.toml'");
        assert_eq!(display_command(&partes(&["nodusctl", "a'b"])), "nodusctl 'a'\\''b'");
        assert_eq!(display_command(&partes(&["nodusctl", "x;y", "$HOME", "`ls`", "a&b", "", "*"])), "nodusctl 'x;y' '$HOME' '`ls`' 'a&b' '' '*'");
    }

    // --- history undo: privilegiada (`sudo pacman -Rs`), argumento `n` vindo da tela ---

    #[test]
    fn history_undo_lanca_o_terminal_com_o_argv_exato() {
        for n in [1_u32, 3, u32::MAX] {
            let dir = pasta();
            let bin = nodusctl_falso(&dir, "0.1.4");
            let term = terminal_que_grava(&dir);
            let saida = chamar_n("history_undo", Some(n), env(&bin, None, Some(&term), None)).unwrap();
            assert!(saida.launched);
            assert_eq!(
                argumentos_do_terminal(&dir),
                ["-e", "sh", "-c", WRAPPER, "sh", bin.to_str().unwrap(), "history", "undo", &n.to_string()]
            );
            assert_eq!(saida.command, format!("{} history undo {n}", bin.display()), "o número em decimal canônico");
            assert_eq!(chamadas(&dir), ["--version"], "a GUI só confere a versão; o `undo` roda dentro do terminal");
        }
    }

    #[test]
    fn history_undo_nao_precisa_nem_le_o_preset() {
        // `NODUS_PRESET` ausente ou ruim não pode atrapalhar uma ação que não usa preset.
        for preset in [None, Some("relativo.toml"), Some("")] {
            let dir = pasta();
            let bin = nodusctl_falso(&dir, "0.1.4");
            let term = terminal_que_grava(&dir);
            let saida = chamar_n("history_undo", Some(2), env(&bin, preset, Some(&term), None)).unwrap();
            assert!(saida.launched, "{preset:?}");
            assert_eq!(argumentos_do_terminal(&dir)[5..], [bin.to_str().unwrap(), "history", "undo", "2"]);
        }
    }

    #[test]
    fn history_undo_sem_n_valido_nada_executa() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = terminal_que_grava(&dir);
        for n in [None, Some(0)] {
            let erro = chamar_n("history_undo", n, env(&bin, Some("/tmp/p.toml"), Some(&term), None)).unwrap_err();
            assert_eq!(erro.kind, "invalid_argument", "{n:?}");
        }
        assert!(chamadas(&dir).is_empty(), "nem o `--version` pode rodar");
        nada_lancado(&dir);
    }

    #[test]
    fn preset_apply_com_n_solto_nada_executa() {
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let term = terminal_que_grava(&dir);
        let erro = chamar_n("preset_apply", Some(1), env(&bin, Some("/tmp/p.toml"), Some(&term), None)).unwrap_err();
        assert_eq!(erro.kind, "invalid_argument");
        assert!(chamadas(&dir).is_empty());
        nada_lancado(&dir);
    }

    #[test]
    fn history_undo_segue_as_mesmas_regras_de_terminal_versao_e_fallback() {
        // Versão incompatível: nada é lançado.
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.2.0");
        let term = terminal_que_grava(&dir);
        let erro = chamar_n("history_undo", Some(1), env(&bin, None, Some(&term), None)).unwrap_err();
        assert_eq!(erro.kind, "version_mismatch");
        nada_lancado(&dir);

        // NODUS_TERMINAL fora da tabela: recusado antes de qualquer processo.
        let dir = pasta();
        let bin = nodusctl_falso(&dir, "0.1.4");
        let erro = chamar_n("history_undo", Some(1), env(&bin, None, Some(Path::new("/bin/sh")), None)).unwrap_err();
        assert_eq!(erro.kind, "invalid_terminal");
        assert!(chamadas(&dir).is_empty());

        // Sem terminal: o comando para colar, e nada lançado.
        let vazio = pasta();
        let saida = chamar_n("history_undo", Some(4), env(&bin, None, None, Some(&vazio))).unwrap();
        assert!(!saida.launched);
        assert_eq!(saida.command, format!("{} history undo 4", bin.display()));
        nada_lancado(&dir);
    }
}
