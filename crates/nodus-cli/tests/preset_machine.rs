//! `--machine` e as exceções `[machines.<rótulo>]` do preset, pelo binário de verdade.
//!
//! `pacman` e `flatpak` são scripts falsos no `PATH`. Sem terminal o `apply` mostra o plano e o
//! `Comando:` e para antes de executar, então o `sudo` nunca roda. O rótulo é sempre passado com
//! `--machine`: o hostname do runner não é controlável, e só vale quando o preset tem tabelas.
//!
//! As pastas dos fakes são criadas uma vez, antes de qualquer `spawn` (`ETXTBSY`, ver
//! `preset_apply_skip_flatpak.rs`).
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const PACMAN: &str = r#"#!/bin/sh
case "$1" in
  -Q) echo "base 1-1" ;;
  -Qe) printf 'sl 1-1\nsteam 1-1\n' ;;
  -Qem) exit 0 ;;
  -S) exit 0 ;;
  *) exit 1 ;;
esac
"#;

const FLATPAK: &str = r#"#!/bin/sh
case "$1" in
  list|remotes) exit 0 ;;
  *) exit 1 ;;
esac
"#;

const COM_TABELAS: &str = "version = 1\n\
    pacman = [\"sl\", \"steam\"]\n\
    flatpak = []\n\
    \n\
    [machines.notebook]\n\
    skip_pacman = [\"steam\"]\n";

const SEM_TABELAS: &str = "version = 1\npacman = [\"sl\", \"steam\"]\nflatpak = []\n";

/// `desktop` acrescenta `steam`; `notebook` pula `sl`; `n2` faz os dois. `sl` e `steam` são os
/// dois instalados de propósito no `pacman` falso (`-Qe`).
const COM_ADD: &str = "version = 1\n\
    pacman = [\"sl\"]\n\
    flatpak = []\n\
    \n\
    [machines.desktop]\n\
    add_pacman = [\"steam\"]\n\
    \n\
    [machines.notebook]\n\
    skip_pacman = [\"sl\"]\n\
    \n\
    [machines.n2]\n\
    skip_pacman = [\"sl\"]\n\
    add_pacman = [\"steam\"]\n";

const ADD_REPETIDO: &str = "version = 1\n\
    pacman = [\"sl\", \"steam\"]\n\
    flatpak = []\n\
    \n\
    [machines.desktop]\n\
    add_pacman = [\"steam\"]\n";

const TIPO_ERRADO: &str = "version = 1\n\
    pacman = [\"sl\", \"steam\"]\n\
    flatpak = []\n\
    \n\
    [machines.notebook]\n\
    skip_pacman = [\"stam\"]\n";

struct Cenario {
    bin: PathBuf,
    home: PathBuf,
}

fn script(dir: &Path, nome: &str, conteudo: &str) {
    let caminho = dir.join(nome);
    std::fs::write(&caminho, conteudo).unwrap();
    std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn cenario() -> &'static Cenario {
    static CENARIO: OnceLock<Cenario> = OnceLock::new();
    CENARIO.get_or_init(|| {
        let raiz = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("preset-machine");
        let _ = std::fs::remove_dir_all(&raiz);
        let bin = raiz.join("bin");
        let home = raiz.join("home");
        for dir in [&bin, &home] {
            std::fs::create_dir_all(dir).unwrap();
        }
        script(&bin, "pacman", PACMAN);
        script(&bin, "flatpak", FLATPAK);
        for (nome, texto) in [
            ("com-tabelas.toml", COM_TABELAS),
            ("sem-tabelas.toml", SEM_TABELAS),
            ("tipo-errado.toml", TIPO_ERRADO),
            ("com-add.toml", COM_ADD),
            ("add-repetido.toml", ADD_REPETIDO),
        ] {
            std::fs::write(raiz.join(nome), texto).unwrap();
        }
        Cenario { bin, home }
    })
}

struct Saida {
    sucesso: bool,
    stdout: String,
    stderr: String,
}

/// `nodusctl preset <acao> <arquivo> <extra...>` sem terminal, com o `PATH` só nos fakes.
fn rodar(acao: &str, arquivo: &str, extra: &[&str]) -> Saida {
    let c = cenario();
    let preset = c.home.parent().unwrap().join(arquivo);
    let saida = Command::new(env!("CARGO_BIN_EXE_nodusctl"))
        .env_clear()
        .env("PATH", &c.bin)
        .env("HOME", &c.home)
        .env("XDG_STATE_HOME", &c.home)
        .env("XDG_CONFIG_HOME", &c.home)
        .args(["preset", acao])
        .arg(&preset)
        .args(extra)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    Saida {
        sucesso: saida.status.success(),
        stdout: String::from_utf8_lossy(&saida.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&saida.stderr).into_owned(),
    }
}

