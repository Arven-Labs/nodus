//! Gravação do snapshot na pasta-fonte dos dotfiles (ADR 0031, fatia 1). Escrita atômica
//! (temporário `create_new` 0600 e `rename`), pastas 0700, e nada de seguir symlink: `.nodus`,
//! `snapshots` e o arquivo de destino precisam ser reais, senão o snapshot (que é do dono) pararia
//! fora do repositório.

use anyhow::{Context, Result, bail};
use nodus_core::snapshot::{Snapshot, parse, to_json, validate_name};
use std::collections::BTreeSet;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{ErrorKind, Read, Write};
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

/// Teto do arquivo lido: o snapshot veio de outra máquina por git (entrada externa) e um arquivo
/// enorme não pode ser lido inteiro. Um snapshot real tem alguns KB.
pub const MAX_SNAPSHOT_BYTES: u64 = 1 << 20;

/// Lê `<dir>/.nodus/snapshots/<rótulo>.json` (só leitura: não cria nada). O rótulo é validado antes
/// de montar o caminho; `.nodus`, `snapshots` e o arquivo precisam ser reais (sem symlink, que
/// poderia apontar para fora do repositório); a leitura é a estrita do core; e o `name` de dentro
/// do arquivo tem de ser o rótulo pedido (um arquivo copiado com outro nome enganaria o relatório).
pub fn read(dir: &Path, label: &str) -> Result<Snapshot> {
    validate_name(label)?;
    let meta = fs::metadata(dir).with_context(|| format!("lendo {}", dir.display()))?;
    if !meta.is_dir() {
        bail!("{} não é uma pasta", dir.display());
    }
    let nodus = require_real_dir(&dir.join(".nodus"))?;
    let snapshots = require_real_dir(&nodus.join("snapshots"))?;
    let path = snapshots.join(format!("{label}.json"));
    let meta = fs::symlink_metadata(&path)
        .with_context(|| format!("não achei o snapshot {label:?} em {}", path.display()))?;
    if !meta.is_file() {
        bail!(
            "{} não é um arquivo comum (symlink ou pasta); não vou ler",
            path.display()
        );
    }
    if meta.len() > MAX_SNAPSHOT_BYTES {
        bail!(
            "{} é grande demais para um snapshot (limite {MAX_SNAPSHOT_BYTES} bytes)",
            path.display()
        );
    }
    // `take` limita a leitura mesmo que o arquivo cresça depois da conferência acima.
    let mut text = String::new();
    fs::File::open(&path)
        .with_context(|| format!("abrindo {}", path.display()))?
        .take(MAX_SNAPSHOT_BYTES + 1)
        .read_to_string(&mut text)
        .with_context(|| format!("lendo {}", path.display()))?;
    if text.len() as u64 > MAX_SNAPSHOT_BYTES {
        bail!("{} é grande demais para um snapshot", path.display());
    }
    let snapshot = parse(&text).with_context(|| format!("lendo {}", path.display()))?;
    if snapshot.name != label {
        bail!(
            "{} diz ser o snapshot {:?}, não {label:?}; renomeie o arquivo ou regrave",
            path.display(),
            snapshot.name
        );
    }
    Ok(snapshot)
}

/// Um snapshot que existe na pasta mas não pôde ser lido (versão nova, symlink, nome divergente,
/// grande demais...). Mostrado como aviso: não derruba a leitura dos outros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    pub label: String,
    pub reason: String,
}

