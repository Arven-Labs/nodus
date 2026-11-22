//! Gravação do snapshot na pasta-fonte dos dotfiles (ADR 0031, fatia 1). Escrita atômica
//! (temporário `create_new` 0600 e `rename`), pastas 0700, e nada de seguir symlink: `.nodus`,
//! `snapshots` e o arquivo de destino precisam ser reais, senão o snapshot (que é do dono) pararia
//! fora do repositório.

use anyhow::{Context, Result, bail};
use nodus_core::snapshot::{Snapshot, to_json};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// Grava `<dir>/.nodus/snapshots/<rótulo>.json` e devolve o caminho. `dir` é a pasta-fonte (já
/// resolvida por quem chama); o rótulo já foi validado pelo core, então não escapa da pasta.
pub fn write(dir: &Path, snapshot: &Snapshot) -> Result<PathBuf> {
    let meta = fs::metadata(dir).with_context(|| format!("lendo {}", dir.display()))?;
    if !meta.is_dir() {
        bail!("{} não é uma pasta", dir.display());
    }
    let nodus = ensure_real_dir(&dir.join(".nodus"))?;
    let snapshots = ensure_real_dir(&nodus.join("snapshots"))?;
    let target = snapshots.join(format!("{}.json", snapshot.name));
    match fs::symlink_metadata(&target) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => bail!(
            "{} existe e não é um arquivo comum (symlink ou pasta); não vou sobrescrever",
            target.display()
        ),
        Err(erro) if erro.kind() == ErrorKind::NotFound => {}
        Err(erro) => return Err(erro).with_context(|| format!("lendo {}", target.display())),
    }

    let tmp = snapshots.join(format!(
        ".{}.json.nodus-{}",
        snapshot.name,
        std::process::id()
    ));
    let outcome = write_and_swap(&tmp, &target, to_json(snapshot).as_bytes());
    if outcome.is_err() {
        // O temporário é nosso (`create_new`): se algo falhar até o `rename`, só ele é removido.
        let _ = fs::remove_file(&tmp);
    }
    outcome?;
    Ok(target)
}

fn write_and_swap(tmp: &Path, target: &Path, data: &[u8]) -> Result<()> {
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(tmp)
        .with_context(|| format!("criando {}", tmp.display()))?;
    out.write_all(data)
        .with_context(|| format!("gravando {}", tmp.display()))?;
    out.sync_all()
        .with_context(|| format!("gravando {} no disco", tmp.display()))?;
    drop(out);
    // `rename` troca o próprio nome: se o destino virou symlink no meio do caminho, é o symlink que
    // é substituído, nunca o alvo dele.
    fs::rename(tmp, target).with_context(|| format!("trocando {}", target.display()))
}

