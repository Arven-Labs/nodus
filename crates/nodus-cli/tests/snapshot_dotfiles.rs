//! Dotfiles no snapshot v2 e no `compare` (Fase 2, fatia 6), pelo binário de verdade.
//!
//! `pacman` e `flatpak` são scripts falsos no `PATH`; `HOME`, a pasta-fonte e o config são de
//! mentira, numa pasta própria de cada teste. As pastas dos fakes são criadas uma vez, antes de
//! qualquer `spawn` (`ETXTBSY`, ver `preset_apply_skip_flatpak.rs`).
#![cfg(unix)]

use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const PACMAN: &str = r#"#!/bin/sh
case "$1" in
  -Qe) echo "base 1-1" ;;
  *) exit 1 ;;
esac
"#;

const FLATPAK: &str = r#"#!/bin/sh
case "$1" in
  list) exit 0 ;;
  *) exit 1 ;;
esac
"#;

const V1_ANTIGO: &str = "{\"version\": 1, \"name\": \"antigo\", \"os\": {\"name\": \"Arch Linux\", \"version\": null}, \
    \"capabilities\": {\"pacman\": true, \"flatpak\": true}, \"pacman\": [\"base\"], \"flatpak\": []}\n";

fn bin() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let bin = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("snapshot-dotfiles-bin");
        let _ = std::fs::remove_dir_all(&bin);
        std::fs::create_dir_all(&bin).unwrap();
        for (nome, conteudo) in [("pacman", PACMAN), ("flatpak", FLATPAK)] {
            let caminho = bin.join(nome);
            std::fs::write(&caminho, conteudo).unwrap();
            std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        bin
    })
}

struct Cenario {
    raiz: PathBuf,
    home: PathBuf,
    fonte: PathBuf,
    config_home: PathBuf,
}

