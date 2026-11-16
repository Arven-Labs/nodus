//! Leitura do arquivo de preset, com limite de tamanho (o preset pode vir de terceiro).

use anyhow::{Context, Result, bail};
use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

/// Um preset de pacotes tem poucos KiB; 1 MiB barra arquivo gigante (ou `/dev/zero`) antes do parse.
pub const MAX_PRESET_BYTES: u64 = 1024 * 1024;

pub fn read_preset(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path).with_context(|| format!("abrindo {}", path.display()))?;
    // Lê no máximo um byte além do limite: não confia no tamanho informado pelo sistema de arquivos.
    let mut bytes = Vec::new();
    file.take(MAX_PRESET_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("lendo {}", path.display()))?;
    if bytes.len() as u64 > MAX_PRESET_BYTES {
        bail!("{} é maior que 1 MiB", path.display());
    }
    String::from_utf8(bytes).with_context(|| format!("{} não é UTF-8", path.display()))
}

/// Troca o conteúdo do preset por `new_text`, de forma atômica. Se `path` é um symlink (um preset
/// que vive no seu repositório de dotfiles), grava no alvo real: um `rename` por cima do link o
/// trocaria por um arquivo comum. Preserva as permissões do original e só escreve se o arquivo
/// ainda tem exatamente `old_text` (o que foi lido e editado): se mudou no meio, é erro e nada é
/// escrito.
pub fn write_preset(path: &Path, old_text: &str, new_text: &str) -> Result<()> {
    let real =
        std::fs::canonicalize(path).with_context(|| format!("abrindo {}", path.display()))?;
    let meta =
        std::fs::metadata(&real).with_context(|| format!("examinando {}", real.display()))?;
    if !meta.is_file() {
        bail!("{} não é um arquivo comum", real.display());
    }
    if read_preset(&real)?.len() > old_text.len() + 10_000_000 {
        bail!(
            "{} mudou desde a leitura; nada foi escrito: rode o comando de novo",
            real.display()
        );
    }
    let dir = real.parent().context("caminho do preset sem pasta")?;
    let name = real
        .file_name()
        .context("caminho do preset sem nome de arquivo")?
        .to_string_lossy();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&tmp)
        .with_context(|| format!("criando {}", tmp.display()))?;
    let written = file
        .set_permissions(meta.permissions())
        .with_context(|| format!("ajustando as permissões de {}", tmp.display()))
        .and_then(|()| {
            crate::config::commit(file, new_text, &tmp, &real)
                .with_context(|| format!("gravando {}", real.display()))
        });
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONTADOR: AtomicUsize = AtomicUsize::new(0);

    /// Arquivo temporário único, apagado no fim. Nunca toca no `$HOME` real.
    struct Tmp(PathBuf);

    impl Tmp {
        fn com(conteudo: &[u8]) -> Self {
            let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("nodus-preset-{}-{n}.toml", std::process::id()));
            fs::write(&path, conteudo).unwrap();
            Tmp(path)
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn le_um_arquivo_pequeno() {
        let f = Tmp::com(b"version = 1\n");
        assert_eq!(read_preset(&f.0).unwrap(), "version = 1\n");
    }

    #[test]
    fn aceita_exatamente_o_limite() {
        let f = Tmp::com(&vec![b'#'; MAX_PRESET_BYTES as usize]);
        assert_eq!(read_preset(&f.0).unwrap().len(), MAX_PRESET_BYTES as usize);
    }

    #[test]
    fn recusa_arquivo_acima_de_1_mib() {
        assert_eq!(MAX_PRESET_BYTES, 1024 * 1024);
        let f = Tmp::com(&vec![b'#'; MAX_PRESET_BYTES as usize + 1]);
        let erro = read_preset(&f.0).unwrap_err().to_string();
        assert!(erro.contains("maior que 1 MiB"), "{erro}");
    }

    #[test]
    fn recusa_arquivo_que_nao_e_utf8() {
        let f = Tmp::com(&[0xff, 0xfe, b'\n']);
        assert!(read_preset(&f.0).is_err());
    }

    #[test]
    fn arquivo_inexistente_e_erro_com_o_caminho() {
        let path = std::env::temp_dir().join("nodus-preset-nao-existe.toml");
        let erro = format!("{:#}", read_preset(&path).unwrap_err());
        assert!(erro.contains("nodus-preset-nao-existe.toml"), "{erro}");
    }

    // --- write_preset: troca o conteúdo do preset de forma atômica ---

    use crate::testutil::TmpDir;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn preset_em(tmp: &TmpDir, conteudo: &str, modo: u32) -> PathBuf {
        let path = tmp.path().join("preset.toml");
        fs::write(&path, conteudo).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(modo)).unwrap();
        path
    }

    fn modo(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn grava_o_texto_novo_e_preserva_as_permissoes() {
        for original in [0o600, 0o644, 0o640] {
            let tmp = TmpDir::novo();
            let path = preset_em(&tmp, "version = 1\n", original);
            write_preset(&path, "version = 1\n", "version = 1\npacman = []\n").unwrap();
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                "version = 1\npacman = []\n"
            );
            assert_eq!(modo(&path), original);
        }
    }

    #[test]
    fn segue_o_symlink_e_grava_no_alvo_sem_trocar_o_link() {
        let tmp = TmpDir::novo();
        let real = preset_em(&tmp, "version = 1\n", 0o644);
        let link = tmp.path().join("link.toml");
        symlink(&real, &link).unwrap();
        write_preset(&link, "version = 1\n", "version = 1\npacman = []\n").unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(&real).unwrap(),
            "version = 1\npacman = []\n"
        );
    }

    #[test]
    fn arquivo_que_mudou_desde_a_leitura_e_erro_e_nao_e_sobrescrito() {
        let tmp = TmpDir::novo();
        let path = preset_em(&tmp, "version = 1\n# mudei na mão\n", 0o644);
        let erro = write_preset(&path, "version = 1\n", "version = 1\npacman = []\n")
            .unwrap_err()
            .to_string();
        assert!(erro.contains("mudou"), "{erro}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "version = 1\n# mudei na mão\n"
        );
    }

    #[test]
    fn nao_deixa_arquivo_temporario_sobrando() {
        let tmp = TmpDir::novo();
        let path = preset_em(&tmp, "version = 1\n", 0o644);
        write_preset(&path, "version = 1\n", "version = 1\npacman = []\n").unwrap();
        let _ = write_preset(&path, "outro\n", "x\n");
        let nomes: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(nomes, ["preset.toml"]);
    }

    #[test]
    fn recusa_o_que_nao_e_arquivo_comum_e_o_que_nao_existe() {
        let tmp = TmpDir::novo();
        let erro = format!("{:#}", write_preset(tmp.path(), "", "x").unwrap_err());
        assert!(erro.contains("arquivo comum"), "{erro}");
        let inexistente = tmp.path().join("nao-existe.toml");
        let erro = format!("{:#}", write_preset(&inexistente, "", "x").unwrap_err());
        assert!(erro.contains("nao-existe.toml"), "{erro}");
        assert!(!inexistente.exists());
    }
}
