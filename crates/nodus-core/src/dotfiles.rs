//! Relatório de dotfiles (ADR 0021): lista curada de caminhos relativos ao `$HOME`, validada contra
//! uma denylist de segredos, e a classificação do que o I/O observou. Núcleo puro: quem toca o
//! disco (só `symlink_metadata` e `read_link`, nunca o conteúdo) é o CLI.

use serde::Serialize;
use std::ffi::OsString;
use std::path::{Component, Path};
use thiserror::Error;

/// Lista padrão. `.gtkrc-2.0` e os `rc` do KDE ficam fora de propósito: o desktop os reescreve.
pub const DEFAULT_PATHS: &[&str] = &[
    ".bashrc",
    ".bash_profile",
    ".bash_logout",
    ".profile",
    ".gitconfig",
    ".config/git/config",
    ".zshrc",
    ".vimrc",
    ".tmux.conf",
];

/// Primeiro componente que nunca entra na lista (segredos e credenciais). Fixa, não configurável.
const DENIED_FIRST: &[&str] = &[
    ".gnupg", ".aws", ".docker", ".pki", ".mozilla", ".npmrc",
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DotfileError {
    #[error("caminho vazio")]
    Empty,
    #[error("o caminho precisa ser relativo ao HOME, não absoluto")]
    Absolute,
    #[error("componente inválido (vazio, `.`, `..` ou byte nulo)")]
    BadComponent,
    #[error("caminho de segredo ou credencial, que o nodus não lista")]
    Denied,
}

/// Aceita só caminho relativo ao `$HOME`, sem `.`, `..` nem componente vazio, e fora da denylist.
pub fn validate_path(path: &str) -> Result<(), DotfileError> {
    if path.is_empty() {
        return Err(DotfileError::Empty);
    }
    if path.starts_with('/') {
        return Err(DotfileError::Absolute);
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || p.contains('\0'))
    {
        return Err(DotfileError::BadComponent);
    }
    if is_denied(&parts) {
        return Err(DotfileError::Denied);
    }
    Ok(())
}

/// Compara por componente, não por prefixo de texto: `.sshrc` não é `.ssh`.
fn is_denied(parts: &[&str]) -> bool {
    let first = parts[0];
    let last = parts[parts.len() - 1];
    DENIED_FIRST.contains(&first)
        || first.starts_with(".claude")
        || (first == ".config" && parts.get(1) == Some(&"gh"))
        || last.ends_with("_history")
}

/// A lista padrão mais os extras (sem repetir, no fim), menos os pulados. Não valida: quem lê o
/// config valida cada item com `validate_path`.
pub fn effective_list(extra: &[String], skip: &[String]) -> Vec<String> {
    let mut list: Vec<String> = DEFAULT_PATHS.iter().map(|p| (*p).to_string()).collect();
    for path in extra {
        if !list.contains(path) {
            list.push(path.clone());
        }
    }
    list.retain(|path| !skip.contains(path));
    list
}

/// O que o I/O viu num caminho, sem seguir symlink e sem abrir o arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    Missing,
    File { size: u64 },
    Dir,
    Symlink { target: String },
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    Missing,
    File { size: u64 },
    Dir,
    Symlink { target: String, outside_home: bool },
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub path: String,
    #[serde(flatten)]
    pub kind: Kind,
}

/// Transforma a observação em entrada. Para symlink, marca se o alvo sai do `$HOME`, só por texto
/// (nada é seguido nem lido).
pub fn classify(home: &Path, path: &str, observed: Observed) -> Entry {
    let kind = match observed {
        Observed::Missing => Kind::Missing,
        Observed::File { size } => Kind::File { size },
        Observed::Dir => Kind::Dir,
        Observed::Other => Kind::Other,
        Observed::Symlink { target } => {
            let outside_home = leaves_home(home, path, &target);
            Kind::Symlink {
                target,
                outside_home,
            }
        }
    };
    Entry {
        path: path.to_string(),
        kind,
    }
}

/// O alvo, resolvido a partir da pasta do link, cai fora do `$HOME`? Normalização léxica: `..`
/// sobe um nível, `.` some. Compara componente a componente (`/home/u2` não está em `/home/u`).
fn leaves_home(home: &Path, link: &str, target: &str) -> bool {
    let link_dir = home.join(link).parent().map(Path::to_path_buf);
    let resolved = link_dir.unwrap_or_else(|| home.to_path_buf()).join(target);
    !normalize(&resolved).starts_with(&normalize(home))
}

fn normalize(path: &Path) -> Vec<OsString> {
    let mut stack: Vec<OsString> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => stack.push(name.to_os_string()),
            Component::ParentDir => {
                stack.pop();
            }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    stack
}

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

    #[test]
    fn json_tem_path_kind_e_os_campos_do_tipo() {
        let json = |entry: Entry| serde_json::to_value(entry).unwrap();
        assert_eq!(
            json(classify_at(".bashrc", Observed::File { size: 7 })),
            serde_json::json!({"path": ".bashrc", "kind": "file", "size": 7})
        );
        assert_eq!(
            json(classify_at(".gitconfig", Observed::Missing)),
            serde_json::json!({"path": ".gitconfig", "kind": "missing"})
        );
        assert_eq!(
            json(classify_at(
                ".profile",
                Observed::Symlink {
                    target: "/etc/profile".into()
                }
            )),
            serde_json::json!({
                "path": ".profile",
                "kind": "symlink",
                "target": "/etc/profile",
                "outside_home": true
            })
        );
    }
}
