//! Relatório de dotfiles (ADR 0021): lista curada de caminhos relativos ao `$HOME`, validada contra
//! uma denylist de segredos, e a classificação do que o I/O observou. Núcleo puro: quem toca o
//! disco (só `symlink_metadata` e `read_link`, nunca o conteúdo) é o CLI.

use serde::Serialize;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
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
    ".ssh", ".gnupg", ".aws", ".docker", ".pki", ".mozilla", ".npmrc",
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DotfileError {
    #[error("caminho vazio")]
    Empty,
    #[error("o caminho precisa ser relativo ao HOME, não absoluto")]
    Absolute,
    #[error("componente inválido (vazio, `.`, `..` ou caractere de controle)")]
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
    // O caminho vai para a tela (relatório, plano): nada de escape de terminal.
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == ".." || p.chars().any(char::is_control))
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

/// Tamanho máximo de um arquivo adotado: dotfile é pequeno; barra bancos, históricos e blobs.
pub const MAX_ADOPT_BYTES: u64 = 1 << 20;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DirError {
    #[error("a pasta-fonte precisa ser um caminho absoluto")]
    Relative,
    #[error(
        "a pasta-fonte tem componente inválido (vazio, `.`, `..`, fora de UTF-8 ou caractere de controle)"
    )]
    BadComponent,
    #[error("a pasta-fonte não pode ser o próprio HOME")]
    IsHome,
    #[error("a pasta-fonte está dentro de um caminho de segredo ou credencial")]
    Denied,
}

/// A parte da validação que não precisa do HOME: absoluta e sem componente estranho. É o que o
/// config confere ao ler `dotfiles_dir`; o resto (HOME, denylist) é do `validate_dir`, no `adopt`.
pub fn check_dir_shape(dir: &Path) -> Result<(), DirError> {
    if !dir.is_absolute() {
        return Err(DirError::Relative);
    }
    let text = dir.to_str().ok_or(DirError::BadComponent)?;
    let trimmed = if text.len() > 1 {
        text.strip_suffix('/').unwrap_or(text)
    } else {
        text
    };
    // `dir` é absoluto, então `trimmed` começa por `/`.
    if trimmed[1..]
        .split('/')
        .any(|p| p.is_empty() || p == "." || p == ".." || p.chars().any(char::is_control))
    {
        return Err(DirError::BadComponent);
    }
    Ok(())
}