fn faltando(saida: &Saida) -> Vec<String> {
    let json: serde_json::Value = serde_json::from_str(&saida.stdout)
        .unwrap_or_else(|e| panic!("JSON inválido ({e}): {}", saida.stdout));
    json["pacman_missing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn plan_na_maquina_com_tabela_nao_pede_o_item_pulado() {
    let s = rodar(
        "plan",
        "com-tabelas.toml",
        &["--machine", "notebook", "--json"],
    );
    assert!(s.sucesso, "{}", s.stderr);
    assert_eq!(faltando(&s), ["sl"]);
}

#[test]
fn plan_em_outra_maquina_pede_tudo() {
    let s = rodar(
        "plan",
        "com-tabelas.toml",
        &["--machine", "desktop", "--json"],
    );
    assert!(s.sucesso, "{}", s.stderr);
    assert_eq!(faltando(&s), ["sl", "steam"]);
}

#[test]
fn apply_so_oferece_o_que_nao_foi_pulado_e_diz_o_que_pulou() {
    let s = rodar("apply", "com-tabelas.toml", &["--machine", "notebook"]);
    assert!(!s.sucesso, "sem terminal o apply recusa: {}", s.stderr);
    assert!(
        s.stderr.contains("precisa de terminal interativo"),
        "{}",
        s.stderr
    );
    assert!(
        s.stdout
            .contains("Comando: sudo pacman -S --needed -- sl\n"),
        "{}",
        s.stdout
    );
    let aviso = s
        .stdout
        .lines()
        .find(|l| l.contains("pulados nesta máquina"))
        .unwrap_or_else(|| panic!("falta o aviso dos pulados:\n{}", s.stdout));
    assert!(aviso.contains("notebook"), "{aviso}");
    assert!(aviso.contains("steam"), "{aviso}");
    let comando = s
        .stdout
        .lines()
        .find(|l| l.starts_with("Comando:"))
        .unwrap();
    assert!(!comando.contains("steam"), "{comando}");
}

#[test]
fn apply_em_outra_maquina_oferece_tudo_e_nao_avisa_pulados() {
    let s = rodar("apply", "com-tabelas.toml", &["--machine", "desktop"]);
    assert!(!s.sucesso);
    assert!(
        s.stdout
            .contains("Comando: sudo pacman -S --needed -- sl steam\n"),
        "{}",
        s.stdout
    );
    assert!(!s.stdout.contains("pulados nesta máquina"), "{}", s.stdout);
}

#[test]
fn suggest_usa_o_preset_efetivo_da_maquina() {
    // `steam` está instalado de propósito. No notebook ele é pulado, então está fora do preset
    // efetivo e é sugerido; no desktop o preset o tem, então não há o que sugerir.
    let s = rodar("suggest", "com-tabelas.toml", &["--machine", "notebook"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert!(s.stdout.contains("steam"), "{}", s.stdout);
    let s = rodar("suggest", "com-tabelas.toml", &["--machine", "desktop"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert!(!s.stdout.contains("steam"), "{}", s.stdout);
}

#[test]
fn rotulo_invalido_em_machine_e_recusado_antes_de_qualquer_coisa() {
    let s = rodar("plan", "com-tabelas.toml", &["--machine", "Notebook"]);
    assert!(!s.sucesso);
    assert!(s.stderr.contains("rótulo inválido"), "{}", s.stderr);
    assert!(s.stdout.is_empty(), "{}", s.stdout);
}

#[test]
fn skip_de_item_que_nao_esta_no_preset_derruba_o_comando_todo() {
    // Mesmo numa máquina sem tabela: o arquivo inteiro é validado, não só a sua parte.
    let s = rodar("plan", "tipo-errado.toml", &["--machine", "desktop"]);
    assert!(!s.sucesso);
    assert!(s.stderr.contains("stam"), "{}", s.stderr);
    assert!(s.stdout.is_empty(), "{}", s.stdout);
}

#[test]
fn preset_sem_tabelas_nao_depende_de_rotulo_nem_do_hostname() {
    // Sem `--machine` e sem `[machines]`, o comportamento é o de antes: nada de ler o hostname.
    let s = rodar("plan", "sem-tabelas.toml", &["--json"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert_eq!(faltando(&s), ["sl", "steam"]);
}

// Acréscimos por máquina (`add_*`, Fase 2, fatia 4).

fn linha_com<'a>(saida: &'a Saida, trecho: &str) -> &'a str {
    saida
        .stdout
        .lines()
        .find(|l| l.contains(trecho))
        .unwrap_or_else(|| panic!("falta a linha com {trecho:?}:\n{}", saida.stdout))
}

#[test]
fn plan_inclui_o_que_a_maquina_acrescenta() {
    for (maquina, esperado) in [
        ("desktop", vec!["sl", "steam"]),
        ("n2", vec!["steam"]),
        ("outra-maquina", vec!["sl"]),
    ] {
        let s = rodar("plan", "com-add.toml", &["--machine", maquina, "--json"]);
        assert!(s.sucesso, "{maquina}: {}", s.stderr);
        assert_eq!(faltando(&s), esperado, "{maquina}");
    }
}

#[test]
fn apply_oferece_o_acrescentado_e_diz_o_que_acrescentou() {
    let s = rodar("apply", "com-add.toml", &["--machine", "desktop"]);
    assert!(!s.sucesso, "sem terminal o apply recusa: {}", s.stderr);
    assert!(
        s.stdout
            .contains("Comando: sudo pacman -S --needed -- sl steam\n"),
        "{}",
        s.stdout
    );
    let aviso = linha_com(&s, "acrescentados nesta máquina");
    assert!(
        aviso.contains("desktop") && aviso.contains("steam"),
        "{aviso}"
    );
    assert!(!s.stdout.contains("pulados nesta máquina"), "{}", s.stdout);
}

#[test]
fn apply_com_skip_e_add_na_mesma_maquina_avisa_os_dois() {
    let s = rodar("apply", "com-add.toml", &["--machine", "n2"]);
    assert!(!s.sucesso);
    assert!(
        s.stdout
            .contains("Comando: sudo pacman -S --needed -- steam\n"),
        "{}",
        s.stdout
    );
    let pulados = linha_com(&s, "pulados nesta máquina");
    assert!(
        pulados.contains("n2") && pulados.contains("sl"),
        "{pulados}"
    );
    let acrescentados = linha_com(&s, "acrescentados nesta máquina");
    assert!(
        acrescentados.contains("n2") && acrescentados.contains("steam"),
        "{acrescentados}"
    );
}

#[test]
fn maquina_que_so_pula_nao_avisa_acrescentados() {
    let s = rodar("apply", "com-add.toml", &["--machine", "notebook"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert!(s.stdout.contains("Nada a fazer"), "{}", s.stdout);
    assert!(
        !s.stdout.contains("acrescentados nesta máquina"),
        "{}",
        s.stdout
    );
}

#[test]
fn suggest_trata_o_acrescentado_como_conhecido() {
    // `steam` está instalado de propósito. No desktop ele é acrescentado: nada a sugerir. Em outra
    // máquina está fora do preset efetivo: é sugerido.
    let s = rodar("suggest", "com-add.toml", &["--machine", "desktop"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert!(!s.stdout.contains("steam"), "{}", s.stdout);
    let s = rodar("suggest", "com-add.toml", &["--machine", "outra-maquina"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert!(s.stdout.contains("steam"), "{}", s.stdout);
}

#[test]
fn adopt_com_a_maquina_nao_oferece_o_que_ela_acrescenta() {
    let s = rodar("adopt", "com-add.toml", &["--machine", "desktop"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert!(s.stdout.contains("Nada fora do preset"), "{}", s.stdout);
}

#[test]
fn adopt_nao_oferece_o_que_ja_esta_na_lista_compartilhada_mesmo_pulado() {
    // Em `n2`, `sl` é pulado e está instalado: fora do preset efetivo, mas já na lista principal.
    // Oferecer seria mentir (`add_to_preset` ignora o que já está lá). `steam` é acrescentado.
    let s = rodar("adopt", "com-add.toml", &["--machine", "n2"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert!(s.stdout.contains("Nada fora do preset"), "{}", s.stdout);
}

#[test]
fn adopt_em_outra_maquina_ainda_oferece_o_que_esta_fora_do_preset() {
    // `steam` só existe em `[machines.desktop]`: aqui está fora do efetivo e da lista principal.
    let s = rodar("adopt", "com-add.toml", &["--machine", "outra-maquina"]);
    assert!(!s.sucesso);
    assert!(s.stderr.contains("terminal interativo"), "{}", s.stderr);
}

#[test]
fn acrescentar_o_que_ja_esta_na_lista_principal_derruba_o_comando() {
    let s = rodar("plan", "add-repetido.toml", &["--machine", "outra-maquina"]);
    assert!(!s.sucesso);
    assert!(s.stderr.contains("steam"), "{}", s.stderr);
    assert!(
        s.stderr.contains("já está na lista principal"),
        "{}",
        s.stderr
    );
    assert!(s.stdout.is_empty(), "{}", s.stdout);
}
