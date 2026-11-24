//! `repos sync` (ADR 0034): decide, por repositório, se dá para avançar com `merge --ff-only`, e avança.
//!
//! O que protege: um **gate** lê toda a config que o Git leria (`config --list --includes
//! --name-only`, que não executa nada) e recusa o repositório com qualquer `filter.*.{clean,
//! smudge,process}`; o `status` e o `merge` só rodam depois dele, isolados como no ADR 0009. O avanço
//! é só `--ff-only`: nunca `pull`, `reset`, `rebase`, troca de branch nem resolução de conflito.

use crate::repos::{FetchResult, RepoStatus, git_error, run_git};
use crate::runner::CommandRunner;
use serde::Serialize;

/// Os argumentos do `merge` depois do `-C <repo>` (ADR 0034, item 2). `submodule.recurse` e
/// `merge.verifySignatures` são anulados porque o `merge` pode tocá-los; `@{u}` é o upstream da branch
/// atual.
pub const MERGE_ARGS: [&str; 9] = [
    "-c",
    "submodule.recurse=false",
    "-c",
    "merge.verifySignatures=false",
    "merge",
    "--ff-only",
    "--no-edit",
    "--no-verify-signatures",
    "@{u}",
];

/// Por que um repositório fica de fora do avanço.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum SkipReason {
    /// A leitura do estado já tinha falhado.
    Error {
        message: String,
    },
    /// O `fetch` não foi feito ou falhou: o "atrás" estaria velho.
    FetchFailed {
        message: String,
    },
    /// A config tem `filter.*.{clean,smudge,process}` (ADR 0034): não verificável.
    Filter,
    /// A config não pôde ser lida, ou o `status` falhou: a falha é fechada.
    Unreadable {
        message: String,
    },
    DetachedHead,
    NoUpstream,
    UpstreamGone,
    Diverged {
        ahead: u32,
        behind: u32,
    },
    /// Há alteração local (modificado ou não rastreado): o nodus nem tenta.
    DirtyTree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SyncDecision {
    /// Limpo, atrás do upstream e sem commit local: `merge --ff-only` resolve.
    Advance {
        behind: u32,
    },
    UpToDate,
    /// Só à frente do upstream: nada a puxar.
    Ahead {
        ahead: u32,
    },
    Skip {
        reason: SkipReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncItem {
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    /// O commit curto antes do avanço (o que o histórico guarda).
    pub commit: Option<String>,
    pub decision: SyncDecision,
}

/// O que um avanço fez.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Advanced {
    pub before: Option<String>,
    pub after: String,
}

/// Classifica um repositório já inspecionado (com `fetch`). Só roda `git` depois de o estado, o
/// `fetch` e a contagem pedirem; o gate vem antes de qualquer `status` ou `merge`.
pub fn plan_item(runner: &dyn CommandRunner, status: &RepoStatus) -> SyncItem {
    SyncItem {
        name: status.name.clone(),
        path: status.path.clone(),
        branch: status.branch.clone(),
        commit: status.commit.clone(),
        decision: decide(runner, status),
    }
}

fn skip(reason: SkipReason) -> SyncDecision {
    SyncDecision::Skip { reason }
}

fn decide(runner: &dyn CommandRunner, status: &RepoStatus) -> SyncDecision {
    if let Some(message) = &status.error {
        return skip(SkipReason::Error {
            message: message.clone(),
        });
    }
    match &status.fetch {
        Some(FetchResult::Done) => {}
        Some(FetchResult::Failed(message)) => {
            return skip(SkipReason::FetchFailed {
                message: message.clone(),
            });
        }
        None => {
            return skip(SkipReason::FetchFailed {
                message: "o fetch não foi feito".to_string(),
            });
        }
    }
    // O gate vem antes de qualquer `status` ou `merge`: são eles que aplicam filtro.
    match has_filter_command(runner, &status.path) {
        Ok(false) => {}
        Ok(true) => return skip(SkipReason::Filter),
        Err(message) => return skip(SkipReason::Unreadable { message }),
    }
    if status.branch.is_none() {
        return skip(SkipReason::DetachedHead);
    }
    if status.upstream_gone {
        return skip(SkipReason::UpstreamGone);
    }
    let (Some(_), Some(ahead), Some(behind)) = (&status.upstream, status.ahead, status.behind)
    else {
        return skip(SkipReason::NoUpstream);
    };
    match (ahead, behind) {
        (0, 0) => SyncDecision::UpToDate,
        (ahead, 0) => SyncDecision::Ahead { ahead },
        (0, behind) => match is_clean(runner, &status.path) {
            Ok(true) => SyncDecision::Advance { behind },
            Ok(false) => skip(SkipReason::DirtyTree),
            Err(message) => skip(SkipReason::Unreadable { message }),
        },
        (ahead, behind) => skip(SkipReason::Diverged { ahead, behind }),
    }
}

/// `true` se a config que o Git leria (inclusive `include` e `includeIf`) define comando de filtro.
/// Só os nomes das chaves são lidos, e a comparação não diferencia maiúsculas (seção e variável do
/// Git não diferenciam). Falha ao ler a config é erro: o chamador recusa o repositório.
fn has_filter_command(runner: &dyn CommandRunner, repo: &str) -> Result<bool, String> {
    let saida = run_git(
        runner,
        repo,
        &["config", "--list", "--includes", "--name-only"],
    )
    .map_err(|e| git_error(&e))?;
    Ok(saida.lines().any(|nome| {
        let nome = nome.trim().to_string();
        nome.starts_with("filter.")
            && ["clean", "smudge", "process"]
                .iter()
                .any(|var| nome.ends_with(&format!(".{var}")))
    }))
}

/// `status --porcelain` vazio. `--untracked-files=normal` impede que `status.showUntrackedFiles=no`
/// na config do repositório esconda arquivo novo.
fn is_clean(runner: &dyn CommandRunner, repo: &str) -> Result<bool, String> {
    let saida = run_git(
        runner,
        repo,
        &["status", "--porcelain", "--untracked-files=normal"],
    )
    .map_err(|e| git_error(&e))?;
    Ok(saida.trim().is_empty())
}

/// Avança um repositório classificado como `Advance`; recusa qualquer outro (defesa em
/// profundidade). `Err` é a mensagem para o usuário, sem o nome do programa auxiliar.
pub fn fast_forward(runner: &dyn CommandRunner, item: &SyncItem) -> Result<Advanced, String> {
    if !matches!(item.decision, SyncDecision::Advance { .. }) {
        return Err(format!("{} não foi planejado para avançar", item.name));
    }
    run_git(runner, &item.path, &MERGE_ARGS).map_err(|e| git_error(&e))?;
    let after = run_git(runner, &item.path, &["rev-parse", "--short", "HEAD"])
        .map_err(|e| git_error(&e))?
        .trim()
        .to_string();
    if after.is_empty() || after.chars().any(char::is_control) {
        return Err("commit inválido depois do avanço".to_string());
    }
    Ok(Advanced {
        before: item.commit.clone(),
        after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::RunnerError;
    use std::cell::RefCell;

    fn falhou(code: i32) -> RunnerError {
        RunnerError::Failed {
            program: "env".into(),
            code: Some(code),
        }
    }

    /// Runner falso: responde pelo subcomando do git e guarda o argv de cada chamada.
    struct Git {
        config: Result<String, RunnerError>,
        status: Result<String, RunnerError>,
        merge: Result<String, RunnerError>,
        rev_parse: Result<String, RunnerError>,
        chamadas: RefCell<Vec<Vec<String>>>,
    }

    impl Git {
        fn limpo() -> Self {
            Self {
                config: Ok("core.bare\nremote.origin.url\nbranch.main.remote\n".into()),
                status: Ok(String::new()),
                merge: Ok("Updating 1a2b3c4..def5678\nFast-forward\n".into()),
                rev_parse: Ok("def5678\n".into()),
                chamadas: RefCell::new(vec![]),
            }
        }
        fn com_config(saida: &str) -> Self {
            Self {
                config: Ok(saida.into()),
                ..Self::limpo()
            }
        }
        fn subcomandos(&self) -> Vec<String> {
            self.chamadas
                .borrow()
                .iter()
                .map(|a| subcomando(a))
                .collect()
        }
    }

    fn subcomando(args: &[String]) -> String {
        ["config", "status", "merge", "rev-parse"]
            .iter()
            .find(|s| args.iter().any(|a| a == *s))
            .unwrap_or_else(|| panic!("subcomando inesperado: {args:?}"))
            .to_string()
    }

    impl CommandRunner for Git {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError> {
            assert_eq!(program, "env", "o git roda sempre via `env` (isolamento)");
            let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            let sub = subcomando(&args);
            self.chamadas.borrow_mut().push(args);
            match sub.as_str() {
                "config" => self.config.clone(),
                "status" => self.status.clone(),
                "merge" => self.merge.clone(),
                _ => self.rev_parse.clone(),
            }
        }
    }

    /// `main` com upstream `origin/main`, `fetch` feito, `ahead` e `behind` dados.
    fn estado(ahead: u32, behind: u32) -> RepoStatus {
        RepoStatus {
            name: "repo".into(),
            path: "/home/u/ws/repo".into(),
            branch: Some("main".into()),
            commit: Some("1a2b3c4".into()),
            upstream: Some("origin/main".into()),
            ahead: Some(ahead),
            behind: Some(behind),
            upstream_gone: false,
            origin_url: Some("git@github.com:exemplo/repo.git".into()),
            fetch: Some(FetchResult::Done),
            error: None,
        }
    }

    fn decisao(git: &Git, status: &RepoStatus) -> SyncDecision {
        plan_item(git, status).decision
    }

    fn pula(reason: SkipReason) -> SyncDecision {
        SyncDecision::Skip { reason }
    }

    // --- plan_item: o que avança ---

    #[test]
    fn limpo_e_atras_avanca_e_so_leu_config_e_status() {
        let git = Git::limpo();
        let item = plan_item(&git, &estado(0, 2));
        assert_eq!(item.decision, SyncDecision::Advance { behind: 2 });
        assert_eq!(item.name, "repo");
        assert_eq!(item.commit.as_deref(), Some("1a2b3c4"));
        assert_eq!(
            git.subcomandos(),
            ["config", "status"],
            "planejar não avança"
        );
    }

    // --- plan_item: o gate ---

    #[test]
    fn filtro_na_config_pula_e_nao_roda_status_nem_merge() {
        for chave in [
            "filter.lfs.clean",
            "filter.lfs.smudge",
            "filter.lfs.process",
            "FILTER.x.SMUDGE",
            "Filter.evil.Clean",
            "filter.a.b.process",
        ] {
            let git = Git::com_config(&format!("core.bare\n{chave}\n"));
            assert_eq!(
                decisao(&git, &estado(0, 1)),
                pula(SkipReason::Filter),
                "{chave}"
            );
            assert_eq!(git.subcomandos(), ["config"], "{chave}: só o gate roda");
        }
    }

    #[test]
    fn filtro_sem_comando_nao_recusa() {
        // `required` não executa nada; só clean, smudge e process.
        let git = Git::com_config("filter.lfs.required\nfilter.x.clean.extra\n");
        assert_eq!(
            decisao(&git, &estado(0, 1)),
            SyncDecision::Advance { behind: 1 }
        );
    }

    #[test]
    fn config_ilegivel_pula_com_falha_fechada() {
        let git = Git {
            config: Err(falhou(128)),
            ..Git::limpo()
        };
        assert!(matches!(
            decisao(&git, &estado(0, 1)),
            SyncDecision::Skip {
                reason: SkipReason::Unreadable { .. }
            }
        ));
        assert_eq!(git.subcomandos(), ["config"]);
    }

    #[test]
    fn o_gate_le_a_config_inteira_com_includes_e_so_nomes() {
        let git = Git::limpo();
        plan_item(&git, &estado(0, 1));
        let args = git.chamadas.borrow()[0].clone();
        let n = args.len();
        assert_eq!(
            args[n - 4..],
            ["config", "--list", "--includes", "--name-only"]
        );
        assert!(
            args.contains(&"core.fsmonitor=false".to_string()),
            "isolado: {args:?}"
        );
    }

    // --- plan_item: classificação ---

    #[test]
    fn divergido_pula_sem_status() {
        let git = Git::limpo();
        assert_eq!(
            decisao(&git, &estado(1, 1)),
            pula(SkipReason::Diverged {
                ahead: 1,
                behind: 1
            })
        );
        assert_eq!(git.subcomandos(), ["config"]);
    }

    #[test]
    fn so_a_frente_e_em_dia_nao_avancam() {
        let git = Git::limpo();
        assert_eq!(
            decisao(&git, &estado(2, 0)),
            SyncDecision::Ahead { ahead: 2 }
        );
        assert_eq!(decisao(&git, &estado(0, 0)), SyncDecision::UpToDate);
        assert!(!git.subcomandos().contains(&"status".to_string()));
    }

    #[test]
    fn arvore_suja_pula_modificado_e_nao_rastreado() {
        for saida in [" M a.txt\n", "?? novo.txt\n", "A  b.txt\n"] {
            let git = Git {
                status: Ok(saida.into()),
                ..Git::limpo()
            };
            assert_eq!(
                decisao(&git, &estado(0, 1)),
                pula(SkipReason::DirtyTree),
                "{saida:?}"
            );
            assert!(!git.subcomandos().contains(&"merge".to_string()));
        }
    }

    #[test]
    fn status_falho_pula_com_falha_fechada() {
        let git = Git {
            status: Err(falhou(128)),
            ..Git::limpo()
        };
        assert!(matches!(
            decisao(&git, &estado(0, 1)),
            SyncDecision::Skip {
                reason: SkipReason::Unreadable { .. }
            }
        ));
    }

    #[test]
    fn o_status_isolado_conta_arquivo_nao_rastreado_mesmo_se_a_config_esconder() {
        let git = Git::limpo();
        plan_item(&git, &estado(0, 1));
        let args = git.chamadas.borrow()[1].clone();
        assert!(args.contains(&"--porcelain".to_string()), "{args:?}");
        assert!(
            args.contains(&"--untracked-files=normal".to_string()),
            "a config do repositório não pode esconder arquivo novo: {args:?}"
        );
        assert!(
            args.contains(&"core.hooksPath=/dev/null".to_string()),
            "isolado: {args:?}"
        );
    }

    #[test]
    fn sem_upstream_head_solto_e_upstream_sumiu_pulam() {
        let git = Git::limpo();
        let mut s = estado(0, 1);
        s.upstream = None;
        s.ahead = None;
        s.behind = None;
        assert_eq!(decisao(&git, &s), pula(SkipReason::NoUpstream));
        let mut s = estado(0, 1);
        s.branch = None;
        assert_eq!(decisao(&git, &s), pula(SkipReason::DetachedHead));
        let mut s = estado(0, 1);
        s.upstream_gone = true;
        assert_eq!(decisao(&git, &s), pula(SkipReason::UpstreamGone));
    }

    #[test]
    fn erro_de_leitura_e_fetch_que_falhou_pulam_sem_rodar_git() {
        let git = Git::limpo();
        let mut s = estado(0, 1);
        s.error = Some("git terminou com erro (código Some(128))".into());
        assert!(matches!(
            decisao(&git, &s),
            SyncDecision::Skip {
                reason: SkipReason::Error { .. }
            }
        ));
        let mut s = estado(0, 1);
        s.fetch = Some(FetchResult::Failed("sem rede".into()));
        assert!(matches!(
            decisao(&git, &s),
            SyncDecision::Skip {
                reason: SkipReason::FetchFailed { .. }
            }
        ));
        let mut s = estado(0, 1);
        s.fetch = None;
        assert!(matches!(
            decisao(&git, &s),
            SyncDecision::Skip {
                reason: SkipReason::FetchFailed { .. }
            }
        ));
        assert!(git.chamadas.borrow().is_empty(), "nenhum git foi chamado");
    }

    // --- fast_forward ---

    fn avanca() -> SyncItem {
        plan_item(&Git::limpo(), &estado(0, 2))
    }

    #[test]
    fn o_merge_usa_exatamente_a_receita_do_adr_0034_isolada() {
        let git = Git::limpo();
        let feito = fast_forward(&git, &avanca()).unwrap();
        assert_eq!(feito.before.as_deref(), Some("1a2b3c4"));
        assert_eq!(feito.after, "def5678");
        let args = git.chamadas.borrow()[0].clone();
        let c = args.iter().position(|a| a == "-C").unwrap();
        assert_eq!(args[c + 1], "/home/u/ws/repo");
        assert_eq!(args[c + 2..], MERGE_ARGS);
        assert!(MERGE_ARGS.contains(&"--ff-only"));
        assert!(MERGE_ARGS.contains(&"--no-verify-signatures"));
        for par in [
            "core.fsmonitor=false",
            "core.hooksPath=/dev/null",
            "credential.helper=",
        ] {
            assert!(args.contains(&par.to_string()), "falta {par}: {args:?}");
        }
        assert_eq!(git.subcomandos(), ["merge", "rev-parse"]);
    }

    #[test]
    fn fast_forward_recusa_o_que_nao_foi_planejado_como_avanco() {
        let git = Git::limpo();
        for status in [estado(1, 1), estado(2, 0), estado(0, 0)] {
            let item = plan_item(&git, &status);
            assert!(fast_forward(&git, &item).is_err(), "{:?}", item.decision);
        }
        assert!(
            !git.subcomandos().contains(&"merge".to_string()),
            "nenhum merge foi chamado"
        );
    }

    #[test]
    fn merge_que_falha_vira_erro_sem_o_nome_do_programa_auxiliar() {
        let git = Git {
            merge: Err(falhou(128)),
            ..Git::limpo()
        };
        let erro = fast_forward(&git, &avanca()).unwrap_err();
        assert!(erro.contains("git"), "{erro}");
        assert!(!erro.contains("`env`"), "{erro}");
        assert!(!git.subcomandos().contains(&"rev-parse".to_string()));
    }

    // --- JSON ---

    #[test]
    fn o_json_da_decisao_tem_formato_estavel() {
        let avanco = serde_json::to_value(SyncDecision::Advance { behind: 2 }).unwrap();
        assert_eq!(avanco, serde_json::json!({"kind": "advance", "behind": 2}));
        let pulo = serde_json::to_value(pula(SkipReason::Diverged {
            ahead: 1,
            behind: 3,
        }))
        .unwrap();
        assert_eq!(
            pulo,
            serde_json::json!({"kind": "skip", "reason": {"why": "diverged", "ahead": 1, "behind": 3}})
        );
        let filtro = serde_json::to_value(pula(SkipReason::Filter)).unwrap();
        assert_eq!(
            filtro,
            serde_json::json!({"kind": "skip", "reason": {"why": "filter"}})
        );
    }
}
