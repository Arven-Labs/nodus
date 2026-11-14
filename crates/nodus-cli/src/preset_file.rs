//! Leitura do arquivo de preset, com limite de tamanho (o preset pode vir de terceiro).

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
}
