#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::dotfiles::{Kind, Observed};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn home_precisa_ser_absoluto_e_o_erro_diz_a_variavel() {
        assert_eq!(
            home_dir(Some("/home/u")).unwrap(),
            std::path::PathBuf::from("/home/u")
        );
        for home in [None, Some(""), Some("relativo")] {
            let erro = home_dir(home).unwrap_err().to_string();
            assert!(erro.contains("HOME"), "{home:?}: {erro}");
        }
    }

    #[test]
    fn observa_ausente_arquivo_e_pasta() {
        let tmp = TmpDir::novo();
        fs::write(tmp.path().join(".bashrc"), "alias x=y\n").unwrap();
        fs::create_dir_all(tmp.path().join(".config/git")).unwrap();

        assert_eq!(
            observe(tmp.path(), ".gitconfig").unwrap(),
            Observed::Missing
        );
        assert_eq!(
            observe(tmp.path(), ".bashrc").unwrap(),
            Observed::File { size: 10 }
        );
        assert_eq!(observe(tmp.path(), ".config/git").unwrap(), Observed::Dir);
        // O pai nem existe: continua "ausente", não é erro.
        assert_eq!(
            observe(tmp.path(), ".config/nvim/init.lua").unwrap(),
            Observed::Missing
        );
    }

    #[test]
    fn symlink_mostra_o_alvo_sem_seguir_e_vale_mesmo_quebrado() {
        let tmp = TmpDir::novo();
        symlink("dotfiles/profile", tmp.path().join(".profile")).unwrap();
        // Alvo que não existe: o link continua sendo um symlink.
        assert_eq!(
            observe(tmp.path(), ".profile").unwrap(),
            Observed::Symlink {
                target: "dotfiles/profile".into()
            }
        );
        // Link para uma pasta: não entra nela, só mostra o alvo.
        fs::create_dir_all(tmp.path().join("segredos")).unwrap();
        symlink("segredos", tmp.path().join(".vimrc")).unwrap();
        assert_eq!(
            observe(tmp.path(), ".vimrc").unwrap(),
            Observed::Symlink {
                target: "segredos".into()
            }
        );
    }

    #[test]
    fn nao_abre_o_arquivo_so_olha_os_metadados() {
        // Sem permissão de leitura, o tamanho ainda sai: prova que o conteúdo não é aberto.
        let tmp = TmpDir::novo();
        let path = tmp.path().join(".bashrc");
        fs::write(&path, "segredo").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(
            observe(tmp.path(), ".bashrc").unwrap(),
            Observed::File { size: 7 }
        );
    }

    #[test]
    fn report_mantem_a_ordem_da_lista_e_classifica() {
        let tmp = TmpDir::novo();
        fs::write(tmp.path().join(".bashrc"), "x").unwrap();
        symlink("/etc/profile", tmp.path().join(".profile")).unwrap();
        let lista: Vec<String> = [".profile", ".gitconfig", ".bashrc"]
            .iter()
            .map(|p| p.to_string())
            .collect();

        let entradas = report(tmp.path(), &lista).unwrap();

        let caminhos: Vec<&str> = entradas.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(caminhos, [".profile", ".gitconfig", ".bashrc"]);
        assert_eq!(
            entradas[0].kind,
            Kind::Symlink {
                target: "/etc/profile".into(),
                outside_home: true
            }
        );
        assert_eq!(entradas[1].kind, Kind::Missing);
        assert_eq!(entradas[2].kind, Kind::File { size: 1 });
    }

    #[test]
    fn report_recusa_caminho_da_denylist_mesmo_vindo_de_fora_do_config() {
        // Defesa em profundidade: o config já valida, mas o relatório não confia nisso.
        let tmp = TmpDir::novo();
        fs::create_dir_all(tmp.path().join(".ssh")).unwrap();
        let erro = report(tmp.path(), &[".ssh/id_ed25519".to_string()]).unwrap_err();
        assert!(format!("{erro:#}").contains(".ssh/id_ed25519"), "{erro:#}");
    }
}