/// A pasta-fonte (ADR 0022): absoluta, sem componente estranho, diferente do HOME e, se estiver
/// dentro dele, fora da denylist de segredos. Pura: não toca o disco (o CLI resolve o caminho real).
pub fn validate_dir(home: &Path, dir: &Path) -> Result<(), DirError> {
    check_dir_shape(dir)?;
    if dir == home {
        return Err(DirError::IsHome);
    }
    if let Ok(relative) = dir.strip_prefix(home) {
        let parts: Vec<&str> = relative.iter().filter_map(|c| c.to_str()).collect();
        if !parts.is_empty() && is_denied(&parts) {
            return Err(DirError::Denied);
        }
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdoptError {
    #[error("caminho inválido: {0}")]
    Path(#[from] DotfileError),
    #[error("pasta-fonte inválida: {0}")]
    Dir(#[from] DirError),
    #[error("o arquivo não existe")]
    Missing,
    #[error("o caminho já é um symlink; o nodus só adota arquivo comum")]
    Symlink,
    #[error("o caminho não é um arquivo comum (pasta, dispositivo etc.)")]
    NotAFile,
    #[error("o arquivo tem {0} bytes; o limite é 1 MiB")]
    TooBig(u64),
}

/// O que o `adopt` vai fazer, calculado a partir do que o I/O observou. Dado, sem efeito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptPlan {
    pub path: String,
    /// `<home>/<caminho>`: o arquivo real que será trocado por um symlink.
    pub source: PathBuf,
    /// `<pasta-fonte>/<caminho>`: onde a cópia será criada (nunca sobrescreve).
    pub dest: PathBuf,
    pub size: u64,
    /// Só os 9 bits de permissão; setuid, setgid e sticky ficam de fora.
    pub mode: u32,
}

/// Valida tudo o que dá para validar sem tocar o disco. `mode` é o `st_mode` observado.
pub fn plan_adopt(
    home: &Path,
    dir: &Path,
    path: &str,
    observed: &Observed,
    mode: u32,
) -> Result<AdoptPlan, AdoptError> {
    validate_path(path)?;
    validate_dir(home, dir)?;
    let size = match observed {
        Observed::File { size } if *size > MAX_ADOPT_BYTES => {
            return Err(AdoptError::TooBig(*size));
        }
        Observed::File { size } => *size,
        Observed::Missing => return Err(AdoptError::Missing),
        Observed::Symlink { .. } => return Err(AdoptError::Symlink),
        Observed::Dir | Observed::Other => return Err(AdoptError::NotAFile),
    };
    Ok(AdoptPlan {
        path: path.to_string(),
        source: home.join(path),
        dest: dir.join(path),
        size,
        mode: mode & 0o777,
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LinkError {
    #[error("caminho inválido: {0}")]
    Path(#[from] DotfileError),
    #[error("pasta-fonte inválida: {0}")]
    Dir(#[from] DirError),
    #[error("o arquivo na pasta-fonte não existe")]
    SourceMissing,
    #[error("na pasta-fonte, o caminho não é um arquivo comum (pasta, symlink, dispositivo etc.)")]
    SourceNotAFile,
    #[error("o arquivo na pasta-fonte tem {0} bytes; o limite é 1 MiB")]
    SourceTooBig(u64),
    #[error("já está ligado: o destino é um symlink para a pasta-fonte")]
    AlreadyLinked,
    #[error("o destino já existe no HOME; o nodus nunca sobrescreve (apague ou renomeie à mão)")]
    DestExists,
}

/// O que o `link` vai fazer (ADR 0024): criar o symlink `link` apontando para `target`. Dado, sem
/// efeito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPlan {
    pub path: String,
    /// `<home>/<caminho>`: onde o symlink será criado (não pode existir).
    pub link: PathBuf,
    /// `<pasta-fonte>/<caminho>`: o arquivo para o qual o symlink aponta.
    pub target: PathBuf,
    pub size: u64,
}

/// Valida tudo o que dá para validar sem tocar o disco. A fonte precisa ser arquivo comum de até
/// 1 MiB; o destino não pode existir (nem como symlink para outro lugar). Um symlink exatamente
/// para o alvo, comparado por texto como o `adopt` o grava, vira `AlreadyLinked`.
pub fn plan_link(
    home: &Path,
    dir: &Path,
    path: &str,
    source: &Observed,
    dest: &Observed,
) -> Result<LinkPlan, LinkError> {
    validate_path(path)?;
    validate_dir(home, dir)?;
    let target = dir.join(path);
    let size = match source {
        Observed::File { size } if *size > MAX_ADOPT_BYTES => {
            return Err(LinkError::SourceTooBig(*size));
        }
        Observed::File { size } => *size,
        Observed::Missing => return Err(LinkError::SourceMissing),
        Observed::Symlink { .. } | Observed::Dir | Observed::Other => {
            return Err(LinkError::SourceNotAFile);
        }
    };
    match dest {
        Observed::Missing | Observed::File { .. } => {}
        Observed::Symlink { target: found } if Path::new(found) == target => {
            return Err(LinkError::AlreadyLinked);
        }
        _ => return Err(LinkError::DestExists),
    }
    Ok(LinkPlan {
        path: path.to_string(),
        link: home.join(path),
        target,
        size,
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UndoAdoptError {
    #[error("caminho inválido: {0}")]
    Path(#[from] DotfileError),
    #[error("pasta-fonte inválida: {0}")]
    Dir(#[from] DirError),
    #[error("o caminho no HOME não é mais um symlink; o nodus não toca em arquivo comum")]
    NotALink,
    #[error("o symlink aponta para {found}, não para {expected}; o nodus só desfaz o que criou")]
    WrongTarget { expected: String, found: String },
    #[error("o arquivo na pasta-fonte não existe mais")]
    SourceMissing,
    #[error("na pasta-fonte, o caminho não é um arquivo comum")]
    SourceNotAFile,
    #[error("o arquivo na pasta-fonte tem {0} bytes; o limite é 1 MiB")]
    SourceTooBig(u64),
}

/// O que o `undo` de uma adoção vai fazer (ADR 0023): trocar o symlink `link` por uma cópia de
/// `source`, que fica onde está. Dado, sem efeito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoAdoptPlan {
    pub path: String,
    /// `<home>/<caminho>`: o symlink que será trocado por um arquivo comum.
    pub link: PathBuf,
    /// `<pasta-fonte>/<caminho>`: de onde a cópia sai; não é alterada.
    pub source: PathBuf,
    pub size: u64,
    /// Só os 9 bits de permissão da fonte.
    pub mode: u32,
}

/// `path` e `dir` vêm do histórico, que não é assinado: são revalidados aqui. O symlink precisa
/// apontar exatamente para `<dir>/<path>`, e a fonte ser arquivo comum de até 1 MiB.
pub fn plan_undo_adopt(
    home: &Path,
    dir: &Path,
    path: &str,
    link: &Observed,
    source: &Observed,
    source_mode: u32,
) -> Result<UndoAdoptPlan, UndoAdoptError> {
    validate_path(path)?;
    validate_dir(home, dir)?;
    let source_path = dir.join(path);
    // Comparação de texto, não de `Path` (que ignora a barra final): o alvo tem de ser idêntico ao
    // que o `adopt` gravou.
    let expected = source_path.to_string_lossy().into_owned();
    match link {
        Observed::Symlink { target } if *target == expected => {}
        Observed::Symlink { target } => {
            return Err(UndoAdoptError::WrongTarget {
                expected,
                found: target.clone(),
            });
        }
        _ => return Err(UndoAdoptError::NotALink),
    }
    let size = match source {
        Observed::File { size } if *size > MAX_ADOPT_BYTES => {
            return Err(UndoAdoptError::SourceTooBig(*size));
        }
        Observed::File { size } => *size,
        Observed::Missing => return Err(UndoAdoptError::SourceMissing),
        Observed::Symlink { .. } | Observed::Dir | Observed::Other => {
            return Err(UndoAdoptError::SourceNotAFile);
        }
    };
    Ok(UndoAdoptPlan {
        path: path.to_string(),
        link: home.join(path),
        source: source_path,
        size,
        mode: source_mode & 0o777,
    })
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
    fn recusa_caractere_de_controle_porque_o_caminho_vai_para_a_tela() {
        for path in [
            ".bashrc\n",
            "\u{1b}[31m.bashrc",
            ".config/a\tb",
            ".bash\u{7f}rc",
        ] {
            assert_eq!(
                validate_path(path),
                Err(DotfileError::BadComponent),
                "{path:?}"
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

    // ---- validate_dir: a pasta-fonte (ADR 0022) ----

    #[test]
    fn pasta_fonte_aceita_dentro_ou_fora_do_home() {
        for dir in ["/home/u/dotfiles", "/mnt/hd/dotfiles", "/home/u2/dotfiles"] {
            assert_eq!(
                validate_dir(Path::new(HOME), Path::new(dir)),
                Ok(()),
                "{dir}"
            );
        }
    }

    #[test]
    fn pasta_fonte_recusa_relativa_dot_dotdot_e_controle() {
        for dir in [
            "dotfiles",
            "",
            "/home/u/../u/dotfiles",
            "/home/u/./dotfiles",
            "/home/u/dot\nfiles",
            "/home/u/\u{1b}[31mx",
        ] {
            assert!(
                matches!(
                    validate_dir(Path::new(HOME), Path::new(dir)),
                    Err(DirError::Relative | DirError::BadComponent)
                ),
                "{dir:?}"
            );
        }
    }

    #[test]
    fn pasta_fonte_recusa_a_denylist_e_o_proprio_home() {
        for dir in [
            "/home/u/.ssh",
            "/home/u/.ssh/dotfiles",
            "/home/u/.gnupg/x",
            "/home/u/.config/gh/x",
            "/home/u/.aws",
        ] {
            assert_eq!(
                validate_dir(Path::new(HOME), Path::new(dir)),
                Err(DirError::Denied),
                "{dir}"
            );
        }
        for dir in ["/home/u", "/home/u/"] {
            assert_eq!(
                validate_dir(Path::new(HOME), Path::new(dir)),
                Err(DirError::IsHome),
                "{dir}"
            );
        }
    }

    // ---- plan_adopt: o plano puro, a partir do que o I/O observou ----

    const DIR: &str = "/home/u/dotfiles";
    const REGULAR: u32 = 0o100_000;

    fn plan_for(path: &str, observed: Observed, mode: u32) -> Result<AdoptPlan, AdoptError> {
        plan_adopt(Path::new(HOME), Path::new(DIR), path, &observed, mode)
    }

    #[test]
    fn plano_de_arquivo_comum_guarda_origem_destino_tamanho_e_permissao() {
        let plan = plan_for(".bashrc", Observed::File { size: 1670 }, REGULAR | 0o644).unwrap();
        assert_eq!(plan.path, ".bashrc");
        assert_eq!(plan.source, Path::new("/home/u/.bashrc"));
        assert_eq!(plan.dest, Path::new("/home/u/dotfiles/.bashrc"));
        assert_eq!(plan.size, 1670);
        assert_eq!(plan.mode, 0o644);
    }

    #[test]
    fn plano_aninhado_espelha_o_caminho_na_pasta_fonte() {
        let plan = plan_for(
            ".config/git/config",
            Observed::File { size: 10 },
            REGULAR | 0o600,
        )
        .unwrap();
        assert_eq!(plan.dest, Path::new("/home/u/dotfiles/.config/git/config"));
        assert_eq!(plan.mode, 0o600);
    }

    #[test]
    fn plano_tira_setuid_setgid_e_sticky_da_permissao() {
        for special in [0o4000, 0o2000, 0o1000, 0o7000] {
            let plan = plan_for(
                ".bashrc",
                Observed::File { size: 1 },
                REGULAR | special | 0o755,
            )
            .unwrap();
            assert_eq!(plan.mode, 0o755, "{special:o}");
        }
    }

    #[test]
    fn plano_aceita_vazio_e_exatamente_o_limite_mas_recusa_um_byte_a_mais() {
        assert!(plan_for(".bashrc", Observed::File { size: 0 }, REGULAR | 0o644).is_ok());
        let limite = MAX_ADOPT_BYTES;
        assert!(plan_for(".bashrc", Observed::File { size: limite }, REGULAR | 0o644).is_ok());
        assert_eq!(
            plan_for(
                ".bashrc",
                Observed::File { size: limite + 1 },
                REGULAR | 0o644
            ),
            Err(AdoptError::TooBig(limite + 1))
        );
    }

    #[test]
    fn plano_recusa_o_que_nao_e_arquivo_comum() {
        assert_eq!(
            plan_for(".bashrc", Observed::Missing, 0),
            Err(AdoptError::Missing)
        );
        assert_eq!(
            plan_for(".bashrc", Observed::Symlink { target: "x".into() }, 0),
            Err(AdoptError::Symlink)
        );
        assert_eq!(
            plan_for(".config/git", Observed::Dir, 0),
            Err(AdoptError::NotAFile)
        );
        assert_eq!(
            plan_for(".bashrc", Observed::Other, 0),
            Err(AdoptError::NotAFile)
        );
    }

    #[test]
    fn plano_recusa_caminho_invalido_antes_de_olhar_o_resto() {
        for (path, erro) in [
            (".ssh/config", DotfileError::Denied),
            ("../.bashrc", DotfileError::BadComponent),
            ("/etc/passwd", DotfileError::Absolute),
            ("", DotfileError::Empty),
        ] {
            assert_eq!(
                plan_for(path, Observed::File { size: 1 }, REGULAR | 0o644),
                Err(AdoptError::Path(erro)),
                "{path:?}"
            );
        }
    }

    #[test]
    fn plano_recusa_pasta_fonte_invalida_e_pasta_igual_ao_home() {
        let observed = Observed::File { size: 1 };
        assert_eq!(
            plan_adopt(
                Path::new(HOME),
                Path::new("/home/u/.ssh/dot"),
                ".bashrc",
                &observed,
                REGULAR | 0o644
            ),
            Err(AdoptError::Dir(DirError::Denied))
        );
        // Com a pasta-fonte igual ao HOME, o destino seria o próprio arquivo.
        assert_eq!(
            plan_adopt(
                Path::new(HOME),
                Path::new(HOME),
                ".bashrc",
                &observed,
                REGULAR | 0o644
            ),
            Err(AdoptError::Dir(DirError::IsHome))
        );
    }

    // ---- plan_link: ligar um arquivo da pasta-fonte ao HOME (ADR 0024) ----

    fn link_for(path: &str, source: Observed, dest: Observed) -> Result<LinkPlan, LinkError> {
        plan_link(Path::new(HOME), Path::new(DIR), path, &source, &dest)
    }

    #[test]
    fn link_com_fonte_comum_e_destino_ausente_guarda_link_alvo_e_tamanho() {
        let plan = link_for(".bashrc", Observed::File { size: 1670 }, Observed::Missing).unwrap();
        assert_eq!(plan.path, ".bashrc");
        assert_eq!(plan.link, Path::new("/home/u/.bashrc"));
        assert_eq!(plan.target, Path::new("/home/u/dotfiles/.bashrc"));
        assert_eq!(plan.size, 1670);
    }

    #[test]
    fn link_aninhado_espelha_o_caminho_na_pasta_fonte() {
        let plan = link_for(
            ".config/git/config",
            Observed::File { size: 10 },
            Observed::Missing,
        )
        .unwrap();
        assert_eq!(plan.link, Path::new("/home/u/.config/git/config"));
        assert_eq!(
            plan.target,
            Path::new("/home/u/dotfiles/.config/git/config")
        );
    }

    #[test]
    fn link_recusa_destino_que_existe_seja_qual_for_o_tipo() {
        for dest in [
            Observed::File { size: 5 },
            Observed::Dir,
            Observed::Other,
            Observed::Symlink {
                target: "/etc/profile".into(),
            },
            Observed::Symlink {
                target: "/home/u/dotfiles/.zshrc".into(),
            },
        ] {
            let erro = link_for(".bashrc", Observed::File { size: 5 }, dest.clone()).unwrap_err();
            assert_eq!(erro, LinkError::DestExists, "{dest:?}");
        }
    }

    #[test]
    fn link_diz_que_ja_esta_ligado_so_quando_o_alvo_e_exatamente_o_da_fonte() {
        let ligado = Observed::Symlink {
            target: "/home/u/dotfiles/.bashrc".into(),
        };
        assert_eq!(
            link_for(".bashrc", Observed::File { size: 5 }, ligado).unwrap_err(),
            LinkError::AlreadyLinked
        );
        // barra final ou caminho relativo não são o que o nodus grava: contam como outro destino
        for alvo in ["/home/u/dotfiles/.bashrc/", "dotfiles/.bashrc", ".bashrc"] {
            let erro = link_for(
                ".bashrc",
                Observed::File { size: 5 },
                Observed::Symlink {
                    target: alvo.to_string(),
                },
            )
            .unwrap_err();
            assert_eq!(erro, LinkError::DestExists, "{alvo}");
        }
    }

    #[test]
    fn link_recusa_fonte_ausente_ou_que_nao_e_arquivo_comum() {
        let casos = [
            (Observed::Missing, LinkError::SourceMissing),
            (Observed::Dir, LinkError::SourceNotAFile),
            (Observed::Other, LinkError::SourceNotAFile),
            (
                Observed::Symlink {
                    target: "/etc/passwd".into(),
                },
                LinkError::SourceNotAFile,
            ),
        ];
        for (source, esperado) in casos {
            let erro = link_for(".bashrc", source, Observed::Missing).unwrap_err();
            assert_eq!(erro, esperado);
        }
    }

    #[test]
    fn link_aceita_fonte_de_exatamente_1_mib_e_recusa_um_byte_a_mais() {
        assert!(
            link_for(
                ".bashrc",
                Observed::File {
                    size: MAX_ADOPT_BYTES
                },
                Observed::Missing
            )
            .is_ok()
        );
        assert_eq!(
            link_for(
                ".bashrc",
                Observed::File {
                    size: MAX_ADOPT_BYTES + 1
                },
                Observed::Missing
            )
            .unwrap_err(),
            LinkError::SourceTooBig(MAX_ADOPT_BYTES + 1)
        );
    }

    #[test]
    fn link_revalida_o_caminho_e_a_pasta() {
        for path in [".ssh/config", "../x", "/etc/passwd", ""] {
            let erro = link_for(path, Observed::File { size: 5 }, Observed::Missing).unwrap_err();
            assert!(matches!(erro, LinkError::Path(_)), "{path}: {erro:?}");
        }
        for dir in ["/home/u", "/home/u/.ssh/dots", "relativa"] {
            let erro = plan_link(
                Path::new(HOME),
                Path::new(dir),
                ".bashrc",
                &Observed::File { size: 5 },
                &Observed::Missing,
            )
            .unwrap_err();
            assert!(matches!(erro, LinkError::Dir(_)), "{dir}: {erro:?}");
        }
    }

    // ---- plan_undo_adopt: desfazer a adoção (ADR 0023) ----

    fn undo_for(
        path: &str,
        link: Observed,
        source: Observed,
        mode: u32,
    ) -> Result<UndoAdoptPlan, UndoAdoptError> {
        plan_undo_adopt(Path::new(HOME), Path::new(DIR), path, &link, &source, mode)
    }

    fn link_ok(path: &str) -> Observed {
        Observed::Symlink {
            target: format!("{DIR}/{path}"),
        }
    }

    #[test]
    fn undo_com_symlink_certo_e_fonte_comum_guarda_link_fonte_tamanho_e_permissao() {
        let plan = undo_for(
            ".bashrc",
            link_ok(".bashrc"),
            Observed::File { size: 1670 },
            REGULAR | 0o644,
        )
        .unwrap();
        assert_eq!(plan.path, ".bashrc");
        assert_eq!(plan.link, Path::new("/home/u/.bashrc"));
        assert_eq!(plan.source, Path::new("/home/u/dotfiles/.bashrc"));
        assert_eq!(plan.size, 1670);
        assert_eq!(plan.mode, 0o644);
    }

    #[test]
    fn undo_tira_setuid_setgid_e_sticky_da_permissao() {
        let plan = undo_for(
            ".bashrc",
            link_ok(".bashrc"),
            Observed::File { size: 1 },
            REGULAR | 0o7755,
        )
        .unwrap();
        assert_eq!(plan.mode, 0o755);
    }

    #[test]
    fn undo_recusa_quando_o_home_nao_e_symlink() {
        for link in [
            Observed::Missing,
            Observed::File { size: 5 },
            Observed::Dir,
            Observed::Other,
        ] {
            let erro = undo_for(
                ".bashrc",
                link.clone(),
                Observed::File { size: 5 },
                REGULAR | 0o644,
            )
            .unwrap_err();
            assert_eq!(erro, UndoAdoptError::NotALink, "{link:?}");
        }
    }

    #[test]
    fn undo_recusa_symlink_que_aponta_para_outro_lugar() {
        for alvo in [
            "/etc/profile",
            "/home/u/dotfiles/.zshrc",
            "/home/u/dotfiles/.bashrc/",
            "/home/u/outra/.bashrc",
            ".bashrc",
        ] {
            let erro = undo_for(
                ".bashrc",
                Observed::Symlink {
                    target: alvo.to_string(),
                },
                Observed::File { size: 5 },
                REGULAR | 0o644,
            )
            .unwrap_err();
            assert_eq!(
                erro,
                UndoAdoptError::WrongTarget {
                    expected: "/home/u/dotfiles/.bashrc".to_string(),
                    found: alvo.to_string(),
                },
                "{alvo}"
            );
        }
    }

    #[test]
    fn undo_recusa_fonte_ausente_ou_que_nao_e_arquivo_comum() {
        let casos = [
            (Observed::Missing, UndoAdoptError::SourceMissing),
            (Observed::Dir, UndoAdoptError::SourceNotAFile),
            (Observed::Other, UndoAdoptError::SourceNotAFile),
            (
                Observed::Symlink {
                    target: "/etc/passwd".into(),
                },
                UndoAdoptError::SourceNotAFile,
            ),
        ];
        for (source, esperado) in casos {
            let erro =
                undo_for(".bashrc", link_ok(".bashrc"), source, REGULAR | 0o644).unwrap_err();
            assert_eq!(erro, esperado);
        }
    }

    #[test]
    fn undo_aceita_fonte_de_exatamente_1_mib_e_recusa_um_byte_a_mais() {
        assert!(
            undo_for(
                ".bashrc",
                link_ok(".bashrc"),
                Observed::File {
                    size: MAX_ADOPT_BYTES
                },
                REGULAR | 0o644
            )
            .is_ok()
        );
        assert_eq!(
            undo_for(
                ".bashrc",
                link_ok(".bashrc"),
                Observed::File {
                    size: MAX_ADOPT_BYTES + 1
                },
                REGULAR | 0o644
            )
            .unwrap_err(),
            UndoAdoptError::SourceTooBig(MAX_ADOPT_BYTES + 1)
        );
    }

    #[test]
    fn undo_revalida_o_caminho_e_a_pasta_vindos_do_historico() {
        // Histórico não é assinado (T5): uma linha plantada não pode virar escrita fora da regra.
        for path in [".ssh/authorized_keys", "../x", "/etc/passwd", ""] {
            let erro = undo_for(
                path,
                Observed::Symlink {
                    target: format!("{DIR}/{path}"),
                },
                Observed::File { size: 5 },
                REGULAR | 0o644,
            )
            .unwrap_err();
            assert!(matches!(erro, UndoAdoptError::Path(_)), "{path}: {erro:?}");
        }
        for dir in ["/home/u", "/home/u/.ssh/dots", "relativa"] {
            let erro = plan_undo_adopt(
                Path::new(HOME),
                Path::new(dir),
                ".bashrc",
                &Observed::Symlink {
                    target: format!("{dir}/.bashrc"),
                },
                &Observed::File { size: 5 },
                REGULAR | 0o644,
            )
            .unwrap_err();
            assert!(matches!(erro, UndoAdoptError::Dir(_)), "{dir}: {erro:?}");
        }
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
