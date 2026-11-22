//! `nodusctl dotfiles`: o I/O fino do relatório (ADR 0021). Só `symlink_metadata` e `read_link`:
//! nunca abre o arquivo nem segue symlink. A lógica (validação, denylist, classificação) é do core.

use anyhow::{Context, Result, bail};
use nodus_core::dotfiles::{Entry, Observed, classify, validate_path};
use nodus_core::snapshot::{DotfileEntry, dotfile_state};
use std::path::{Path, PathBuf};

/// O `$HOME`, que precisa ser absoluto: é a raiz de todos os caminhos da lista.
pub fn home_dir(home: Option<&str>) -> Result<PathBuf> {
    match home {
        Some(home) if !home.is_empty() && Path::new(home).is_absolute() => Ok(PathBuf::from(home)),
        _ => bail!("não achei o HOME: defina a variável HOME com um caminho absoluto"),
    }
}

/// O que há em `home/path`, sem abrir o arquivo e sem seguir o symlink. Ausente (inclusive pai
/// ausente) não é erro; qualquer outra falha (permissão no pai, por exemplo) é.
pub fn observe(home: &Path, path: &str) -> Result<Observed> {
    let full = home.join(path);
    let meta = match std::fs::symlink_metadata(&full) {
        Ok(meta) => meta,
        Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => return Ok(Observed::Missing),
        Err(erro) => return Err(erro).with_context(|| format!("lendo {}", full.display())),
    };
    let kind = meta.file_type();
    if kind.is_symlink() {
        let target = std::fs::read_link(&full)
            .with_context(|| format!("lendo o link {}", full.display()))?;
        Ok(Observed::Symlink {
            target: target.to_string_lossy().into_owned(),
        })
    } else if kind.is_file() {
        Ok(Observed::File { size: meta.len() })
    } else if kind.is_dir() {
        Ok(Observed::Dir)
    } else {
        Ok(Observed::Other)
    }
}

/// O estado de cada item da lista para o snapshot v2, na ordem dela. `dir` é a pasta-fonte: só um
/// symlink cujo alvo é, em texto, `<dir>/<caminho>` conta como `linked`. Valida cada caminho de novo
/// (denylist) e, como o relatório, não abre arquivo nem segue symlink.
pub fn dotfile_entries(home: &Path, dir: &Path, list: &[String]) -> Result<Vec<DotfileEntry>> {
    list.iter()
        .map(|path| {
            validate_path(path).with_context(|| format!("dotfile {path:?}"))?;
            Ok(DotfileEntry {
                path: path.clone(),
                state: dotfile_state(dir, path, &observe(home, path)?),
            })
        })
        .collect()
}

/// Uma entrada por item da lista, na ordem dela. Valida cada caminho de novo, mesmo vindo do
/// config (que já valida): o relatório não confia em quem o chamou para barrar segredo.
pub fn report(home: &Path, list: &[String]) -> Result<Vec<Entry>> {
    list.iter()
        .map(|path| {
            validate_path(path).with_context(|| format!("dotfile {path:?}"))?;
            Ok(classify(home, path, observe(home, path)?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::dotfiles::{Kind, Observed};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    // --- estados para o snapshot v2 (Fase 2, fatia 6) ---

    fn lista(caminhos: &[&str]) -> Vec<String> {
        caminhos.iter().map(|c| (*c).to_string()).collect()
    }

    #[test]
    fn entries_classifica_ligado_arquivo_ausente_e_outro_na_ordem_da_lista() {
        use nodus_core::snapshot::DotfileState::{File, Linked, Missing, Other};
        let tmp = TmpDir::novo();
        let home = tmp.path().join("home");
        let dir = tmp.path().join("fonte");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(dir.join(".config/git")).unwrap();
        fs::write(dir.join(".bashrc"), "x").unwrap();
        fs::write(dir.join(".config/git/config"), "x").unwrap();
        symlink(dir.join(".bashrc"), home.join(".bashrc")).unwrap();
        fs::write(home.join(".gitconfig"), "oi").unwrap();
        symlink("/etc/hostname", home.join(".zshrc")).unwrap();
        fs::create_dir_all(home.join(".tmux.conf")).unwrap();
        fs::create_dir_all(home.join(".config/git")).unwrap();
        symlink(
            dir.join(".config/git/config"),
            home.join(".config/git/config"),
        )
        .unwrap();
        let entradas = dotfile_entries(
            &home,
            &dir,
            &lista(&[
                ".bashrc",
                ".gitconfig",
                ".vimrc",
                ".zshrc",
                ".tmux.conf",
                ".config/git/config",
            ]),
        )
        .unwrap();
        let visto: Vec<(&str, nodus_core::snapshot::DotfileState)> = entradas
            .iter()
            .map(|e| (e.path.as_str(), e.state))
            .collect();
        assert_eq!(
            visto,
            [
                (".bashrc", Linked),
                (".gitconfig", File),
                (".vimrc", Missing),
                (".zshrc", Other),
                (".tmux.conf", Other),
                (".config/git/config", Linked),
            ]
        );
    }

    #[test]
    fn entries_symlink_quebrado_para_a_fonte_ainda_e_ligado_e_nao_segue_nada() {
        // Só o texto do alvo conta: a fonte pode nem ter o arquivo (ligado por engano) e o link vale.
        use nodus_core::snapshot::DotfileState::Linked;
        let tmp = TmpDir::novo();
        let home = tmp.path().join("home");
        let dir = tmp.path().join("fonte");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&dir).unwrap();
        symlink(dir.join(".bashrc"), home.join(".bashrc")).unwrap();
        let entradas = dotfile_entries(&home, &dir, &lista(&[".bashrc"])).unwrap();
        assert_eq!(entradas[0].state, Linked);
    }

    #[test]
    fn entries_recusa_caminho_da_denylist_mesmo_vindo_de_fora_do_config() {
        let tmp = TmpDir::novo();
        let home = tmp.path().join("home");
        let dir = tmp.path().join("fonte");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&dir).unwrap();
        for ruim in [
            ".ssh/id_rsa",
            ".gnupg/x",
            ".config/gh/hosts.yml",
            "../x",
            "/etc/passwd",
        ] {
            let erro = format!(
                "{:#}",
                dotfile_entries(&home, &dir, &lista(&[".bashrc", ruim])).unwrap_err()
            );
            assert!(erro.contains(ruim), "{ruim}: {erro}");
        }
    }

    #[test]
    fn entries_nao_abre_o_arquivo() {
        use nodus_core::snapshot::DotfileState::File;
        let tmp = TmpDir::novo();
        let home = tmp.path().join("home");
        let dir = tmp.path().join("fonte");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(home.join(".bashrc"), "segredo").unwrap();
        fs::set_permissions(home.join(".bashrc"), fs::Permissions::from_mode(0o000)).unwrap();
        let entradas = dotfile_entries(&home, &dir, &lista(&[".bashrc"])).unwrap();
        assert_eq!(entradas[0].state, File);
    }

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