/// Lê todos os `*.json` de `<dir>/.nodus/snapshots/`, ordenados por rótulo. Só leitura. Entradas
/// que não são snapshot (sem `.json`, ou começadas por `.`, como o temporário de uma gravação) são
/// ignoradas; um snapshot ruim vira `Unreadable` e os outros seguem. Sem `.nodus` ou sem
/// `snapshots` não é erro (ainda não há snapshots); symlink nessas pastas é.
pub fn read_all(dir: &Path) -> Result<(Vec<Snapshot>, Vec<Unreadable>)> {
    let meta = fs::metadata(dir).with_context(|| format!("lendo {}", dir.display()))?;
    if !meta.is_dir() {
        bail!("{} não é uma pasta", dir.display());
    }
    let nodus = dir.join(".nodus");
    if !exists(&nodus)? {
        return Ok((Vec::new(), Vec::new()));
    }
    let nodus = require_real_dir(&nodus)?;
    let snapshots = nodus.join("snapshots");
    if !exists(&snapshots)? {
        return Ok((Vec::new(), Vec::new()));
    }
    let snapshots = require_real_dir(&snapshots)?;

    let mut found = Vec::new();
    let mut unreadable = Vec::new();
    for entry in
        fs::read_dir(&snapshots).with_context(|| format!("lendo {}", snapshots.display()))?
    {
        let entry = entry.with_context(|| format!("lendo {}", snapshots.display()))?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let Some(label) = name.strip_suffix(".json") else {
            continue;
        };
        match read(dir, label) {
            Ok(snapshot) => found.push(snapshot),
            Err(erro) => unreadable.push(Unreadable {
                label: label.to_string(),
                reason: format!("{erro:#}"),
            }),
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    unreadable.sort_by(|a, b| a.label.cmp(&b.label));
    Ok((found, unreadable))
}

/// Lê só os rótulos pedidos, ordenados e sem repetição. Pedido explícito erra alto: um rótulo
/// ausente ou ruim é erro, não aviso.
pub fn read_labels(dir: &Path, labels: &[String]) -> Result<Vec<Snapshot>> {
    let unique: BTreeSet<&String> = labels.iter().collect();
    unique.into_iter().map(|label| read(dir, label)).collect()
}

/// `true` se o caminho existe (inclusive como symlink, sem segui-lo).
fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(erro) if erro.kind() == ErrorKind::NotFound => Ok(false),
        Err(erro) => Err(erro).with_context(|| format!("lendo {}", path.display())),
    }
}

