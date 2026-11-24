//! `nodusctl repos sync` (ADR 0034): fetch, plano por repositório, uma confirmação e `merge
//! --ff-only` só nos que só estão atrás do upstream. A decisão de cada repositório (o gate de filtro,
//! a árvore suja, o divergido) está em `nodus_core::sync`; aqui ficam o plano na tela, a pergunta, a
//! execução em ordem e o histórico (uma linha v5 por repositório que avançou).

use crate::discovery::RepoDir;
use anyhow::{Context, Result, bail};
use nodus_core::history::{HistoryEntry, entry_from_sync};
use nodus_core::repos::inspect_repo;
use nodus_core::runner::CommandRunner;
use nodus_core::sync::{MERGE_ARGS, SkipReason, SyncDecision, SyncItem, fast_forward, plan_item};
use std::fmt::Write as _;
use std::io::{BufRead, Write};
use std::path::Path;

/// Tudo o que o `sync` precisa saber, já resolvido pelo `main`.
pub struct Request<'a> {
    /// Os repositórios já filtrados por `--only` (filhos diretos, caminho absoluto).
    pub repos: &'a [RepoDir],
    /// A entrada é um terminal: sem ele não há confirmação possível.
    pub interactive: bool,
    /// Só mostra o plano em JSON; não pergunta nem executa.
    pub json: bool,
    pub history_path: &'a Path,
    pub now_unix: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nenhum repositório estava em condição de avançar: nem pergunta.
    NothingToDo,
    /// `--json`: só o plano.
    PlanOnly,
    /// O plano foi mostrado e a resposta não foi `s`.
    Declined,
    /// Quantos repositórios avançaram.
    Advanced(usize),
}

/// Os repositórios pedidos com `--only`: cada nome tem de ser idêntico ao de um filho direto listado
/// na hora (padrão do ADR 0028); nome desconhecido é erro **antes** de qualquer comando.
pub fn select_repos(all: Vec<RepoDir>, only: &[String]) -> Result<Vec<RepoDir>> {
    if only.is_empty() {
        return Ok(all);
    }
    for name in only {
        if !all.iter().any(|repo| &repo.name == name) {
            bail!("repositório {name:?} não encontrado entre os filhos diretos da pasta");
        }
    }
    Ok(all
        .into_iter()
        .filter(|repo| only.contains(&repo.name))
        .collect())
}

/// O `git` que o `merge` roda, como o histórico o guarda (sem as variáveis e as opções de
/// isolamento, que são do nodus e não do comando).
fn merge_command(path: &str) -> Vec<String> {
    let mut command = vec!["git".to_string(), "-C".to_string(), path.to_string()];
    command.extend(MERGE_ARGS.iter().map(|arg| arg.to_string()));
    command
}

fn describe(decision: &SyncDecision) -> String {
    match decision {
        SyncDecision::Advance { behind } => {
            let plural = if *behind == 1 { "" } else { "s" };
            format!("avança {behind} commit{plural}")
        }
        SyncDecision::UpToDate => "em dia".to_string(),
        SyncDecision::Ahead { ahead } => format!("à frente do upstream em {ahead} (nada a puxar)"),
        SyncDecision::Skip { reason } => format!("fica de fora: {}", describe_skip(reason)),
    }
}

fn describe_skip(reason: &SkipReason) -> String {
    match reason {
        SkipReason::Error { message } => format!("erro ({message})"),
        SkipReason::FetchFailed { message } => format!("o fetch falhou ({message})"),
        SkipReason::Filter => {
            "a config tem filtro (`filter.*`): não verificável, o nodus não olha esse repositório"
                .to_string()
        }
        SkipReason::Unreadable { message } => format!("não consegui ler o estado ({message})"),
        SkipReason::DetachedHead => "HEAD solto".to_string(),
        SkipReason::NoUpstream => "a branch não tem upstream".to_string(),
        SkipReason::UpstreamGone => "o upstream sumiu".to_string(),
        SkipReason::Diverged { ahead, behind } => {
            format!("divergiu ({ahead} à frente, {behind} atrás): resolva à mão")
        }
        SkipReason::DirtyTree => "alterações locais na árvore".to_string(),
    }
}

