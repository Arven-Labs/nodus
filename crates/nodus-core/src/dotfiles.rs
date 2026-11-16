#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ---- validate_path: caminho relativo ao HOME, fora da denylist ----

    #[test]
    fn aceita_dotfiles_comuns() {
        for path in [
            ".bashrc",
            ".gitconfig",
            ".config/git/config",
            ".config/nvim/init.lua",
            ".sshrc",
            ".config/ghostty",
        ] {
            assert_eq!(validate_path(path), Ok(()), "{path}");
        }
    }

    #[test]
    fn recusa_caminho_vazio_absoluto_ou_com_componente_estranho() {
        assert_eq!(validate_path(""), Err(DotfileError::Empty));
        for path in ["/etc/passwd", "/home/x/.bashrc"] {
            assert_eq!(validate_path(path), Err(DotfileError::Absolute), "{path}");
        }
        for path in [
            "../.ssh/config",
            ".config/../.ssh/id_ed25519",
            "./.bashrc",
            ".config//git",
            ".bashrc/",
            "a\0b",
        ] {
            assert_eq!(
                validate_path(path),
                Err(DotfileError::BadComponent),
                "{path}"
            );
        }
    }

    #[test]
    fn recusa_a_denylist_de_segredos() {
        for path in [
            ".ssh",
            ".ssh/config",
            ".ssh/id_ed25519",
            ".gnupg/private-keys-v1.d",
            ".config/gh",
            ".config/gh/hosts.yml",
            ".aws/credentials",
            ".docker/config.json",
            ".pki/nssdb",
            ".mozilla/firefox",
            ".npmrc",
            ".claude.json",
            ".claude/settings.json",
            ".bash_history",
            ".config/fish/fish_history",
            ".irb_history",
        ] {
            assert_eq!(validate_path(path), Err(DotfileError::Denied), "{path}");
        }
    }

    #[test]
    fn denylist_e_por_componente_nao_por_prefixo_de_texto() {
        // `.sshrc` e `.config/ghostty` não são `.ssh` nem `.config/gh`.
        assert_eq!(validate_path(".sshrc"), Ok(()));
        assert_eq!(validate_path(".config/ghostty/config"), Ok(()));
        assert_eq!(validate_path(".history_notes"), Ok(()));
    }

    #[test]
    fn a_lista_padrao_passa_na_propria_validacao() {
        for path in DEFAULT_PATHS {
            assert_eq!(validate_path(path), Ok(()), "{path}");
        }
    }

    // ---- effective_list: padrão + extras - pulados ----

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|i| i.to_string()).collect()
    }

    #[test]
    fn sem_ajuste_a_lista_e_a_padrao() {
        let list = effective_list(&[], &[]);
        let default: Vec<String> = DEFAULT_PATHS.iter().map(|p| p.to_string()).collect();
        assert_eq!(list, default);
    }

    #[test]
    fn extra_entra_no_fim_sem_repetir() {
        let list = effective_list(&s(&[".config/starship.toml", ".bashrc"]), &[]);
        assert_eq!(
            list.last().map(String::as_str),
            Some(".config/starship.toml")
        );
        assert_eq!(list.iter().filter(|p| *p == ".bashrc").count(), 1);
    }

    #[test]
    fn skip_tira_da_lista_padrao_e_dos_extras() {
        let list = effective_list(
            &s(&[".config/starship.toml"]),
            &s(&[".zshrc", ".config/starship.toml"]),
        );
        assert!(!list.contains(&".zshrc".to_string()));
        assert!(!list.contains(&".config/starship.toml".to_string()));
        assert!(list.contains(&".bashrc".to_string()));
    }

    // ---- classify: o que foi observado vira uma entrada do relatório ----

    const HOME: &str = "/home/u";

    fn classify_at(path: &str, observed: Observed) -> Entry {
        classify(Path::new(HOME), path, observed)
    }

    #[test]
    fn ausente_arquivo_diretorio_e_outro() {
        assert_eq!(
            classify_at(".gitconfig", Observed::Missing).kind,
            Kind::Missing
        );
        assert_eq!(
            classify_at(".bashrc", Observed::File { size: 1670 }).kind,
            Kind::File { size: 1670 }
        );
        assert_eq!(classify_at(".config/git", Observed::Dir).kind, Kind::Dir);
        assert_eq!(classify_at(".profile", Observed::Other).kind, Kind::Other);
        assert_eq!(classify_at(".bashrc", Observed::Missing).path, ".bashrc");
    }

    #[test]
    fn symlink_guarda_o_alvo_e_marca_se_sai_do_home() {
        let link = |path: &str, target: &str| {
            classify_at(
                path,
                Observed::Symlink {
                    target: target.to_string(),
                },
            )
            .kind
        };
        let inside = |target: &str| Kind::Symlink {
            target: target.to_string(),
            outside_home: false,
        };
        let outside = |target: &str| Kind::Symlink {
            target: target.to_string(),
            outside_home: true,
        };

        assert_eq!(
            link(".profile", "/home/u/dotfiles/profile"),
            inside("/home/u/dotfiles/profile")
        );
        assert_eq!(
            link(".profile", "dotfiles/profile"),
            inside("dotfiles/profile")
        );
        assert_eq!(
            link(".config/git/config", "../../dotfiles/git"),
            inside("../../dotfiles/git")
        );
        assert_eq!(link(".profile", "/etc/profile"), outside("/etc/profile"));
        assert_eq!(
            link(".profile", "../other/profile"),
            outside("../other/profile")
        );
        assert_eq!(
            link(".config/git/config", "../../../x"),
            outside("../../../x")
        );
        // `/home/u2` começa com o texto `/home/u`, mas é outro diretório.
        assert_eq!(
            link(".profile", "/home/u2/profile"),
            outside("/home/u2/profile")
        );
        // Passa por dentro e volta para fora.
        assert_eq!(link(".profile", "a/../../x"), outside("a/../../x"));
    }
}
