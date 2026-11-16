//! Arquivo do histórico: `$XDG_STATE_HOME/nodus/history.jsonl` (ou `~/.local/state/...`), uma
//! entrada por linha, só acrescenta. É estado do usuário e entrada externa na leitura
//! (SECURITY.md T5): diretório 0700, arquivo 0600, sem symlink, leitura estrita.

use anyhow::{Context, Result, bail};
use nodus_core::history::{HistoryEntry, parse_line, to_line};
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// Cada linha tem ~200 bytes: 8 MiB cabem dezenas de milhares de execuções e barram arquivo gigante.
pub const MAX_HISTORY_BYTES: u64 = 8 * 1024 * 1024;

/// Onde fica o histórico. Função pura: as variáveis de ambiente entram como argumento. Só valem
/// caminhos absolutos (o XDG manda ignorar os relativos).
pub fn history_path(xdg_state_home: Option<&str>, home: Option<&str>) -> Result<PathBuf> {
    let absolute = |s: &&str| !s.is_empty() && Path::new(s).is_absolute();
    if let Some(xdg) = xdg_state_home.filter(absolute) {
        return Ok(Path::new(xdg).join("nodus/history.jsonl"));
    }
    if let Some(home) = home.filter(absolute) {
        return Ok(Path::new(home).join(".local/state/nodus/history.jsonl"));
    }
    bail!(
        "não achei onde guardar o histórico: defina XDG_STATE_HOME ou HOME com um caminho absoluto"
    )
}

/// `false` se o arquivo não existe. Recusa symlink, o que não é arquivo comum e permissão mais larga
/// que 0600 (outro usuário não pode ler nem alterar o histórico).
pub(crate) fn check_file(path: &Path) -> Result<bool> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e).with_context(|| format!("examinando {}", path.display())),
    };
    if meta.file_type().is_symlink() {
        bail!("{} é um symlink; recuso seguir", path.display());
    }
    if !meta.file_type().is_file() {
        bail!("{} não é um arquivo comum", path.display());
    }
    let mode = meta.mode() & 0o777;
    if mode & 0o077 != 0 {
        bail!(
            "{} tem permissão {mode:o}, mais larga que 600; corrija com `chmod 600`",
            path.display()
        );
    }
    Ok(true)
}

/// Acrescenta uma entrada (uma única escrita de linha inteira).
pub fn append(path: &Path, entry: &HistoryEntry) -> Result<()> {
    let dir = path.parent().context("caminho do histórico sem pasta")?;
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("criando {}", dir.display()))?;
    check_file(path)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("abrindo {}", path.display()))?;
    let mut line = to_line(entry);
    line.push('\n');
    file.write_all(line.as_bytes())
        .with_context(|| format!("gravando em {}", path.display()))
}

