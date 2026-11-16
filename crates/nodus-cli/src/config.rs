#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TmpDir;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};

    fn modo(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// Escreve um config à mão, com a permissão certa (0600), para testar a leitura.
    fn config_com(tmp: &TmpDir, conteudo: &str) -> PathBuf {
        let path = tmp.path().join("config.toml");
        fs::write(&path, conteudo).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        path
    }

    // --- caminho (função pura: as variáveis de ambiente entram como argumento) ---

    #[test]
    fn caminho_usa_xdg_config_home_quando_absoluto() {
        let p = config_path(Some("/x/cfg"), Some("/home/u")).unwrap();
        assert_eq!(p, PathBuf::from("/x/cfg/nodus/config.toml"));
    }

    #[test]
    fn caminho_ignora_xdg_vazio_ou_relativo() {
        for xdg in ["", "relativo/cfg"] {
            let p = config_path(Some(xdg), Some("/home/u")).unwrap();
            assert_eq!(
                p,
                PathBuf::from("/home/u/.config/nodus/config.toml"),
                "{xdg:?}"
            );
        }
    }

    #[test]
    fn caminho_cai_no_home_sem_xdg() {
        let p = config_path(None, Some("/home/u")).unwrap();
        assert_eq!(p, PathBuf::from("/home/u/.config/nodus/config.toml"));
    }

    #[test]
    fn caminho_sem_xdg_e_sem_home_e_erro_que_diz_as_variaveis() {
        for home in [None, Some(""), Some("relativo")] {
            let erro = config_path(None, home).unwrap_err().to_string();
            assert!(erro.contains("XDG_CONFIG_HOME"), "{home:?}: {erro}");
            assert!(erro.contains("HOME"), "{home:?}: {erro}");
        }
    }

    // --- gravar e ler ---

    #[test]
    fn write_cria_diretorio_0700_e_arquivo_0600_e_le_de_volta() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        write_workspace(&path, Path::new("/home/u/ws")).unwrap();
        assert_eq!(modo(&path), 0o600);
        assert_eq!(modo(path.parent().unwrap()), 0o700);
        assert_eq!(
            read_workspace(&path).unwrap(),
            Some(PathBuf::from("/home/u/ws"))
        );
    }

    #[test]
    fn write_substitui_o_valor_anterior() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        write_workspace(&path, Path::new("/a")).unwrap();
        write_workspace(&path, Path::new("/b")).unwrap();
        assert_eq!(read_workspace(&path).unwrap(), Some(PathBuf::from("/b")));
    }

    #[test]
    fn write_nao_deixa_temporario_sobrando() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        write_workspace(&path, Path::new("/a")).unwrap();
        write_workspace(&path, Path::new("/b")).unwrap();
        let nomes: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(nomes, ["config.toml"]);
    }

    #[test]
    fn caminho_com_aspas_e_barra_invertida_volta_igual() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        let estranho = Path::new("/tmp/a\"b\\c d");
        write_workspace(&path, estranho).unwrap();
        assert_eq!(read_workspace(&path).unwrap(), Some(estranho.to_path_buf()));
    }

    #[test]
    fn write_recusa_caminho_relativo_e_nao_cria_nada() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        let erro = format!(
            "{:#}",
            write_workspace(&path, Path::new("rel/ws")).unwrap_err()
        );
        assert!(erro.contains("absoluto"), "{erro}");
        assert!(!path.exists());
    }

    #[test]
    fn write_recusa_symlink_e_nao_escreve_no_alvo() {
        let tmp = TmpDir::novo();
        let alvo = tmp.path().join("alvo.toml");
        fs::write(&alvo, "").unwrap();
        fs::set_permissions(&alvo, fs::Permissions::from_mode(0o600)).unwrap();
        let path = tmp.path().join("config.toml");
        symlink(&alvo, &path).unwrap();
        let erro = format!("{:#}", write_workspace(&path, Path::new("/a")).unwrap_err());
        assert!(erro.contains("symlink"), "{erro}");
        assert_eq!(fs::read_to_string(&alvo).unwrap(), "");
    }

    #[test]
    fn write_e_read_recusam_permissao_mais_larga_que_0600() {
        for larga in [0o644, 0o640, 0o606, 0o660] {
            let tmp = TmpDir::novo();
            let path = tmp.path().join("config.toml");
            fs::write(&path, "version = 1\n").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(larga)).unwrap();
            let erro_write = format!("{:#}", write_workspace(&path, Path::new("/a")).unwrap_err());
            let erro_read = format!("{:#}", read_workspace(&path).unwrap_err());
            assert!(erro_write.contains("permissão"), "{larga:o}: {erro_write}");
            assert!(erro_read.contains("permissão"), "{larga:o}: {erro_read}");
        }
    }

    #[test]
    fn read_recusa_o_que_nao_e_arquivo_comum() {
        let tmp = TmpDir::novo();
        let erro = format!("{:#}", read_workspace(tmp.path()).unwrap_err());
        assert!(erro.contains("arquivo comum"), "{erro}");
    }

    // --- ler: formato estrito ---

    #[test]
    fn read_de_arquivo_inexistente_e_none() {
        let tmp = TmpDir::novo();
        assert_eq!(
            read_workspace(&tmp.path().join("nodus/config.toml")).unwrap(),
            None
        );
    }

    #[test]
    fn read_de_config_so_com_a_versao_e_none() {
        let tmp = TmpDir::novo();
        let path = config_com(&tmp, "version = 1\n");
        assert_eq!(read_workspace(&path).unwrap(), None);
    }

    #[test]
    fn read_recusa_conteudo_invalido_e_diz_o_arquivo() {
        let casos = [
            (
                "chave desconhecida",
                "version = 1\nworkspace = \"/a\"\nextra = 1\n",
            ),
            ("versão diferente", "version = 2\nworkspace = \"/a\"\n"),
            ("sem versão", "workspace = \"/a\"\n"),
            (
                "versão não numérica",
                "version = \"1\"\nworkspace = \"/a\"\n",
            ),
            ("caminho relativo", "version = 1\nworkspace = \"rel/ws\"\n"),
            ("caminho vazio", "version = 1\nworkspace = \"\"\n"),
            ("caminho não é texto", "version = 1\nworkspace = 3\n"),
            ("TOML inválido", "version = = 1\n"),
        ];
        for (nome, conteudo) in casos {
            let tmp = TmpDir::novo();
            let path = config_com(&tmp, conteudo);
            let erro = format!("{:#}", read_workspace(&path).unwrap_err());
            assert!(erro.contains("config.toml"), "{nome}: {erro}");
        }
    }

    #[test]
    fn read_recusa_arquivo_acima_do_limite() {
        let tmp = TmpDir::novo();
        let grande = format!("version = 1\n# {}\n", "x".repeat(65 * 1024));
        let path = config_com(&tmp, &grande);
        let erro = format!("{:#}", read_workspace(&path).unwrap_err());
        assert!(erro.contains("64 KiB"), "{erro}");
    }

    #[test]
    fn read_recusa_arquivo_que_nao_e_utf8() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("config.toml");
        fs::write(&path, [0x76, 0xff, 0xfe]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let erro = format!("{:#}", read_workspace(&path).unwrap_err());
        assert!(erro.contains("UTF-8"), "{erro}");
    }

    // --- apagar ---

    #[test]
    fn clear_remove_o_arquivo() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        write_workspace(&path, Path::new("/a")).unwrap();
        assert!(clear_workspace(&path).unwrap());
        assert!(!path.exists());
        assert_eq!(read_workspace(&path).unwrap(), None);
    }

    #[test]
    fn clear_sem_arquivo_diz_que_nao_havia() {
        let tmp = TmpDir::novo();
        assert!(!clear_workspace(&tmp.path().join("nodus/config.toml")).unwrap());
    }

    #[test]
    fn clear_recusa_symlink_e_nao_apaga_o_alvo() {
        let tmp = TmpDir::novo();
        let alvo = tmp.path().join("alvo.toml");
        fs::write(&alvo, "version = 1\n").unwrap();
        fs::set_permissions(&alvo, fs::Permissions::from_mode(0o600)).unwrap();
        let path = tmp.path().join("config.toml");
        symlink(&alvo, &path).unwrap();
        let erro = format!("{:#}", clear_workspace(&path).unwrap_err());
        assert!(erro.contains("symlink"), "{erro}");
        assert!(alvo.exists());
    }
}
