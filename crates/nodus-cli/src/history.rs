#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::history::{ExecResult, HistoryEntry};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn entrada(pacote: &str, at: u64) -> HistoryEntry {
        HistoryEntry {
            version: 1,
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