/// A pasta existe e é uma pasta de verdade (não symlink). Não cria.
fn require_real_dir(path: &Path) -> Result<PathBuf> {
    let meta =
        fs::symlink_metadata(path).with_context(|| format!("não achei {}", path.display()))?;
    if !meta.is_dir() {
        bail!(
            "{} não é uma pasta de verdade (symlink ou arquivo); não vou ler por ela",
            path.display()
        );
    }
    Ok(path.to_path_buf())
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

    // --- read_all e read_labels: o status do preset olha várias máquinas ---

    fn rotulos(snaps: &[Snapshot]) -> Vec<&str> {
        snaps.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn read_all_le_todos_ordenados_por_rotulo() {
        let tmp = TmpDir::novo();
        for nome in ["pc-c", "pc-a", "pc-b"] {
            write(tmp.path(), &snap(nome, &["base"])).unwrap();
        }
        let (lidos, ilegiveis) = read_all(tmp.path()).unwrap();
        assert_eq!(rotulos(&lidos), ["pc-a", "pc-b", "pc-c"]);
        assert!(ilegiveis.is_empty());
    }

    #[test]
    fn read_all_ignora_o_que_nao_e_snapshot() {
        let tmp = TmpDir::novo();
        write(tmp.path(), &snap("pc-a", &["base"])).unwrap();
        let snaps = tmp.path().join(".nodus/snapshots");
        fs::write(snaps.join("notas.txt"), "x").unwrap();
        fs::write(snaps.join(".pc-a.json.nodus-123"), "meio gravado").unwrap();
        fs::write(snaps.join(".oculto.json"), "x").unwrap();
        let (lidos, ilegiveis) = read_all(tmp.path()).unwrap();
        assert_eq!(rotulos(&lidos), ["pc-a"]);
        assert!(ilegiveis.is_empty(), "{ilegiveis:?}");
    }

    #[test]
    fn um_snapshot_ruim_nao_derruba_os_outros_e_vira_aviso_com_o_motivo() {
        let tmp = TmpDir::novo();
        write(tmp.path(), &snap("pc-a", &["base"])).unwrap();
        let snaps = tmp.path().join(".nodus/snapshots");
        let v2 = to_json(&snap("pc-b", &["base"])).replacen("\"version\": 1", "\"version\": 2", 1);
        fs::write(snaps.join("pc-b.json"), v2).unwrap();
        let (lidos, ilegiveis) = read_all(tmp.path()).unwrap();
        assert_eq!(rotulos(&lidos), ["pc-a"]);
        assert_eq!(ilegiveis.len(), 1);
        assert_eq!(ilegiveis[0].label, "pc-b");
        assert!(
            ilegiveis[0].reason.contains("versão 2"),
            "{:?}",
            ilegiveis[0]
        );
    }

    #[test]
    fn symlink_nome_invalido_nome_divergente_e_arquivo_grande_viram_aviso() {
        let tmp = TmpDir::novo();
        let fora = TmpDir::novo();
        write(tmp.path(), &snap("pc-a", &["base"])).unwrap();
        let snaps = tmp.path().join(".nodus/snapshots");
        fs::write(
            fora.path().join("x.json"),
            to_json(&snap("pc-x", &["base"])),
        )
        .unwrap();
        symlink(fora.path().join("x.json"), snaps.join("pc-x.json")).unwrap();
        fs::write(snaps.join("Foo.json"), to_json(&snap("pc-f", &["base"]))).unwrap();
        fs::write(snaps.join("pc-d.json"), to_json(&snap("pc-a", &["base"]))).unwrap();
        let grande = "x".repeat(MAX_SNAPSHOT_BYTES as usize + 1);
        fs::write(snaps.join("pc-g.json"), grande).unwrap();
        let (lidos, ilegiveis) = read_all(tmp.path()).unwrap();
        assert_eq!(rotulos(&lidos), ["pc-a"]);
        let mut avisos: Vec<&str> = ilegiveis.iter().map(|u| u.label.as_str()).collect();
        avisos.sort();
        assert_eq!(avisos, ["Foo", "pc-d", "pc-g", "pc-x"]);
    }

    #[test]
    fn sem_pasta_de_snapshots_ou_vazia_nao_e_erro_so_nao_ha_nada() {
        let tmp = TmpDir::novo();
        let (lidos, ilegiveis) = read_all(tmp.path()).unwrap();
        assert!(lidos.is_empty() && ilegiveis.is_empty());
        assert!(!tmp.path().join(".nodus").exists(), "ler não cria pasta");
        pasta_de_snapshots(&tmp);
        let (lidos, ilegiveis) = read_all(tmp.path()).unwrap();
        assert!(lidos.is_empty() && ilegiveis.is_empty());
    }

    #[test]
    fn pasta_fonte_ausente_e_pastas_symlink_sao_erro() {
        let tmp = TmpDir::novo();
        assert!(read_all(&tmp.path().join("nao-existe")).is_err());
        let fora = TmpDir::novo();
        write(fora.path(), &snap("pc-a", &["base"])).unwrap();
        // `.nodus` é um symlink para uma pasta de fora com snapshots válidos: não se lê por ele.
        symlink(fora.path().join(".nodus"), tmp.path().join(".nodus")).unwrap();
        assert!(read_all(tmp.path()).is_err());
        // `snapshots` é um symlink.
        let tmp2 = TmpDir::novo();
        fs::create_dir(tmp2.path().join(".nodus")).unwrap();
        symlink(
            fora.path().join(".nodus/snapshots"),
            tmp2.path().join(".nodus/snapshots"),
        )
        .unwrap();
        assert!(read_all(tmp2.path()).is_err());
    }

    #[test]
    fn read_all_nao_altera_nada() {
        let tmp = TmpDir::novo();
        let caminho = write(tmp.path(), &snap("pc-a", &["base"])).unwrap();
        let antes = fs::read(&caminho).unwrap();
        read_all(tmp.path()).unwrap();
        assert_eq!(fs::read(&caminho).unwrap(), antes);
        assert_eq!(quantos(&tmp.path().join(".nodus/snapshots")), 1);
    }

    #[test]
    fn read_labels_le_so_os_pedidos_ordenados_e_sem_repeticao() {
        let tmp = TmpDir::novo();
        for nome in ["pc-a", "pc-b", "pc-c"] {
            write(tmp.path(), &snap(nome, &["base"])).unwrap();
        }
        let pedidos: Vec<String> = ["pc-c", "pc-a", "pc-c"].map(String::from).to_vec();
        let lidos = read_labels(tmp.path(), &pedidos).unwrap();
        assert_eq!(rotulos(&lidos), ["pc-a", "pc-c"]);
    }

    #[test]
    fn read_labels_erra_alto_se_um_pedido_nao_existe_ou_esta_ruim() {
        let tmp = TmpDir::novo();
        write(tmp.path(), &snap("pc-a", &["base"])).unwrap();
        let snaps = tmp.path().join(".nodus/snapshots");
        fs::write(snaps.join("pc-b.json"), "não é json").unwrap();
        for pedidos in [["pc-a", "pc-z"], ["pc-a", "pc-b"], ["pc-a", "../x"]] {
            let pedidos: Vec<String> = pedidos.map(String::from).to_vec();
            assert!(read_labels(tmp.path(), &pedidos).is_err(), "{pedidos:?}");
        }
    }
}
