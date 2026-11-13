#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ParseError;
    use crate::runner::{CommandRunner, RunnerError};
    use std::cell::RefCell;

    const NA_MAIN: &str = "*\tmain\t1a2b3c4\torigin/main\tahead 1\n \tdev\t9f8e7d6\t\t\n";
    const SO_DEV: &str = " \tdev\t9f8e7d6\t\t\n";

    fn falhou(programa: &str, code: i32) -> RunnerError {
        RunnerError::Failed {
            program: programa.into(),
            code: Some(code),
        }
    }

    /// Runner falso: responde pelo subcomando do git e guarda a linha completa de cada chamada.
    struct Falso {
        for_each_ref: Result<String, RunnerError>,
        rev_parse: Result<String, RunnerError>,
        config: Result<String, RunnerError>,
        fetch: Result<String, RunnerError>,
        chamadas: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Falso {
        fn novo() -> Self {
            Self {
                for_each_ref: Ok(NA_MAIN.into()),
                rev_parse: Ok("abc1234\n".into()),
                config: Ok("git@github.com:exemplo/repo.git\n".into()),
                fetch: Ok(String::new()),
                chamadas: RefCell::new(vec![]),
            }
        }
        fn subcomandos(&self) -> Vec<String> {
            self.chamadas
                .borrow()
                .iter()
                .map(|(_, args)| subcomando(args))
                .collect()
        }
    }

    fn subcomando(args: &[String]) -> String {
        const CONHECIDOS: [&str; 4] = ["for-each-ref", "rev-parse", "config", "fetch"];
        args.iter()
            .find(|a| CONHECIDOS.contains(&a.as_str()) || ["status", "pull", "checkout", "merge", "reset"].contains(&a.as_str()))
            .cloned()
            .unwrap_or_else(|| panic!("subcomando inesperado: {args:?}"))
    }

    impl CommandRunner for Falso {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError> {
            let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            let sub = subcomando(&args);
            self.chamadas
                .borrow_mut()
                .push((program.to_string(), args));
            match sub.as_str() {
                "for-each-ref" => self.for_each_ref.clone(),
                "rev-parse" => self.rev_parse.clone(),
                "config" => self.config.clone(),
                "fetch" => self.fetch.clone(),
                outro => panic!("comando proibido no teste: {outro}"),
            }
        }
    }

    fn inspecionar(runner: &Falso, buscar: bool) -> RepoStatus {
        inspect_repo(runner, "repo", "/home/u/ws/repo", buscar)
    }

    // --- parse_branches ---

    #[test]
    fn branch_atual_a_frente_e_atras() {
        let linhas =
            parse_branches("*\tmain\t1a2b3c4\torigin/main\tahead 2, behind 1\n").unwrap();
        assert_eq!(
            linhas,
            vec![BranchLine {
                current: true,
                name: "main".into(),
                commit: "1a2b3c4".into(),
                upstream: Some("origin/main".into()),
                ahead: Some(2),
                behind: Some(1),
                gone: false,
            }]
        );
    }

    #[test]
    fn so_a_frente_so_atras_e_em_dia() {
        let a = parse_branches("*\tm\tc1\torigin/m\tahead 3\n").unwrap();
        assert_eq!((a[0].ahead, a[0].behind), (Some(3), Some(0)));
        let b = parse_branches("*\tm\tc1\torigin/m\tbehind 4\n").unwrap();
        assert_eq!((b[0].ahead, b[0].behind), (Some(0), Some(4)));
        let c = parse_branches("*\tm\tc1\torigin/m\t\n").unwrap();
        assert_eq!((c[0].ahead, c[0].behind), (Some(0), Some(0)));
    }

    #[test]
    fn upstream_que_sumiu_e_gone() {
        let l = parse_branches("*\tm\tc1\torigin/m\tgone\n").unwrap();
        assert!(l[0].gone);
        assert_eq!((l[0].ahead, l[0].behind), (None, None));
    }

    #[test]
    fn branch_sem_upstream_nao_tem_ahead_nem_behind() {
        let l = parse_branches(SO_DEV).unwrap();
        assert!(!l[0].current);
        assert_eq!(l[0].upstream, None);
        assert_eq!((l[0].ahead, l[0].behind), (None, None));
    }

    #[test]
    fn saida_vazia_e_lista_vazia() {
        assert_eq!(parse_branches("").unwrap(), vec![]);
    }

    #[test]
    fn numero_de_colunas_errado_e_erro_com_a_linha() {
        let erro = parse_branches("*\tmain\tc1\torigin/main\n").unwrap_err();
        assert!(matches!(erro, ParseError::InvalidLine { number: 1, .. }));
        assert!(parse_branches("*\tmain\tc1\torigin/main\t\tx\n").is_err());
    }

    #[test]
    fn caractere_de_controle_e_erro() {
        assert!(parse_branches("*\tma\x1bin\tc1\t\t\n").is_err());
        assert!(parse_branches("*\tmain\tc1\torigin/ma\x07in\t\n").is_err());
    }

    #[test]
    fn nome_vazio_ou_track_desconhecido_e_erro() {
        assert!(parse_branches("*\t\tc1\t\t\n").is_err());
        assert!(parse_branches("*\tm\tc1\torigin/m\tsideways 3\n").is_err());
        assert!(parse_branches("*\tm\tc1\torigin/m\tahead x\n").is_err());
    }

    // --- linha de comando isolada (receita do ADR 0009) ---

    #[test]
    fn linha_isolada_tem_a_receita_do_adr_0009() {
        let linha = isolated_git_args("/home/u/ws/repo", &["for-each-ref"]);
        for var in [
            "GIT_CONFIG_NOSYSTEM=1",
            "GIT_CONFIG_GLOBAL=/dev/null",
            "GIT_ALLOW_PROTOCOL=file:https:ssh",
            "GIT_TERMINAL_PROMPT=0",
        ] {
            assert!(linha.contains(&var.to_string()), "falta {var}");
        }
        for par in [
            "core.fsmonitor=false",
            "core.hooksPath=/dev/null",
            "core.sshCommand=ssh",
            "core.gitProxy=",
            "credential.helper=",
            "protocol.ext.allow=never",
        ] {
            let i = linha.iter().position(|a| a == par).unwrap_or_else(|| panic!("falta {par}"));
            assert_eq!(linha[i - 1], "-c", "{par} precisa vir depois de -c");
        }
    }

    #[test]
    fn linha_isolada_variaveis_antes_do_git_e_repo_depois_do_c_maiusculo() {
        let linha = isolated_git_args("/home/u/ws/repo", &["rev-parse", "--short", "HEAD"]);
        let git = linha.iter().position(|a| a == "git").unwrap();
        let var = linha.iter().position(|a| a == "GIT_CONFIG_GLOBAL=/dev/null").unwrap();
        assert!(var < git, "variáveis de ambiente vêm antes de `git`");
        let c = linha.iter().position(|a| a == "-C").unwrap();
        assert_eq!(linha[c + 1], "/home/u/ws/repo");
        assert_eq!(&linha[linha.len() - 3..], ["rev-parse", "--short", "HEAD"]);
    }

    // --- inspect_repo: estado ---

    #[test]
    fn repo_na_main_com_upstream_a_frente() {
        let s = inspecionar(&Falso::novo(), false);
        assert_eq!(s.name, "repo");
        assert_eq!(s.path, "/home/u/ws/repo");
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.commit.as_deref(), Some("1a2b3c4"));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.ahead, s.behind), (Some(1), Some(0)));
        assert_eq!(s.origin_url.as_deref(), Some("git@github.com:exemplo/repo.git"));
        assert_eq!(s.fetch, None);
        assert_eq!(s.error, None);
    }

    #[test]
    fn head_solto_usa_o_commit_do_rev_parse() {
        let mut r = Falso::novo();
        r.for_each_ref = Ok(SO_DEV.into());
        let s = inspecionar(&r, false);
        assert_eq!(s.branch, None);
        assert_eq!(s.commit.as_deref(), Some("abc1234"));
        assert_eq!(s.error, None);
    }

    #[test]
    fn repo_sem_commits_nao_e_erro() {
        let mut r = Falso::novo();
        r.for_each_ref = Ok(String::new());
        r.rev_parse = Err(falhou("env", 128));
        let s = inspecionar(&r, false);
        assert_eq!((s.branch, s.commit), (None, None));
        assert_eq!(s.error, None);
    }

    #[test]
    fn sem_remoto_origin_nao_e_erro() {
        let mut r = Falso::novo();
        r.config = Err(falhou("env", 1));
        let s = inspecionar(&r, false);
        assert_eq!(s.origin_url, None);
        assert_eq!(s.error, None);
    }

    #[test]
    fn falha_do_git_vira_erro_do_repo_e_nao_panico() {
        let mut r = Falso::novo();
        r.for_each_ref = Err(falhou("env", 128));
        let s = inspecionar(&r, false);
        assert!(s.error.is_some());
        assert_eq!((s.branch, s.commit, s.origin_url), (None, None, None));
    }

    #[test]
    fn controle_na_saida_do_git_vira_erro_do_repo() {
        let mut r = Falso::novo();
        r.for_each_ref = Ok("*\tma\x1b[31min\tc1\t\t\n".into());
        assert!(inspecionar(&r, false).error.is_some());
    }

    #[test]
    fn controle_na_url_do_origin_vira_erro_do_repo() {
        let mut r = Falso::novo();
        r.config = Ok("https://x/\x1b]0;pwn\x07\n".into());
        let s = inspecionar(&r, false);
        assert!(s.error.is_some());
        assert_eq!(s.origin_url, None);
    }

    #[test]
    fn caminho_nao_absoluto_e_recusado_sem_chamar_o_git() {
        let r = Falso::novo();
        let s = inspect_repo(&r, "x", "-x", false);
        assert!(s.error.is_some());
        assert!(r.chamadas.borrow().is_empty());
    }

    // --- somente leitura e isolamento em toda chamada ---

    #[test]
    fn so_chama_subcomandos_de_leitura_e_nunca_status() {
        let r = Falso::novo();
        inspecionar(&r, true);
        for sub in r.subcomandos() {
            assert!(
                ["for-each-ref", "rev-parse", "config", "fetch"].contains(&sub.as_str()),
                "subcomando proibido: {sub}"
            );
        }
    }

    #[test]
    fn toda_chamada_vai_por_env_e_isolada() {
        let r = Falso::novo();
        inspecionar(&r, true);
        assert!(!r.chamadas.borrow().is_empty());
        for (programa, args) in r.chamadas.borrow().iter() {
            assert_eq!(programa, "env");
            assert!(args.contains(&"core.hooksPath=/dev/null".to_string()));
            assert!(args.contains(&"GIT_CONFIG_GLOBAL=/dev/null".to_string()));
        }
    }

    // --- fetch ---

    #[test]
    fn sem_pedir_fetch_nao_chama_fetch() {
        let r = Falso::novo();
        inspecionar(&r, false);
        assert!(!r.subcomandos().contains(&"fetch".to_string()));
    }

    #[test]
    fn fetch_vem_antes_de_ler_as_refs_e_usa_os_flags_do_adr() {
        let r = Falso::novo();
        let s = inspecionar(&r, true);
        assert_eq!(s.fetch, Some(FetchResult::Done));
        let subs = r.subcomandos();
        let fetch = subs.iter().position(|x| x == "fetch").unwrap();
        let refs = subs.iter().position(|x| x == "for-each-ref").unwrap();
        assert!(fetch < refs);
        let chamadas = r.chamadas.borrow();
        let args = &chamadas[fetch].1;
        for flag in ["--no-recurse-submodules", "--no-auto-maintenance", "--upload-pack=git-upload-pack"] {
            assert!(args.contains(&flag.to_string()), "falta {flag}");
        }
        assert_eq!(args.last().map(String::as_str), Some("origin"));
    }

    #[test]
    fn fetch_que_falha_nao_apaga_o_estado() {
        let mut r = Falso::novo();
        r.fetch = Err(falhou("env", 128));
        let s = inspecionar(&r, true);
        assert!(matches!(s.fetch, Some(FetchResult::Failed(_))));
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.error, None);
    }

    #[test]
    fn fetch_sem_origin_falha_sem_chamar_o_git() {
        let mut r = Falso::novo();
        r.config = Err(falhou("env", 1));
        let s = inspecionar(&r, true);
        assert_eq!(s.fetch, Some(FetchResult::Failed("sem remoto origin".into())));
        assert!(!r.subcomandos().contains(&"fetch".to_string()));
    }

    #[test]
    fn serializa_fetch_em_snake_case() {
        assert_eq!(serde_json::to_string(&FetchResult::Done).unwrap(), "\"done\"");
        assert_eq!(
            serde_json::to_string(&FetchResult::Failed("x".into())).unwrap(),
            "{\"failed\":\"x\"}"
        );
    }
}