/// A pasta existe e é uma pasta de verdade (não symlink), ou é criada 0700.
fn ensure_real_dir(path: &Path) -> Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => bail!(
            "{} existe e não é uma pasta de verdade (symlink ou arquivo); não vou escrever por ela",
            path.display()
        ),
        Err(erro) if erro.kind() == ErrorKind::NotFound => {
            DirBuilder::new()
                .mode(0o700)
                .create(path)
                .with_context(|| format!("criando {}", path.display()))?;
        }
        Err(erro) => return Err(erro).with_context(|| format!("lendo {}", path.display())),
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    //! Gravar o snapshot na pasta-fonte dos dotfiles (ADR 0031, fatia 1). Tudo em pasta temporária:
    //! nunca toca o HOME real.
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::inventory::OsInfo;
    use nodus_core::preset::{MachinePreset, Preset};
    use nodus_core::snapshot::{Snapshot, build, parse, to_json};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::Path;

    fn snap(nome: &str, pacman: &[&str]) -> Snapshot {
        let os = OsInfo {
            name: "Arch Linux".into(),
            version: None,
        };
        let maquina = MachinePreset {
            preset: Preset {
                pacman: pacman.iter().map(|s| s.to_string()).collect(),
                flatpak: vec![],
            },
            flatpak_installed: false,
        };
        build(nome, &os, &maquina).unwrap()
    }

    fn modo(p: &Path) -> u32 {
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    fn quantos(p: &Path) -> usize {
        fs::read_dir(p).unwrap().count()
    }

    #[test]
    fn grava_em_dot_nodus_snapshots_com_o_conteudo_exato_e_permissoes_fechadas() {
        let tmp = TmpDir::novo();
        let s = snap("meu-pc", &["git", "base"]);
        let caminho = write(tmp.path(), &s).unwrap();
        assert_eq!(caminho, tmp.path().join(".nodus/snapshots/meu-pc.json"));
        assert_eq!(fs::read_to_string(&caminho).unwrap(), to_json(&s));
        // Arquivo do dono, sem leitura para o grupo.
        assert_eq!(modo(&caminho), 0o600);
        assert_eq!(modo(&tmp.path().join(".nodus")), 0o700);
        assert_eq!(modo(&tmp.path().join(".nodus/snapshots")), 0o700);
    }

    #[test]
    fn gravar_duas_vezes_sem_mudanca_da_os_mesmos_bytes() {
        let tmp = TmpDir::novo();
        let s = snap("meu-pc", &["git", "base"]);
        let caminho = write(tmp.path(), &s).unwrap();
        let primeira = fs::read(&caminho).unwrap();
        write(tmp.path(), &s).unwrap();
        assert_eq!(fs::read(&caminho).unwrap(), primeira);
    }

    #[test]
    fn regravar_com_mudanca_substitui_e_nao_deixa_temporario() {
        let tmp = TmpDir::novo();
        write(tmp.path(), &snap("meu-pc", &["base"])).unwrap();
        let novo = snap("meu-pc", &["base", "vim"]);
        let caminho = write(tmp.path(), &novo).unwrap();
        assert_eq!(fs::read_to_string(&caminho).unwrap(), to_json(&novo));
        let nomes: Vec<_> = fs::read_dir(tmp.path().join(".nodus/snapshots"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(nomes, ["meu-pc.json"], "nenhum temporário sobra");
    }

    #[test]
    fn dois_rotulos_dao_dois_arquivos_e_um_nao_toca_o_outro() {
        let tmp = TmpDir::novo();
        let a = write(tmp.path(), &snap("pc-a", &["base"])).unwrap();
        let antes = fs::read(&a).unwrap();
        write(tmp.path(), &snap("pc-b", &["base", "vim"])).unwrap();
        assert_eq!(fs::read(&a).unwrap(), antes);
        assert!(tmp.path().join(".nodus/snapshots/pc-b.json").exists());
    }

    #[test]
    fn pasta_fonte_que_nao_existe_ou_nao_e_pasta_e_recusada_sem_criar_nada() {
        let tmp = TmpDir::novo();
        let inexistente = tmp.path().join("nao-existe");
        assert!(write(&inexistente, &snap("meu-pc", &["base"])).is_err());
        // Não cria a pasta-fonte por conta própria.
        assert!(!inexistente.exists());
        let arquivo = tmp.path().join("arquivo");
        fs::write(&arquivo, "x").unwrap();
        assert!(write(&arquivo, &snap("meu-pc", &["base"])).is_err());
    }

    #[test]
    fn recusa_dot_nodus_ou_snapshots_que_seja_symlink_e_nada_vai_para_fora() {
        let fora = TmpDir::novo();
        // `.nodus` apontando para fora.
        let tmp = TmpDir::novo();
        symlink(fora.path(), tmp.path().join(".nodus")).unwrap();
        assert!(write(tmp.path(), &snap("meu-pc", &["base"])).is_err());
        assert_eq!(quantos(fora.path()), 0, "nada vazou pelo symlink");
        // `.nodus/snapshots` apontando para fora.
        let tmp2 = TmpDir::novo();
        fs::create_dir(tmp2.path().join(".nodus")).unwrap();
        symlink(fora.path(), tmp2.path().join(".nodus/snapshots")).unwrap();
        assert!(write(tmp2.path(), &snap("meu-pc", &["base"])).is_err());
        assert_eq!(quantos(fora.path()), 0);
    }

    #[test]
    fn recusa_destino_que_seja_symlink_e_nao_escreve_no_alvo() {
        let tmp = TmpDir::novo();
        let alvo = tmp.path().join("segredo.txt");
        fs::write(&alvo, "intacto").unwrap();
        fs::create_dir_all(tmp.path().join(".nodus/snapshots")).unwrap();
        symlink(&alvo, tmp.path().join(".nodus/snapshots/meu-pc.json")).unwrap();
        assert!(write(tmp.path(), &snap("meu-pc", &["base"])).is_err());
        assert_eq!(fs::read_to_string(&alvo).unwrap(), "intacto");
    }

    #[test]
    fn recusa_destino_que_seja_pasta() {
        let tmp = TmpDir::novo();
        fs::create_dir_all(tmp.path().join(".nodus/snapshots/meu-pc.json")).unwrap();
        assert!(write(tmp.path(), &snap("meu-pc", &["base"])).is_err());
    }

    #[test]
    fn o_arquivo_gravado_e_lido_de_volta_pela_leitura_estrita() {
        let tmp = TmpDir::novo();
        let s = snap("meu-pc", &["git", "base"]);
        let caminho = write(tmp.path(), &s).unwrap();
        assert_eq!(parse(&fs::read_to_string(caminho).unwrap()).unwrap(), s);
    }

    // --- read: o snapshot de outra máquina é entrada externa (veio por git) ---

    fn pasta_de_snapshots(tmp: &TmpDir) -> std::path::PathBuf {
        let dir = tmp.path().join(".nodus/snapshots");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn le_o_que_write_gravou() {
        let tmp = TmpDir::novo();
        let s = snap("pc-a", &["git", "base"]);
        write(tmp.path(), &s).unwrap();
        assert_eq!(read(tmp.path(), "pc-a").unwrap(), s);
    }

    #[test]
    fn rotulo_invalido_e_recusado_antes_de_montar_o_caminho() {
        let tmp = TmpDir::novo();
        // Um arquivo válido em `.nodus/x.json`: `../x` o alcançaria se o rótulo não fosse validado.
        let s = snap("x", &["base"]);
        fs::create_dir_all(tmp.path().join(".nodus")).unwrap();
        pasta_de_snapshots(&tmp);
        fs::write(tmp.path().join(".nodus/x.json"), to_json(&s)).unwrap();
        for ruim in ["../x", "", "A", "a/b", "..", "/etc/passwd", "x\0"] {
            assert!(read(tmp.path(), ruim).is_err(), "{ruim:?}");
        }
    }

    #[test]
    fn arquivo_ou_pasta_fonte_ausente_e_erro_e_nada_e_criado() {
        let tmp = TmpDir::novo();
        assert!(read(tmp.path(), "pc-a").is_err());
        assert!(!tmp.path().join(".nodus").exists(), "ler não cria pasta");
        assert!(read(&tmp.path().join("nao-existe"), "pc-a").is_err());
        assert!(!tmp.path().join("nao-existe").exists());
    }

    #[test]
    fn recusa_symlink_no_arquivo_e_nas_pastas() {
        let tmp = TmpDir::novo();
        let fora = TmpDir::novo();
        let s = snap("pc-a", &["base"]);
        fs::write(fora.path().join("pc-a.json"), to_json(&s)).unwrap();
        // O arquivo é um symlink para um snapshot válido fora do repositório.
        let snaps = pasta_de_snapshots(&tmp);
        symlink(fora.path().join("pc-a.json"), snaps.join("pc-a.json")).unwrap();
        assert!(read(tmp.path(), "pc-a").is_err());
        // `snapshots` é um symlink para uma pasta de fora.
        let tmp2 = TmpDir::novo();
        fs::create_dir(tmp2.path().join(".nodus")).unwrap();
        symlink(fora.path(), tmp2.path().join(".nodus/snapshots")).unwrap();
        assert!(read(tmp2.path(), "pc-a").is_err());
        // `.nodus` é um symlink.
        let tmp3 = TmpDir::novo();
        symlink(fora.path(), tmp3.path().join(".nodus")).unwrap();
        assert!(read(tmp3.path(), "pc-a").is_err());
    }

    #[test]
    fn recusa_destino_que_e_pasta_e_arquivo_grande_demais() {
        let tmp = TmpDir::novo();
        let snaps = pasta_de_snapshots(&tmp);
        fs::create_dir(snaps.join("pc-a.json")).unwrap();
        assert!(read(tmp.path(), "pc-a").is_err());
        // Entrada externa: um arquivo enorme não é lido inteiro.
        let grande = "x".repeat(MAX_SNAPSHOT_BYTES as usize + 1);
        fs::write(snaps.join("pc-b.json"), grande).unwrap();
        let erro = read(tmp.path(), "pc-b").unwrap_err().to_string();
        assert!(erro.contains("grande"), "{erro}");
    }

    #[test]
    fn recusa_arquivo_cujo_nome_interno_nao_e_o_rotulo() {
        let tmp = TmpDir::novo();
        let snaps = pasta_de_snapshots(&tmp);
        // `pc-a.json` copiado para `pc-b.json`: o relatório diria "pc-b" sobre os dados de "pc-a".
        fs::write(snaps.join("pc-b.json"), to_json(&snap("pc-a", &["base"]))).unwrap();
        let erro = read(tmp.path(), "pc-b").unwrap_err().to_string();
        assert!(erro.contains("pc-a") && erro.contains("pc-b"), "{erro}");
    }

    #[test]
    fn versao_desconhecida_e_json_ruim_dao_erro_com_o_motivo() {
        let tmp = TmpDir::novo();
        let snaps = pasta_de_snapshots(&tmp);
        let v2 = to_json(&snap("pc-a", &["base"])).replacen("\"version\": 1", "\"version\": 2", 1);
        fs::write(snaps.join("pc-a.json"), v2).unwrap();
        let erro = format!("{:#}", read(tmp.path(), "pc-a").unwrap_err());
        assert!(erro.contains("versão 2"), "{erro}");
        fs::write(snaps.join("pc-b.json"), "não é json").unwrap();
        assert!(read(tmp.path(), "pc-b").is_err());
        fs::write(snaps.join("pc-c.json"), "").unwrap();
        assert!(read(tmp.path(), "pc-c").is_err());
    }

    #[test]
    fn ler_nao_altera_nada() {
        let tmp = TmpDir::novo();
        let caminho = write(tmp.path(), &snap("pc-a", &["base"])).unwrap();
        let antes = fs::read(&caminho).unwrap();
        read(tmp.path(), "pc-a").unwrap();
        assert_eq!(fs::read(&caminho).unwrap(), antes);
        assert_eq!(quantos(&tmp.path().join(".nodus/snapshots")), 1);
    }
}