fn render_plan(items: &[SyncItem]) -> String {
    let mut out = format!("Repositórios ({}):\n", items.len());
    for item in items {
        let branch = item.branch.as_deref().unwrap_or("-");
        let _ = writeln!(
            out,
            "  {}  {branch}  {}",
            item.name,
            describe(&item.decision)
        );
    }
    out
}

/// Plano, confirmação, avanço e histórico. Cada repositório é lido e classificado (com `fetch`); só
/// os classificados como `avança` recebem o `merge --ff-only`, depois de **um** `[s/N]` para o plano
/// todo. A falha de um avanço interrompe os seguintes e diz o que já avançou. O histórico ganha uma
/// linha v5 por repositório que avançou, gravada logo depois do avanço.
pub fn run(
    req: &Request<'_>,
    runner: &dyn CommandRunner,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Outcome> {
    let mut items = Vec::with_capacity(req.repos.len());
    for repo in req.repos {
        let path = repo
            .path
            .to_str()
            .with_context(|| format!("caminho fora de UTF-8: {}", repo.path.display()))?;
        let status = inspect_repo(runner, &repo.name, path, true);
        items.push(plan_item(runner, &status));
    }
    if req.json {
        writeln!(output, "{}", serde_json::to_string_pretty(&items)?)?;
        return Ok(Outcome::PlanOnly);
    }
    write!(output, "{}", render_plan(&items))?;
    let to_advance: Vec<&SyncItem> = items
        .iter()
        .filter(|item| matches!(item.decision, SyncDecision::Advance { .. }))
        .collect();
    if to_advance.is_empty() {
        writeln!(output, "Nada a avançar.")?;
        return Ok(Outcome::NothingToDo);
    }
    if !req.interactive {
        bail!(
            "`repos sync` precisa de terminal interativo para confirmar (não há modo automático)"
        );
    }
    if !crate::apply::confirm(input, output)? {
        writeln!(output, "Nada foi executado.")?;
        return Ok(Outcome::Declined);
    }
    let mut advanced: Vec<&str> = Vec::new();
    for item in to_advance {
        let done = match fast_forward(runner, item) {
            Ok(done) => done,
            Err(erro) => {
                let already = if advanced.is_empty() {
                    "Nenhum repositório avançou antes.".to_string()
                } else {
                    format!("Já avançaram: {}.", advanced.join(", "))
                };
                bail!("o avanço de {} falhou: {erro}. {already}", item.name);
            }
        };
        let before = done.before.as_deref().with_context(|| {
            format!(
                "{} avançou para {}, mas não sei o commit de antes para o histórico",
                item.name, done.after
            )
        })?;
        writeln!(output, "  {}: {before} -> {}", item.name, done.after)?;
        let entry = entry_from_sync(
            &item.path,
            before,
            &done.after,
            &merge_command(&item.path),
            req.now_unix,
        );
        crate::history::append(req.history_path, &entry).with_context(|| {
            format!("{} avançou, mas não consegui gravar o histórico", item.name)
        })?;
        advanced.push(&item.name);
    }
    writeln!(output, "Avançaram {} repositório(s).", advanced.len())?;
    Ok(Outcome::Advanced(advanced.len()))
}

/// Cita para o shell só se precisar (o comando é para o dono colar; o nodus não o executa).
fn shell_quote(text: &str) -> String {
    let plain = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/_.,:+@%-".contains(c));
    if plain {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

/// A recusa do `history undo` numa linha de `repos sync`: o avanço não tem desfazer seguro (voltaria
/// com `reset --hard`, que apaga trabalho local). Diz o que fazer à mão; nada é executado.
pub fn undo_refusal(entry: &HistoryEntry, n: u32) -> String {
    let dir = shell_quote(entry.dir.as_deref().unwrap_or("?"));
    let before = entry.requested.first().map_or("?", String::as_str);
    format!(
        "a entrada [{n}] é de `repos sync` e não tem desfazer: voltar exigiria `git reset --hard`, que apaga trabalho local. O repositório estava em {before}. Se quiser voltar mesmo assim, rode por sua conta (confira antes se não há nada a perder):\n  git -C {dir} reset --hard {before}\n(nada foi executado pelo nodus.)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::history::{Action, Manager};
    use nodus_core::runner::RunnerError;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::path::PathBuf;

    const SUBCOMANDOS: [&str; 6] = [
        "config",
        "fetch",
        "for-each-ref",
        "rev-parse",
        "status",
        "merge",
    ];

    /// O que o `git` falso responde para um repositório.
    struct Cfg {
        for_each_ref: String,
        config_list: String,
        status: String,
        fetch: Result<String, RunnerError>,
        merge: Result<String, RunnerError>,
        depois: String,
    }

    fn falhou() -> RunnerError {
        RunnerError::Failed {
            program: "env".into(),
            code: Some(128),
        }
    }

    impl Cfg {
        /// Na `main`, limpo, 2 atrás do upstream: avança.
        fn avanca() -> Self {
            Self {
                for_each_ref: "*\tmain\t1a2b3c4\torigin/main\tbehind 2\n".into(),
                config_list: "core.bare\nremote.origin.url\n".into(),
                status: String::new(),
                fetch: Ok(String::new()),
                merge: Ok(String::new()),
                depois: "def5678\n".into(),
            }
        }
        fn suja(mut self) -> Self {
            self.status = " M a.txt\n".into();
            self
        }
        fn com_filtro(mut self) -> Self {
            self.config_list.push_str("filter.lfs.clean\n");
            self
        }
        fn divergida(mut self) -> Self {
            self.for_each_ref = "*\tmain\t1a2b3c4\torigin/main\tahead 1, behind 2\n".into();
            self
        }
        fn merge_falha(mut self) -> Self {
            self.merge = Err(falhou());
            self
        }
        fn fetch_falha(mut self) -> Self {
            self.fetch = Err(falhou());
            self
        }
    }

    /// Runner falso por repositório (a chave é o caminho depois do `-C`); guarda `(caminho, subcomando)`.
    struct Falso {
        repos: HashMap<String, Cfg>,
        chamadas: RefCell<Vec<(String, String)>>,
    }

    impl Falso {
        fn novo(repos: Vec<(&str, Cfg)>) -> Self {
            Self {
                repos: repos
                    .into_iter()
                    .map(|(nome, cfg)| (format!("/ws/{nome}"), cfg))
                    .collect(),
                chamadas: RefCell::new(vec![]),
            }
        }
        /// Os caminhos onde o `merge` rodou, na ordem.
        fn merges(&self) -> Vec<String> {
            self.chamadas
                .borrow()
                .iter()
                .filter(|(_, sub)| sub == "merge")
                .map(|(caminho, _)| caminho.clone())
                .collect()
        }
        fn chamou(&self, sub: &str) -> usize {
            self.chamadas
                .borrow()
                .iter()
                .filter(|(_, s)| s == sub)
                .count()
        }
    }

    impl CommandRunner for Falso {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError> {
            assert_eq!(program, "env", "o git roda sempre via `env` (isolamento)");
            let c = args.iter().position(|a| *a == "-C").expect("falta -C");
            let caminho = args[c + 1].to_string();
            let resto = &args[c + 2..];
            let sub = resto
                .iter()
                .find(|a| SUBCOMANDOS.contains(*a))
                .unwrap_or_else(|| panic!("subcomando inesperado: {args:?}"));
            self.chamadas
                .borrow_mut()
                .push((caminho.clone(), sub.to_string()));
            let cfg = self.repos.get(&caminho).expect("repositório desconhecido");
            match *sub {
                "config" if resto.contains(&"--list") => Ok(cfg.config_list.clone()),
                "config" => Ok("git@github.com:exemplo/repo.git\n".into()),
                "fetch" => cfg.fetch.clone(),
                "for-each-ref" => Ok(cfg.for_each_ref.clone()),
                "rev-parse" => Ok(cfg.depois.clone()),
                "status" => Ok(cfg.status.clone()),
                _ => cfg.merge.clone(),
            }
        }
    }

    fn dirs(nomes: &[&str]) -> Vec<RepoDir> {
        nomes
            .iter()
            .map(|nome| RepoDir {
                name: nome.to_string(),
                path: PathBuf::from(format!("/ws/{nome}")),
            })
            .collect()
    }

    struct Rodada {
        resultado: Result<Outcome>,
        saida: String,
    }

    fn rodar(
        falso: &Falso,
        nomes: &[&str],
        interactive: bool,
        json: bool,
        entrada: &str,
        historico: &Path,
    ) -> Rodada {
        let repos = dirs(nomes);
        let req = Request {
            repos: &repos,
            interactive,
            json,
            history_path: historico,
            now_unix: 1_700_000_000,
        };
        let mut input = Cursor::new(entrada.as_bytes().to_vec());
        let mut output = Cursor::new(Vec::new());
        let resultado = run(&req, falso, &mut input, &mut output);
        Rodada {
            resultado,
            saida: String::from_utf8(output.into_inner()).unwrap(),
        }
    }

    // --- o plano ---

    #[test]
    fn o_plano_diz_quem_avanca_e_por_que_os_outros_ficam() {
        let falso = Falso::novo(vec![
            ("alfa", Cfg::avanca()),
            ("beta", Cfg::avanca().suja()),
            ("gama", Cfg::avanca().com_filtro()),
            ("delta", Cfg::avanca().divergida()),
            ("epsilon", Cfg::avanca().fetch_falha()),
        ]);
        let tmp = TmpDir::novo();
        let h = tmp.path().join("history.jsonl");
        let r = rodar(
            &falso,
            &["alfa", "beta", "gama", "delta", "epsilon"],
            true,
            false,
            "n\n",
            &h,
        );
        assert_eq!(r.resultado.unwrap(), Outcome::Declined);
        for esperado in [
            "alfa",
            "avança 2 commits",
            "alterações locais",
            "filtro",
            "divergiu",
            "fetch",
        ] {
            assert!(
                r.saida.contains(esperado),
                "falta {esperado:?}:\n{}",
                r.saida
            );
        }
        assert_eq!(falso.chamou("fetch"), 5, "o sync sempre faz fetch");
    }

    #[test]
    fn so_planeja_sem_terminal_e_sem_avancar() {
        let falso = Falso::novo(vec![("alfa", Cfg::avanca())]);
        let tmp = TmpDir::novo();
        let h = tmp.path().join("history.jsonl");
        let r = rodar(&falso, &["alfa"], false, false, "", &h);
        let erro = r.resultado.unwrap_err().to_string();
        assert!(erro.contains("terminal interativo"), "{erro}");
        assert!(
            r.saida.contains("avança 2 commits"),
            "o plano aparece antes do erro"
        );
        assert!(falso.merges().is_empty());
        assert!(!h.exists());
    }

    // --- a confirmação ---

    #[test]
    fn resposta_diferente_de_s_nao_executa_nada() {
        for entrada in ["n\n", "\n", "", "talvez\n"] {
            let falso = Falso::novo(vec![("alfa", Cfg::avanca())]);
            let tmp = TmpDir::novo();
            let h = tmp.path().join("history.jsonl");
            let r = rodar(&falso, &["alfa"], true, false, entrada, &h);
            assert_eq!(r.resultado.unwrap(), Outcome::Declined, "{entrada:?}");
            assert!(r.saida.contains("Nada foi executado."), "{}", r.saida);
            assert!(falso.merges().is_empty(), "{entrada:?}");
            assert!(!h.exists(), "{entrada:?}");
        }
    }

    #[test]
    fn nada_a_avancar_nao_pergunta() {
        let falso = Falso::novo(vec![
            ("alfa", Cfg::avanca().suja()),
            ("beta", Cfg::avanca().com_filtro()),
        ]);
        let tmp = TmpDir::novo();
        let h = tmp.path().join("history.jsonl");
        let r = rodar(&falso, &["alfa", "beta"], true, false, "s\n", &h);
        assert_eq!(r.resultado.unwrap(), Outcome::NothingToDo);
        assert!(!r.saida.contains("Executar?"), "{}", r.saida);
        assert!(falso.merges().is_empty());
    }

    #[test]
    fn confirmando_so_os_que_avancam_recebem_o_merge_e_o_historico() {
        let falso = Falso::novo(vec![
            ("alfa", Cfg::avanca()),
            ("beta", Cfg::avanca().suja()),
            ("gama", Cfg::avanca().com_filtro()),
            ("delta", Cfg::avanca()),
        ]);
        let tmp = TmpDir::novo();
        let h = tmp.path().join("history.jsonl");
        let r = rodar(
            &falso,
            &["alfa", "beta", "gama", "delta"],
            true,
            false,
            "s\n",
            &h,
        );
        assert_eq!(r.resultado.unwrap(), Outcome::Advanced(2));
        assert_eq!(falso.merges(), ["/ws/alfa", "/ws/delta"]);
        assert!(r.saida.contains("1a2b3c4 -> def5678"), "{}", r.saida);
        let entradas = crate::history::read(&h).unwrap();
        assert_eq!(entradas.len(), 2);
        for (entrada, dir) in entradas.iter().zip(["/ws/alfa", "/ws/delta"]) {
            assert_eq!(entrada.version, 5);
            assert_eq!(entrada.action, Action::Sync);
            assert_eq!(entrada.manager, Manager::Repos);
            assert_eq!(entrada.dir.as_deref(), Some(dir));
            assert_eq!(entrada.requested, ["1a2b3c4"]);
            assert_eq!(entrada.installed, ["def5678"]);
            assert_eq!(entrada.at_unix, 1_700_000_000);
        }
    }

    // --- falhas ---

    #[test]
    fn merge_que_falha_para_no_primeiro_erro_e_diz_o_que_ja_avancou() {
        let falso = Falso::novo(vec![
            ("alfa", Cfg::avanca()),
            ("beta", Cfg::avanca().merge_falha()),
            ("gama", Cfg::avanca()),
        ]);
        let tmp = TmpDir::novo();
        let h = tmp.path().join("history.jsonl");
        let r = rodar(&falso, &["alfa", "beta", "gama"], true, false, "s\n", &h);
        let erro = r.resultado.unwrap_err().to_string();
        assert!(erro.contains("beta"), "{erro}");
        assert!(erro.contains("alfa"), "diz o que já avançou: {erro}");
        assert_eq!(
            falso.merges(),
            ["/ws/alfa", "/ws/beta"],
            "gama não foi tentado"
        );
        let entradas = crate::history::read(&h).unwrap();
        assert_eq!(entradas.len(), 1, "só o alfa avançou e foi registrado");
        assert_eq!(entradas[0].dir.as_deref(), Some("/ws/alfa"));
    }

    #[test]
    fn primeiro_merge_que_falha_nao_deixa_historico() {
        let falso = Falso::novo(vec![("alfa", Cfg::avanca().merge_falha())]);
        let tmp = TmpDir::novo();
        let h = tmp.path().join("history.jsonl");
        let r = rodar(&falso, &["alfa"], true, false, "s\n", &h);
        assert!(r.resultado.is_err());
        assert!(!h.exists());
    }

    #[test]
    fn historico_que_nao_grava_depois_de_avancar_avisa_que_avancou() {
        let falso = Falso::novo(vec![("alfa", Cfg::avanca())]);
        let tmp = TmpDir::novo();
        // O pai do histórico é um arquivo comum: criar a pasta falha.
        let arquivo = tmp.path().join("arquivo");
        std::fs::write(&arquivo, "x").unwrap();
        let h = arquivo.join("history.jsonl");
        let r = rodar(&falso, &["alfa"], true, false, "s\n", &h);
        let erro = format!("{:#}", r.resultado.unwrap_err());
        assert!(erro.contains("avançou"), "{erro}");
        assert!(erro.contains("histórico"), "{erro}");
        assert_eq!(falso.merges(), ["/ws/alfa"]);
    }

    // --- --json ---

    #[test]
    fn json_so_mostra_o_plano_e_nao_pergunta_nem_avanca() {
        let falso = Falso::novo(vec![
            ("alfa", Cfg::avanca()),
            ("beta", Cfg::avanca().suja()),
        ]);
        let tmp = TmpDir::novo();
        let h = tmp.path().join("history.jsonl");
        let r = rodar(&falso, &["alfa", "beta"], false, true, "", &h);
        assert_eq!(r.resultado.unwrap(), Outcome::PlanOnly);
        let json: serde_json::Value = serde_json::from_str(&r.saida).unwrap();
        assert_eq!(json[0]["name"], "alfa");
        assert_eq!(
            json[0]["decision"],
            serde_json::json!({"kind": "advance", "behind": 2})
        );
        assert_eq!(
            json[1]["decision"],
            serde_json::json!({"kind": "skip", "reason": {"why": "dirty_tree"}})
        );
        assert!(falso.merges().is_empty());
        assert!(!h.exists());
    }

    // --- --only ---

    #[test]
    fn only_vazio_pega_todos_e_only_com_nome_pega_so_ele() {
        let todos = dirs(&["alfa", "beta", "gama"]);
        assert_eq!(select_repos(todos.clone(), &[]).unwrap(), todos);
        let so_beta = select_repos(todos.clone(), &["beta".to_string()]).unwrap();
        assert_eq!(so_beta, dirs(&["beta"]));
        let repetido = select_repos(todos, &["beta".to_string(), "beta".to_string()]).unwrap();
        assert_eq!(repetido, dirs(&["beta"]));
    }

    #[test]
    fn only_com_nome_que_nao_e_filho_direto_e_erro() {
        let todos = dirs(&["alfa", "beta"]);
        for ruim in ["gama", "../alfa", "alfa/x", "-C", "", "ALFA"] {
            let erro = select_repos(todos.clone(), &[ruim.to_string()])
                .unwrap_err()
                .to_string();
            assert!(
                erro.contains("não encontrado") || erro.contains("não é"),
                "{ruim}: {erro}"
            );
        }
    }

    // --- sem desfazer ---

    #[test]
    fn o_undo_de_sync_e_recusado_com_o_commit_de_antes_e_sem_executar_nada() {
        let entrada = nodus_core::history::entry_from_sync(
            "/home/u/ws/repo",
            "1a2b3c4",
            "def5678",
            &["git".to_string()],
            1_700_000_000,
        );
        let texto = undo_refusal(&entrada, 3);
        for esperado in [
            "[3]",
            "repos sync",
            "não tem desfazer",
            "git -C /home/u/ws/repo reset --hard 1a2b3c4",
            "nada foi executado",
        ] {
            assert!(texto.contains(esperado), "falta {esperado:?}: {texto}");
        }
    }

    #[test]
    fn o_comando_de_voltar_cita_o_caminho_so_quando_precisa() {
        let com_espaco = nodus_core::history::entry_from_sync(
            "/home/u/meu ws/repo",
            "1a2b3c4",
            "def5678",
            &["git".to_string()],
            1,
        );
        assert!(
            undo_refusal(&com_espaco, 1)
                .contains("git -C '/home/u/meu ws/repo' reset --hard 1a2b3c4"),
        );
        let com_aspas = nodus_core::history::entry_from_sync(
            "/home/u/it's",
            "1a2b3c4",
            "def5678",
            &["git".to_string()],
            1,
        );
        assert!(
            undo_refusal(&com_aspas, 1).contains(r"git -C '/home/u/it'\''s' reset --hard 1a2b3c4")
        );
    }
}

/// O `sync` contra `git` de verdade, em repositórios temporários: gate, árvore suja, divergência e
/// isolamento de ponta a ponta (a parte que o runner falso não prova).
#[cfg(test)]
mod git_real {
    use super::*;
    use crate::discovery::find_repos;
    use crate::process::RealRunner;
    use crate::testutil::TmpDir;
    use std::io::Cursor;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) -> String {
        let saida = Command::new("git")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(["-c", "init.defaultBranch=main", "-C"])
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            saida.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&saida.stderr)
        );
        String::from_utf8_lossy(&saida.stdout).trim().to_string()
    }

    struct Cenario {
        tmp: TmpDir,
        ws: PathBuf,
        work: PathBuf,
        historico: PathBuf,
    }

    /// `ws/work` clonado de um remoto local; o remoto ganha um segundo commit (muda `a.txt`, cria
    /// `b.txt`), então `work` fica 1 commit atrás. O primeiro commit tem um `.gitattributes` que manda
    /// todo arquivo para o filtro `evil`, inofensivo enquanto a config não o define.
    fn cenario() -> Cenario {
        let tmp = TmpDir::novo();
        let raiz = tmp.path().to_path_buf();
        let ws = raiz.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        git(&raiz, &["init", "-q", "pusher"]);
        let pusher = raiz.join("pusher");
        std::fs::write(pusher.join("a.txt"), "um\n").unwrap();
        std::fs::write(pusher.join(".gitattributes"), "* filter=evil\n").unwrap();
        git(&pusher, &["add", "-A"]);
        git(&pusher, &["commit", "-q", "-m", "um"]);
        git(&raiz, &["clone", "-q", "--bare", "pusher", "remoto.git"]);
        git(
            &pusher,
            &[
                "remote",
                "add",
                "origin",
                raiz.join("remoto.git").to_str().unwrap(),
            ],
        );
        git(&raiz, &["clone", "-q", "remoto.git", "ws/work"]);
        std::fs::write(pusher.join("a.txt"), "dois\n").unwrap();
        std::fs::write(pusher.join("b.txt"), "novo\n").unwrap();
        git(&pusher, &["add", "-A"]);
        git(&pusher, &["commit", "-q", "-m", "dois"]);
        git(&pusher, &["push", "-q", "origin", "main"]);
        let work = ws.join("work");
        let historico = raiz.join("history.jsonl");
        Cenario {
            tmp,
            ws,
            work,
            historico,
        }
    }

    fn sincronizar(c: &Cenario, entrada: &str) -> Result<Outcome> {
        let repos = find_repos(&c.ws).unwrap();
        let req = Request {
            repos: &repos,
            interactive: true,
            json: false,
            history_path: &c.historico,
            now_unix: 1_700_000_000,
        };
        let mut input = Cursor::new(entrada.as_bytes().to_vec());
        let mut output = Cursor::new(Vec::new());
        run(&req, &RealRunner, &mut input, &mut output)
    }

    fn head(c: &Cenario) -> String {
        git(&c.work, &["rev-parse", "--short", "HEAD"])
    }

    #[test]
    fn avanca_de_verdade_e_registra_os_dois_commits() {
        let c = cenario();
        let antes = head(&c);
        assert_eq!(sincronizar(&c, "s\n").unwrap(), Outcome::Advanced(1));
        let depois = head(&c);
        assert_ne!(antes, depois);
        assert_eq!(
            depois,
            git(&c.work, &["rev-parse", "--short", "origin/main"])
        );
        assert_eq!(
            std::fs::read_to_string(c.work.join("a.txt")).unwrap(),
            "dois\n"
        );
        assert_eq!(
            std::fs::read_to_string(c.work.join("b.txt")).unwrap(),
            "novo\n"
        );
        let entradas = crate::history::read(&c.historico).unwrap();
        assert_eq!(entradas.len(), 1);
        assert_eq!(entradas[0].requested, [antes]);
        assert_eq!(entradas[0].installed, [depois]);
        assert_eq!(entradas[0].dir.as_deref(), c.work.to_str());
        assert_eq!(
            git(&c.work, &["rev-list", "--merges", "--count", "HEAD"]),
            "0"
        );
    }

    #[test]
    fn recusando_com_n_o_repositorio_nao_muda() {
        let c = cenario();
        let antes = head(&c);
        assert_eq!(sincronizar(&c, "n\n").unwrap(), Outcome::Declined);
        assert_eq!(head(&c), antes);
        assert!(!c.historico.exists());
    }

    #[test]
    fn filtro_na_config_do_repositorio_nao_executa_e_nao_avanca() {
        let c = cenario();
        let canario = c.tmp.path().join("canario-filtro");
        let script = c.tmp.path().join("filtro.sh");
        std::fs::write(
            &script,
            format!("#!/bin/sh\n: > '{}'\ncat\n", canario.display()),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        git(
            &c.work,
            &["config", "filter.evil.clean", script.to_str().unwrap()],
        );
        git(
            &c.work,
            &["config", "filter.evil.smudge", script.to_str().unwrap()],
        );
        let antes = head(&c);
        assert_eq!(sincronizar(&c, "s\n").unwrap(), Outcome::NothingToDo);
        assert!(!canario.exists(), "o filtro do repositório foi executado");
        assert_eq!(head(&c), antes);
        assert_eq!(
            std::fs::read_to_string(c.work.join("a.txt")).unwrap(),
            "um\n"
        );
    }

    #[test]
    fn hook_do_repositorio_nao_roda_no_merge() {
        let c = cenario();
        let canario = c.tmp.path().join("canario-hook");
        let hook = c.work.join(".git/hooks/post-merge");
        std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
        std::fs::write(&hook, format!("#!/bin/sh\n: > '{}'\n", canario.display())).unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(sincronizar(&c, "s\n").unwrap(), Outcome::Advanced(1));
        assert!(!canario.exists(), "o hook post-merge foi executado");
    }

    #[test]
    fn arvore_com_alteracao_local_fica_de_fora_e_intacta() {
        let c = cenario();
        std::fs::write(c.work.join("a.txt"), "mexi\n").unwrap();
        let antes = head(&c);
        assert_eq!(sincronizar(&c, "s\n").unwrap(), Outcome::NothingToDo);
        assert_eq!(head(&c), antes);
        assert_eq!(
            std::fs::read_to_string(c.work.join("a.txt")).unwrap(),
            "mexi\n"
        );
    }

    #[test]
    fn arquivo_novo_conta_mesmo_se_a_config_do_repositorio_esconde() {
        let c = cenario();
        git(&c.work, &["config", "status.showUntrackedFiles", "no"]);
        // O remoto cria `b.txt`: um `b.txt` local não rastreado seria sobrescrito.
        std::fs::write(c.work.join("b.txt"), "meu\n").unwrap();
        let antes = head(&c);
        assert_eq!(sincronizar(&c, "s\n").unwrap(), Outcome::NothingToDo);
        assert_eq!(head(&c), antes);
        assert_eq!(
            std::fs::read_to_string(c.work.join("b.txt")).unwrap(),
            "meu\n"
        );
    }

    #[test]
    fn historico_divergido_fica_de_fora() {
        let c = cenario();
        std::fs::write(c.work.join("local.txt"), "x\n").unwrap();
        git(&c.work, &["add", "local.txt"]);
        git(&c.work, &["commit", "-q", "-m", "local"]);
        let antes = head(&c);
        assert_eq!(sincronizar(&c, "s\n").unwrap(), Outcome::NothingToDo);
        assert_eq!(head(&c), antes);
    }
}
