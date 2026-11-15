#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TmpDir;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};

    /// Cria `raiz/rel/.git` (um repositório de mentira) e devolve o caminho do repositório.
    fn repo(raiz: &Path, rel: &str) -> PathBuf {
        let pasta = raiz.join(rel);
        fs::create_dir_all(pasta.join(".git")).unwrap();
        pasta
    }

    /// `n` repositórios filhos diretos de `raiz/rel`.
    fn com_repos(raiz: &Path, rel: &str, n: usize) -> PathBuf {
        let pasta = raiz.join(rel);
        fs::create_dir_all(&pasta).unwrap();
        for i in 0..n {
            repo(&pasta, &format!("r{i}"));
        }
        pasta
    }

    fn real(p: &Path) -> PathBuf {
        fs::canonicalize(p).unwrap()
    }

    fn home(tmp: &TmpDir) -> String {
        tmp.path().to_str().unwrap().to_string()
    }

    fn detectar(tmp: &TmpDir) -> Report {
        resolve(None, None, Some(&home(tmp))).unwrap()
    }

    fn candidato(pasta: &Path, repo_count: usize) -> Candidate {
        Candidate {
            path: real(pasta),
            repo_count,
        }
    }

    // --- detecção: candidatos conhecidos sob o HOME ---

    #[test]
    fn detecta_o_candidato_com_mais_repositorios() {
        let tmp = TmpDir::novo();
        let ws = com_repos(tmp.path(), "Documents/workspace", 3);
        let proj = com_repos(tmp.path(), "Projects", 1);
        let r = detectar(&tmp);
        assert_eq!(
            r.chosen,
            Some(Chosen {
                path: real(&ws),
                source: Source::Detected,
                repo_count: 3
            })
        );
        assert_eq!(r.candidates, [candidato(&ws, 3), candidato(&proj, 1)]);
    }

    #[test]
    fn candidatos_saem_ordenados_por_repositorios_decrescente() {
        let tmp = TmpDir::novo();
        // Na lista, `Documents/workspace` vem antes de `dev`, mas tem menos repositórios.
        let ws = com_repos(tmp.path(), "Documents/workspace", 1);
        let dev = com_repos(tmp.path(), "dev", 4);
        let r = detectar(&tmp);
        assert_eq!(r.candidates, [candidato(&dev, 4), candidato(&ws, 1)]);
        assert_eq!(r.chosen.unwrap().path, real(&dev));
    }

    #[test]
    fn empate_vale_a_ordem_da_lista() {
        let tmp = TmpDir::novo();
        // `workspace` vem antes de `Projects` na lista.
        let projects = com_repos(tmp.path(), "Projects", 2);
        let workspace = com_repos(tmp.path(), "workspace", 2);
        let r = detectar(&tmp);
        assert_eq!(r.chosen.unwrap().path, real(&workspace));
        assert_eq!(
            r.candidates,
            [candidato(&workspace, 2), candidato(&projects, 2)]
        );
    }

    #[test]
    fn candidato_sem_repositorio_aparece_e_nao_e_escolhido() {
        let tmp = TmpDir::novo();
        let ws = com_repos(tmp.path(), "Documents/workspace", 2);
        let vazio = tmp.path().join("Projects");
        fs::create_dir_all(&vazio).unwrap();
        let r = detectar(&tmp);
        assert_eq!(r.chosen.unwrap().path, real(&ws));
        assert_eq!(r.candidates, [candidato(&ws, 2), candidato(&vazio, 0)]);
    }

    #[test]
    fn so_candidatos_vazios_nao_escolhem_nenhum() {
        let tmp = TmpDir::novo();
        let vazio = tmp.path().join("Projects");
        fs::create_dir_all(&vazio).unwrap();
        let r = detectar(&tmp);
        assert_eq!(r.chosen, None);
        assert_eq!(r.candidates, [candidato(&vazio, 0)]);
    }

    #[test]
    fn nenhum_candidato_existe() {
        let tmp = TmpDir::novo();
        let r = detectar(&tmp);
        assert_eq!(r.chosen, None);
        assert!(r.candidates.is_empty());
    }

    #[test]
    fn arquivo_no_lugar_de_candidato_e_pulado() {
        let tmp = TmpDir::novo();
        fs::write(tmp.path().join("workspace"), "não sou uma pasta").unwrap();
        let r = detectar(&tmp);
        assert!(r.candidates.is_empty());
        assert_eq!(r.chosen, None);
    }

    #[test]
    fn candidatos_que_resolvem_ao_mesmo_caminho_real_aparecem_uma_vez() {
        let tmp = TmpDir::novo();
        let dev = com_repos(tmp.path(), "dev", 1);
        // `code` vem depois de `dev` na lista e aponta para a mesma pasta.
        symlink(&dev, tmp.path().join("code")).unwrap();
        let r = detectar(&tmp);
        assert_eq!(r.candidates, [candidato(&dev, 1)]);
    }

    #[test]
    fn repositorio_aninhado_nao_conta() {
        let tmp = TmpDir::novo();
        let ws = tmp.path().join("workspace");
        repo(&ws, "grupo/projeto");
        let r = detectar(&tmp);
        assert_eq!(r.chosen, None);
        assert_eq!(r.candidates, [candidato(&ws, 0)]);
    }

    // --- manual: argumento e variável de ambiente ---

    #[test]
    fn argumento_vence_a_variavel_e_a_deteccao() {
        let tmp = TmpDir::novo();
        com_repos(tmp.path(), "Documents/workspace", 5);
        let ambiente = com_repos(tmp.path(), "do-ambiente", 3);
        let informado = com_repos(tmp.path(), "informado", 2);
        let r = resolve(
            Some(&informado),
            Some(ambiente.to_str().unwrap()),
            Some(&home(&tmp)),
        )
        .unwrap();
        assert_eq!(
            r.chosen,
            Some(Chosen {
                path: real(&informado),
                source: Source::Informed,
                repo_count: 2
            })
        );
        assert!(r.candidates.is_empty());
    }

    #[test]
    fn variavel_vence_a_deteccao() {
        let tmp = TmpDir::novo();
        com_repos(tmp.path(), "Documents/workspace", 5);
        let ambiente = com_repos(tmp.path(), "do-ambiente", 1);
        let r = resolve(None, Some(ambiente.to_str().unwrap()), Some(&home(&tmp))).unwrap();
        assert_eq!(
            r.chosen,
            Some(Chosen {
                path: real(&ambiente),
                source: Source::Environment,
                repo_count: 1
            })
        );
        assert!(r.candidates.is_empty());
    }

    #[test]
    fn variavel_vazia_e_ignorada() {
        let tmp = TmpDir::novo();
        let ws = com_repos(tmp.path(), "Documents/workspace", 2);
        let r = resolve(None, Some(""), Some(&home(&tmp))).unwrap();
        assert_eq!(r.chosen.unwrap().path, real(&ws));
    }

    #[test]
    fn variavel_invalida_e_erro_e_nao_cai_na_deteccao() {
        let tmp = TmpDir::novo();
        com_repos(tmp.path(), "Documents/workspace", 2);
        let arquivo = tmp.path().join("arquivo");
        fs::write(&arquivo, "x").unwrap();
        for ruim in [
            "relativo/caminho",
            "/nodus/nao/existe",
            arquivo.to_str().unwrap(),
        ] {
            let erro = resolve(None, Some(ruim), Some(&home(&tmp)))
                .unwrap_err()
                .to_string();
            assert!(erro.contains("NODUS_WORKSPACE"), "{ruim}: {erro}");
        }
    }

    #[test]
    fn argumento_invalido_e_erro() {
        let tmp = TmpDir::novo();
        let arquivo = tmp.path().join("arquivo");
        fs::write(&arquivo, "x").unwrap();
        let inexistente = tmp.path().join("nao-existe");
        assert!(resolve(Some(&inexistente), None, Some(&home(&tmp))).is_err());
        let erro = resolve(Some(&arquivo), None, Some(&home(&tmp)))
            .unwrap_err()
            .to_string();
        assert!(erro.contains("não é uma pasta"), "{erro}");
    }

    #[test]
    fn pasta_informada_sem_repositorios_e_aceita_com_contagem_zero() {
        let tmp = TmpDir::novo();
        let vazia = tmp.path().join("vazia");
        fs::create_dir_all(&vazia).unwrap();
        let r = resolve(Some(&vazia), None, Some(&home(&tmp))).unwrap();
        assert_eq!(r.chosen.unwrap().repo_count, 0);
    }

    #[test]
    fn caminho_informado_e_resolvido_ao_real() {
        let tmp = TmpDir::novo();
        let alvo = com_repos(tmp.path(), "alvo", 1);
        let atalho = tmp.path().join("atalho");
        symlink(&alvo, &atalho).unwrap();
        let r = resolve(Some(&atalho), None, Some(&home(&tmp))).unwrap();
        assert_eq!(r.chosen.unwrap().path, real(&alvo));
    }

    // --- HOME ---

    #[test]
    fn sem_home_e_sem_manual_e_erro_claro() {
        for home in [None, Some(""), Some("relativo")] {
            let erro = resolve(None, None, home).unwrap_err().to_string();
            assert!(erro.contains("HOME"), "{home:?}: {erro}");
        }
    }

    #[test]
    fn com_argumento_o_home_nao_e_necessario() {
        let tmp = TmpDir::novo();
        let ws = com_repos(tmp.path(), "ws", 1);
        let r = resolve(Some(&ws), None, None).unwrap();
        assert_eq!(r.chosen.unwrap().source, Source::Informed);
    }

    // --- JSON ---

    #[test]
    fn json_traz_a_escolha_e_os_candidatos() {
        let r = Report {
            chosen: Some(Chosen {
                path: PathBuf::from("/h/ws"),
                source: Source::Detected,
                repo_count: 3,
            }),
            candidates: vec![
                Candidate {
                    path: PathBuf::from("/h/ws"),
                    repo_count: 3,
                },
                Candidate {
                    path: PathBuf::from("/h/Projects"),
                    repo_count: 0,
                },
            ],
        };
        assert_eq!(
            report_json(&r),
            serde_json::json!({
                "chosen": {"path": "/h/ws", "source": "detected", "repo_count": 3},
                "candidates": [
                    {"path": "/h/ws", "repo_count": 3},
                    {"path": "/h/Projects", "repo_count": 0}
                ]
            })
        );
    }

    #[test]
    fn json_sem_escolha_tem_chosen_nulo_e_os_nomes_das_origens() {
        let vazio = Report {
            chosen: None,
            candidates: vec![],
        };
        assert_eq!(
            report_json(&vazio),
            serde_json::json!({"chosen": null, "candidates": []})
        );
        for (source, nome) in [
            (Source::Informed, "informed"),
            (Source::Environment, "environment"),
            (Source::Detected, "detected"),
        ] {
            let r = Report {
                chosen: Some(Chosen {
                    path: PathBuf::from("/x"),
                    source,
                    repo_count: 0,
                }),
                candidates: vec![],
            };
            assert_eq!(report_json(&r)["chosen"]["source"], nome);
        }
    }
}
