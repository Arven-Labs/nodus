//! Descoberta de repositórios Git: só filhos diretos de uma pasta, sem seguir symlink (T2).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Um repositório encontrado: nome da pasta e caminho real (absoluto).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoDir {
    pub name: String,
    pub path: PathBuf,
}

/// Lista os filhos diretos de `root` cuja pasta `.git` é uma **pasta real**. Ignora, em silêncio:
/// symlinks (para o repo ou para o `.git`), `.git` que é arquivo (worktree/submódulo, aponta para
/// fora) e nomes com caractere de controle ou fora de UTF-8 (não vão para o terminal). Ordem alfabética.
pub fn find_repos(root: &Path) -> io::Result<Vec<RepoDir>> {
    let root = fs::canonicalize(root)?;
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} não é uma pasta", root.display()),
        ));
    }
    let mut repos = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.chars().any(char::is_control) {
            continue;
        }
        let path = entry.path();
        let is_real_dir = |p: &Path| {
            fs::symlink_metadata(p)
                .map(|m| m.file_type().is_dir())
                .unwrap_or(false)
        };
        if !is_real_dir(&path) || !is_real_dir(&path.join(".git")) {
            continue;
        }
        repos.push(RepoDir { name, path });
    }
    repos.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(repos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONTADOR: AtomicUsize = AtomicUsize::new(0);

    /// Diretório temporário único, apagado no fim. Nunca toca no `$HOME` real.
    struct Tmp(PathBuf);

    impl Tmp {
        fn novo() -> Self {
            let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("nodus-disc-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Tmp(fs::canonicalize(dir).unwrap())
        }
        fn repo(&self, nome: &str) {
            fs::create_dir_all(self.0.join(nome).join(".git")).unwrap();
        }
        fn pasta(&self, nome: &str) {
            fs::create_dir_all(self.0.join(nome)).unwrap();
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn nomes(repos: &[RepoDir]) -> Vec<&str> {
        repos.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn acha_filhos_diretos_com_git_em_ordem_alfabetica() {
        let t = Tmp::novo();
        t.repo("zeta");
        t.repo("alfa");
        t.repo("meio");
        let repos = find_repos(t.path()).unwrap();
        assert_eq!(nomes(&repos), ["alfa", "meio", "zeta"]);
        assert_eq!(repos[0].path, t.path().join("alfa"));
    }

    #[test]
    fn ignora_pasta_sem_git() {
        let t = Tmp::novo();
        t.repo("repo");
        t.pasta("so-pasta");
        assert_eq!(nomes(&find_repos(t.path()).unwrap()), ["repo"]);
    }

    #[test]
    fn nao_desce_em_netos() {
        let t = Tmp::novo();
        t.repo("grupo/repo");
        t.pasta("grupo");
        assert_eq!(find_repos(t.path()).unwrap(), vec![]);
    }

    #[test]
    fn ignora_git_que_e_arquivo_worktree_ou_submodulo() {
        let t = Tmp::novo();
        t.pasta("wt");
        fs::write(t.path().join("wt/.git"), "gitdir: /em/outro/lugar\n").unwrap();
        assert_eq!(find_repos(t.path()).unwrap(), vec![]);
    }

    #[test]
    fn ignora_symlink_para_repo() {
        let t = Tmp::novo();
        let fora = Tmp::novo();
        fora.repo("real");
        symlink(fora.path().join("real"), t.path().join("atalho")).unwrap();
        assert_eq!(find_repos(t.path()).unwrap(), vec![]);
    }

    #[test]
    fn ignora_git_que_e_symlink() {
        let t = Tmp::novo();
        let fora = Tmp::novo();
        fora.pasta("gitdir");
        t.pasta("repo");
        symlink(fora.path().join("gitdir"), t.path().join("repo/.git")).unwrap();
        assert_eq!(find_repos(t.path()).unwrap(), vec![]);
    }

    #[test]
    fn nome_com_caractere_de_controle_e_ignorado() {
        let t = Tmp::novo();
        t.repo("ok");
        t.repo("ma\x1bl");
        assert_eq!(nomes(&find_repos(t.path()).unwrap()), ["ok"]);
    }

    #[test]
    fn raiz_que_e_symlink_para_pasta_funciona() {
        let t = Tmp::novo();
        let fora = Tmp::novo();
        fora.repo("a");
        symlink(fora.path(), t.path().join("ws")).unwrap();
        let repos = find_repos(&t.path().join("ws")).unwrap();
        assert_eq!(nomes(&repos), ["a"]);
        assert_eq!(repos[0].path, fora.path().join("a"));
    }

    #[test]
    fn raiz_inexistente_ou_arquivo_e_erro() {
        let t = Tmp::novo();
        assert!(find_repos(&t.path().join("nao-existe")).is_err());
        fs::write(t.path().join("arquivo"), "x").unwrap();
        assert!(find_repos(&t.path().join("arquivo")).is_err());
    }

    #[test]
    fn pasta_vazia_e_lista_vazia() {
        let t = Tmp::novo();
        assert_eq!(find_repos(t.path()).unwrap(), vec![]);
    }
}
