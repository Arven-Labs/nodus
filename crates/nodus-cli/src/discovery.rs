#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
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