/// Uma pasta nova por teste: HOME, pasta-fonte (dentro do HOME, como o `~/dotfiles` real) e config.
fn cenario(nome: &str, config_extra: &str) -> Cenario {
    let bin = bin();
    let raiz = std::fs::canonicalize(bin.parent().unwrap())
        .unwrap()
        .join(format!("snapshot-dotfiles-{nome}"));
    let _ = std::fs::remove_dir_all(&raiz);
    let home = raiz.join("home");
    let fonte = home.join("dotfiles");
    let config_home = raiz.join("config");
    std::fs::create_dir_all(&fonte).unwrap();
    std::fs::create_dir_all(config_home.join("nodus")).unwrap();
    std::fs::set_permissions(
        config_home.join("nodus"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let config = config_home.join("nodus/config.toml");
    std::fs::write(
        &config,
        format!(
            "version = 1\ndotfiles_dir = {:?}\n{config_extra}",
            fonte.to_str().unwrap()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
    Cenario {
        raiz,
        home,
        fonte,
        config_home,
    }
}

struct Saida {
    sucesso: bool,
    stdout: String,
    stderr: String,
}

fn rodar_com(c: &Cenario, home: Option<&Path>, args: &[&str]) -> Saida {
    let mut comando = Command::new(env!("CARGO_BIN_EXE_nodusctl"));
    comando
        .env_clear()
        .env("PATH", bin())
        .env("XDG_CONFIG_HOME", &c.config_home)
        .env("XDG_STATE_HOME", c.raiz.join("state"))
        .args(args);
    if let Some(home) = home {
        comando.env("HOME", home);
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
    }
}

fn rodar(c: &Cenario, args: &[&str]) -> Saida {
    rodar_com(c, Some(&c.home), args)
}

fn arquivo_do_snapshot(c: &Cenario, rotulo: &str) -> PathBuf {
    c.fonte.join(format!(".nodus/snapshots/{rotulo}.json"))
}

fn json_do_snapshot(c: &Cenario, rotulo: &str) -> serde_json::Value {
    let texto = std::fs::read_to_string(arquivo_do_snapshot(c, rotulo)).unwrap();
    serde_json::from_str(&texto).unwrap()
}

/// `(caminho, estado)` de cada dotfile do snapshot, na ordem do arquivo.
fn estados(json: &serde_json::Value) -> Vec<(String, String)> {
    json["dotfiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["path"].as_str().unwrap().to_string(),
                d["state"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// `.bashrc` ligado à fonte, `.gitconfig` arquivo, `.zshrc` symlink para outro lugar, `.tmux.conf` pasta.
fn montar_home(c: &Cenario) {
    std::fs::write(c.fonte.join(".bashrc"), "# bashrc\n").unwrap();
    symlink(c.fonte.join(".bashrc"), c.home.join(".bashrc")).unwrap();
    std::fs::write(c.home.join(".gitconfig"), "[user]\n").unwrap();
    symlink("/etc/hostname", c.home.join(".zshrc")).unwrap();
    std::fs::create_dir_all(c.home.join(".tmux.conf")).unwrap();
}

#[test]
fn grava_o_estado_de_cada_dotfile_da_lista_e_nada_do_home() {
    let c = cenario("grava", "");
    montar_home(&c);
    let s = rodar(&c, &["snapshot", "--name", "teste"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert!(s.stdout.contains("Snapshot gravado em"), "{}", s.stdout);
    let json = json_do_snapshot(&c, "teste");
    assert_eq!(json["version"], 2);
    assert_eq!(json["capabilities"]["dotfiles"], true);
    let esperado = [
        (".bash_logout", "missing"),
        (".bash_profile", "missing"),
        (".bashrc", "linked"),
        (".config/git/config", "missing"),
        (".gitconfig", "file"),
        (".profile", "missing"),
        (".tmux.conf", "other"),
        (".vimrc", "missing"),
        (".zshrc", "other"),
    ];
    let esperado: Vec<(String, String)> = esperado
        .iter()
        .map(|(p, e)| (p.to_string(), e.to_string()))
        .collect();
    assert_eq!(estados(&json), esperado);
    // O repo pode ser público: nem o HOME, nem o alvo do symlink, nem tamanho ou conteúdo.
    let texto = std::fs::read_to_string(arquivo_do_snapshot(&c, "teste")).unwrap();
    for proibido in [
        c.raiz.to_str().unwrap(),
        "hostname",
        "bashrc\\n",
        "[user]",
        "size",
        "target",
    ] {
        assert!(!texto.contains(proibido), "{proibido:?} vazou:\n{texto}");
    }
    let modo = std::fs::metadata(arquivo_do_snapshot(&c, "teste"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(modo, 0o600);
}

#[test]
fn gravar_de_novo_sem_mudar_nada_da_os_mesmos_bytes() {
    let c = cenario("bytes", "");
    montar_home(&c);
    assert!(rodar(&c, &["snapshot", "--name", "teste"]).sucesso);
    let primeiro = std::fs::read(arquivo_do_snapshot(&c, "teste")).unwrap();
    assert!(rodar(&c, &["snapshot", "--name", "teste"]).sucesso);
    assert_eq!(
        primeiro,
        std::fs::read(arquivo_do_snapshot(&c, "teste")).unwrap()
    );
}

#[test]
fn skip_e_extra_do_config_mudam_a_lista_do_snapshot() {
    let c = cenario(
        "lista",
        "dotfiles_skip = [\".bashrc\"]\ndotfiles_extra = [\".config/foo/conf\"]\n",
    );
    montar_home(&c);
    let s = rodar(&c, &["snapshot", "--name", "teste"]);
    assert!(s.sucesso, "{}", s.stderr);
    let caminhos: Vec<String> = estados(&json_do_snapshot(&c, "teste"))
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    assert!(!caminhos.contains(&".bashrc".to_string()), "{caminhos:?}");
    assert!(
        caminhos.contains(&".config/foo/conf".to_string()),
        "{caminhos:?}"
    );
    assert_eq!(
        caminhos.len(),
        9,
        "8 do padrão (sem .bashrc) + 1 extra: {caminhos:?}"
    );
}

#[test]
fn o_compare_mostra_a_divergencia_de_dotfiles_entre_dois_snapshots() {
    let c = cenario("compare", "");
    montar_home(&c);
    assert!(rodar(&c, &["snapshot", "--name", "um"]).sucesso);
    // O `.bashrc` deixa de ser symlink para a fonte e vira arquivo solto.
    std::fs::remove_file(c.home.join(".bashrc")).unwrap();
    std::fs::write(c.home.join(".bashrc"), "# solto\n").unwrap();
    assert!(rodar(&c, &["snapshot", "--name", "dois"]).sucesso);

    let s = rodar(&c, &["compare", "um", "dois"]);
    assert!(s.sucesso, "divergir não é erro: {}", s.stderr);
    assert!(s.stdout.contains("Dotfiles: 8 em comum\n"), "{}", s.stdout);
    assert!(
        s.stdout
            .contains("  diferem (1):\n    .bashrc: um ligado, dois arquivo\n"),
        "{}",
        s.stdout
    );
    assert!(!s.stdout.contains("Sem divergências"), "{}", s.stdout);

    let j = rodar(&c, &["compare", "um", "dois", "--json"]);
    assert!(j.sucesso, "{}", j.stderr);
    let json: serde_json::Value = serde_json::from_str(&j.stdout).unwrap();
    assert_eq!(json["dotfiles"]["status"], "compared");
    assert_eq!(json["dotfiles"]["same"], 8);
    assert_eq!(
        json["dotfiles"]["differ"],
        serde_json::json!([{"path": ".bashrc", "a": "linked", "b": "file"}])
    );

    let igual = rodar(&c, &["compare", "um", "um"]);
    assert!(igual.sucesso, "{}", igual.stderr);
    assert!(
        igual.stdout.contains("Sem divergências.\n"),
        "{}",
        igual.stdout
    );
}

#[test]
fn o_compare_com_um_snapshot_v1_diz_que_dotfiles_nao_sao_comparaveis() {
    let c = cenario("v1", "");
    montar_home(&c);
    assert!(rodar(&c, &["snapshot", "--name", "teste"]).sucesso);
    // O snapshot que o dono já tem no repo: v1, gravado por um nodus anterior.
    std::fs::write(arquivo_do_snapshot(&c, "antigo"), V1_ANTIGO).unwrap();
    std::fs::set_permissions(
        arquivo_do_snapshot(&c, "antigo"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let s = rodar(&c, &["compare", "antigo", "teste"]);
    assert!(s.sucesso, "{}", s.stderr);
    assert!(
        s.stdout
            .contains("Dotfiles: não comparável, antigo não registrou dotfiles"),
        "{}",
        s.stdout
    );
    assert!(
        s.stdout.contains("nodusctl snapshot"),
        "diz como resolver: {}",
        s.stdout
    );
    let j = rodar(&c, &["compare", "antigo", "teste", "--json"]);
    let json: serde_json::Value = serde_json::from_str(&j.stdout).unwrap();
    assert_eq!(
        json["dotfiles"],
        serde_json::json!({"status": "not_comparable", "lacks": ["antigo"]})
    );
}

#[test]
fn o_preset_status_continua_lendo_snapshots_v1_e_v2() {
    let c = cenario("status", "");
    montar_home(&c);
    assert!(rodar(&c, &["snapshot", "--name", "teste"]).sucesso);
    std::fs::write(arquivo_do_snapshot(&c, "antigo"), V1_ANTIGO).unwrap();
    std::fs::set_permissions(
        arquivo_do_snapshot(&c, "antigo"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let preset = c.raiz.join("preset.toml");
    std::fs::write(
        &preset,
        "version = 1\npacman = [\"base\", \"git\"]\nflatpak = []\n",
    )
    .unwrap();

    let s = rodar(
        &c,
        &["preset", "status", preset.to_str().unwrap(), "--json"],
    );
    assert!(s.sucesso, "{}", s.stderr);
    let json: serde_json::Value = serde_json::from_str(&s.stdout).unwrap();
    let maquinas: Vec<&str> = json["machines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["machine"].as_str().unwrap())
        .collect();
    assert_eq!(maquinas, ["antigo", "teste"]);
    assert_eq!(
        json["unreadable"].as_array().unwrap().len(),
        0,
        "{}",
        s.stdout
    );
}

#[test]
fn sem_home_valido_o_snapshot_erra_e_nao_grava() {
    let c = cenario("sem-home", "");
    montar_home(&c);
    for home in [None, Some(Path::new("relativo"))] {
        let s = rodar_com(&c, home, &["snapshot", "--name", "teste"]);
        assert!(!s.sucesso, "{home:?}");
        assert!(s.stderr.contains("HOME"), "{home:?}: {}", s.stderr);
        assert!(
            !arquivo_do_snapshot(&c, "teste").exists(),
            "{home:?}: não pode gravar um snapshot sem dotfiles"
        );
    }
}