/// Lê todo o histórico. Arquivo inexistente é histórico vazio; linha inválida é erro (não adivinha).
pub fn read(path: &Path) -> Result<Vec<HistoryEntry>> {
    if !check_file(path)? {
        return Ok(vec![]);
    }
    let file = File::open(path).with_context(|| format!("abrindo {}", path.display()))?;
    // Lê no máximo um byte além do limite: não confia no tamanho informado pelo sistema de arquivos.
    let mut bytes = Vec::new();
    file.take(MAX_HISTORY_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("lendo {}", path.display()))?;
    if bytes.len() as u64 > MAX_HISTORY_BYTES {
        bail!("{} é maior que 8 MiB", path.display());
    }
    let text =
        String::from_utf8(bytes).with_context(|| format!("{} não é UTF-8", path.display()))?;
    text.lines()
        .enumerate()
        .map(|(i, line)| {
            parse_line(line).with_context(|| format!("linha {} de {}", i + 1, path.display()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::history::{Action, ExecResult, HistoryEntry, Manager};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn entrada(pacote: &str, at: u64) -> HistoryEntry {
        HistoryEntry {
            version: 3,
            action: Action::Install,
            undoes: None,
            manager: Manager::Pacman,
            dir: None,
            at_unix: at,
            command: ["sudo", "pacman", "-S", "--needed", "--", pacote]
                .map(String::from)
                .to_vec(),
            requested: vec![pacote.into()],
            installed: vec![pacote.into()],
            still_missing: vec![],
            result: ExecResult::Ok,
        }
    }

    fn modo(path: &std::path::Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    // --- caminho (função pura: as variáveis de ambiente entram como argumento) ---

    #[test]
    fn caminho_usa_xdg_state_home_quando_absoluto() {
        let p = history_path(Some("/x/state"), Some("/home/u")).unwrap();
        assert_eq!(p, std::path::PathBuf::from("/x/state/nodus/history.jsonl"));
    }

    #[test]
    fn caminho_ignora_xdg_vazio_ou_relativo() {
        for xdg in ["", "relativo/state", "."] {
            let p = history_path(Some(xdg), Some("/home/u")).unwrap();
            assert_eq!(
                p,
                std::path::PathBuf::from("/home/u/.local/state/nodus/history.jsonl"),
                "xdg = {xdg:?}"
            );
        }
    }

    #[test]
    fn caminho_cai_no_home_sem_xdg() {
        let p = history_path(None, Some("/home/u")).unwrap();
        assert_eq!(
            p,
            std::path::PathBuf::from("/home/u/.local/state/nodus/history.jsonl")
        );
    }

    #[test]
    fn caminho_sem_xdg_e_sem_home_e_erro() {
        assert!(history_path(None, None).is_err());
        assert!(history_path(Some(""), Some("")).is_err());
        assert!(history_path(None, Some("relativo")).is_err());
    }

    // --- gravação ---

    #[test]
    fn append_cria_diretorio_0700_e_arquivo_0600() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("state/nodus/history.jsonl");
        append(&path, &entrada("sl", 1)).unwrap();
        assert_eq!(modo(&path), 0o600);
        assert_eq!(modo(path.parent().unwrap()), 0o700);
    }

    #[test]
    fn append_acrescenta_sem_sobrescrever() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/history.jsonl");
        append(&path, &entrada("sl", 1)).unwrap();
        append(&path, &entrada("zsh", 2)).unwrap();
        let texto = fs::read_to_string(&path).unwrap();
        assert_eq!(texto.lines().count(), 2);
        assert!(texto.ends_with('\n'));
        let lidas = read(&path).unwrap();
        assert_eq!(lidas, vec![entrada("sl", 1), entrada("zsh", 2)]);
    }

    #[test]
    fn append_recusa_symlink() {
        let tmp = TmpDir::novo();
        let alvo = tmp.path().join("alvo.jsonl");
        fs::write(&alvo, "").unwrap();
        fs::set_permissions(&alvo, fs::Permissions::from_mode(0o600)).unwrap();
        let path = tmp.path().join("history.jsonl");
        symlink(&alvo, &path).unwrap();
        let erro = format!("{:#}", append(&path, &entrada("sl", 1)).unwrap_err());
        assert!(erro.contains("symlink"), "{erro}");
        assert_eq!(fs::read_to_string(&alvo).unwrap(), "");
    }

    #[test]
    fn append_e_read_recusam_permissao_mais_larga_que_0600() {
        for larga in [0o644, 0o640, 0o606, 0o660] {
            let tmp = TmpDir::novo();
            let path = tmp.path().join("history.jsonl");
            fs::write(&path, "").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(larga)).unwrap();
            let erro_append = format!("{:#}", append(&path, &entrada("sl", 1)).unwrap_err());
            let erro_read = format!("{:#}", read(&path).unwrap_err());
            assert!(
                erro_append.contains("permissão"),
                "{larga:o}: {erro_append}"
            );
            assert!(erro_read.contains("permissão"), "{larga:o}: {erro_read}");
        }
    }

    #[test]
    fn read_aceita_arquivo_mais_restrito_que_0600() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("history.jsonl");
        fs::write(&path, "").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(read(&path).unwrap().is_empty());
    }

    // --- leitura ---

    #[test]
    fn arquivo_inexistente_e_historico_vazio() {
        let tmp = TmpDir::novo();
        assert!(
            read(&tmp.path().join("nao-existe.jsonl"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn linha_invalida_diz_o_numero_da_linha() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("history.jsonl");
        append(&path, &entrada("sl", 1)).unwrap();
        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        std::io::Write::write_all(&mut f, b"isto nao e json\n").unwrap();
        let erro = format!("{:#}", read(&path).unwrap_err());
        assert!(erro.contains("linha 2"), "{erro}");
    }

    #[test]
    fn recusa_arquivo_acima_do_limite() {
        assert_eq!(MAX_HISTORY_BYTES, 8 * 1024 * 1024);
        let tmp = TmpDir::novo();
        let path = tmp.path().join("history.jsonl");
        fs::write(&path, vec![b'#'; MAX_HISTORY_BYTES as usize + 1]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let erro = format!("{:#}", read(&path).unwrap_err());
        assert!(erro.contains("maior que"), "{erro}");
    }
}
