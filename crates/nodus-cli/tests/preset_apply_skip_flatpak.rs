//! A ligação de `--skip-flatpak` em `main.rs::preset_apply`, pelo binário de verdade.
//!
//! `pacman` e `flatpak` são scripts falsos no `PATH`. Sem terminal (`stdin` nulo) o `apply` mostra o
//! plano e o `Comando:` e para em "precisa de terminal interativo" **antes de executar qualquer
//! coisa**, então o `sudo` nunca roda: o teste enxerga o que seria executado sem executar.
//!
//! As pastas dos fakes são criadas uma vez, antes de qualquer `spawn`, e com o arquivo já fechado:
//! escrever um script enquanto outra thread cria um processo dá `ETXTBSY` ("Text file busy").
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const PACMAN: &str = r#"#!/bin/sh
case "$1" in
  -Q) echo "base 1-1" ;;
  -S) exit 0 ;;
  *) exit 1 ;;
esac
"#;

/// Grava cada chamada em `$FLATPAK_LOG`; `list` e `remotes` não imprimem nada (0 apps, sem remote).
const FLATPAK: &str = r#"#!/bin/sh
echo "$*" >> "$FLATPAK_LOG"
case "$1" in
  list|remotes) exit 0 ;;
  *) exit 1 ;;
esac
"#;

const PRESET: &str = "version = 1\npacman = [\"sl\"]\nflatpak = [\"org.gnome.Calculator\"]\n";

struct Cenario {
    com_flatpak: PathBuf,
    sem_flatpak: PathBuf,
    home: PathBuf,
    preset: PathBuf,
}

fn script(dir: &Path, nome: &str, conteudo: &str) {
    let caminho = dir.join(nome);
    std::fs::write(&caminho, conteudo).unwrap();
    std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn cenario() -> &'static Cenario {
    static CENARIO: OnceLock<Cenario> = OnceLock::new();
    CENARIO.get_or_init(|| {
        let raiz = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("preset-apply-skip-flatpak");
        let _ = std::fs::remove_dir_all(&raiz);
        let com_flatpak = raiz.join("bin-com-flatpak");
        let sem_flatpak = raiz.join("bin-sem-flatpak");
        let home = raiz.join("home");
        for dir in [&com_flatpak, &sem_flatpak, &home] {
            std::fs::create_dir_all(dir).unwrap();
        }
        script(&com_flatpak, "pacman", PACMAN);
        script(&com_flatpak, "flatpak", FLATPAK);
        script(&sem_flatpak, "pacman", PACMAN);
        let preset = raiz.join("preset.toml");
        std::fs::write(&preset, PRESET).unwrap();
        Cenario {
            com_flatpak,
            sem_flatpak,
            home,
            preset,
        }
    })
}

struct Saida {
    sucesso: bool,
    stdout: String,
    stderr: String,
    /// Chamadas que o `flatpak` falso recebeu, uma por linha.
    log_flatpak: String,
}

/// `preset apply <preset> [--skip-flatpak]` sem terminal, com o `PATH` só nos fakes.
fn rodar(nome: &str, com_flatpak: bool, skip_flatpak: bool) -> Saida {
    let c = cenario();
    let bin = if com_flatpak {
        &c.com_flatpak
    } else {
        &c.sem_flatpak
    };
    let log = c.home.join(format!("{nome}.flatpak.log"));
    let _ = std::fs::remove_file(&log);
    let mut comando = Command::new(env!("CARGO_BIN_EXE_nodusctl"));
    comando
        .env_clear()
        .env("PATH", bin)
        .env("HOME", &c.home)
        .env("XDG_STATE_HOME", &c.home)
        .env("FLATPAK_LOG", &log)
        .args(["preset", "apply"])
        .arg(&c.preset);
    if skip_flatpak {
        comando.arg("--skip-flatpak");
    }
    let saida = comando
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    Saida {
        sucesso: saida.status.success(),
        stdout: String::from_utf8_lossy(&saida.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&saida.stderr).into_owned(),
        log_flatpak: std::fs::read_to_string(&log).unwrap_or_default(),
    }
}

const AVISO: &str = "Flatpak pulado por --skip-flatpak";
const PLANO: &str = "Plano (nada foi executado):";
const SO_PACMAN: &str = "Comando: sudo pacman -S --needed -- sl";
const SEM_TERMINAL: &str = "precisa de terminal interativo";
const SEM_FLATPAK: &str = "Flatpak não está instalado";

#[test]
fn com_a_flag_o_aviso_vem_antes_do_plano_e_so_o_pacman_e_oferecido() {
    let s = rodar("com-flag-com-flatpak", true, true);
    assert!(!s.sucesso, "sem terminal o apply recusa: {}", s.stderr);
    assert!(s.stderr.contains(SEM_TERMINAL), "{}", s.stderr);
    let aviso = s
        .stdout
        .find(AVISO)
        .unwrap_or_else(|| panic!("{}", s.stdout));
    let plano = s
        .stdout
        .find(PLANO)
        .unwrap_or_else(|| panic!("{}", s.stdout));
    assert!(aviso < plano, "o aviso vem antes do plano:\n{}", s.stdout);
    assert!(s.stdout.contains("org.gnome.Calculator"), "{}", s.stdout);
    assert!(s.stdout.contains(SO_PACMAN), "{}", s.stdout);
    assert!(!s.stdout.contains("flatpak install"), "{}", s.stdout);
    assert!(
        s.log_flatpak.contains("list"),
        "o fake foi usado: {}",
        s.log_flatpak
    );
    assert!(!s.log_flatpak.contains("remotes"), "{}", s.log_flatpak);
    assert!(!s.log_flatpak.contains("install"), "{}", s.log_flatpak);
}

#[test]
fn com_a_flag_e_sem_flatpak_na_maquina_o_pacman_ainda_e_oferecido() {
    let s = rodar("com-flag-sem-flatpak", false, true);
    assert!(!s.sucesso);
    assert!(s.stderr.contains(SEM_TERMINAL), "{}", s.stderr);
    assert!(!s.stderr.contains(SEM_FLATPAK), "{}", s.stderr);
    assert!(s.stdout.contains(AVISO), "{}", s.stdout);
    assert!(s.stdout.contains(SO_PACMAN), "{}", s.stdout);
}

#[test]
fn sem_a_flag_e_sem_flatpak_o_apply_continua_recusando_tudo() {
    let s = rodar("sem-flag-sem-flatpak", false, false);
    assert!(!s.sucesso);
    assert!(s.stderr.contains(SEM_FLATPAK), "{}", s.stderr);
    assert!(!s.stdout.contains("Comando:"), "{}", s.stdout);
    assert!(!s.stdout.contains(AVISO), "{}", s.stdout);
}

#[test]
fn sem_a_flag_o_flathub_e_consultado_e_a_falta_dele_para_o_apply() {
    let s = rodar("sem-flag-com-flatpak", true, false);
    assert!(!s.sucesso);
    assert!(s.stderr.contains("flathub"), "{}", s.stderr);
    assert!(s.log_flatpak.contains("remotes"), "{}", s.log_flatpak);
    assert!(!s.stdout.contains("Comando:"), "{}", s.stdout);
}
