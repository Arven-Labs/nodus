//! `nodusctl preset apply`: a primeira operação privilegiada (SECURITY.md §4). Mostra o comando exato,
//! pede confirmação (padrão N) e só então executa; o `[Y/n]` do pacman vem depois, com as dependências.

use anyhow::{Context, Result, bail};
use nodus_core::history::{ExecResult, entry_from};
use nodus_core::preset::{Plan, pacman_install_command};
use nodus_core::runner::RunnerError;
use std::io::{self, BufRead, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    NothingToDo,
    /// Só faltam apps Flatpak, que esta versão ainda não instala.
    FlatpakOnly,
    Declined,
    Executed,
}

/// Pergunta `Executar? [s/N]`. Só `s` ou `sim` (sem diferenciar maiúscula) confirmam; vazio, fim da
/// entrada e qualquer outra coisa são "não".
pub fn confirm(input: &mut impl BufRead, output: &mut impl Write) -> io::Result<bool> {
    write!(output, "Executar? [s/N] ")?;
    output.flush()?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    Ok(answer == "s" || answer == "sim")
}

/// Executa o plano. `interactive`: a entrada é um terminal (sem ele não há confirmação possível, e
/// não existe "sim para tudo"). `exec` recebe o comando exato; no programa real ele herda o terminal.
pub fn apply(
    plan: &Plan,
    interactive: bool,
    input: &mut impl BufRead,
    output: &mut impl Write,
    mut exec: impl FnMut(&[String]) -> Result<()>,
) -> Result<Outcome> {
    if !plan.flatpak_missing.is_empty() {
        writeln!(
            output,
            "aviso: Flatpak ainda não é instalado pelo nodus: {} app(s) do preset ficam de fora",
            plan.flatpak_missing.len()
        )?;
    }
    let Some(command) = pacman_install_command(plan) else {
        return Ok(if plan.flatpak_missing.is_empty() {
            Outcome::NothingToDo
        } else {
            Outcome::FlatpakOnly
        });
    };
    writeln!(output, "Comando: {}", command.join(" "))?;
    if !interactive {
        bail!(
            "`preset apply` precisa de terminal interativo para confirmar (não há modo automático)"
        );
    }
    if !confirm(input, output)? {
        writeln!(output, "Nada foi executado.")?;
        return Ok(Outcome::Declined);
    }
    exec(&command)?;
    Ok(Outcome::Executed)
}

#[derive(Debug)]
pub struct Applied {
    pub outcome: Outcome,
    /// O plano recalculado depois de executar; `None` se nada foi executado.
    pub after: Option<Plan>,
}

/// `apply` mais o registro no histórico. Grava quando o comando chegou a rodar, mesmo que o
/// `sudo`/`pacman` tenha falhado (é quando o registro mais importa); não grava em recusa, plano
/// vazio, só Flatpak nem sem terminal, porque nada mudou. `now_unix` é o momento em que o apply
/// começou (antes da confirmação). Se não der para gravar, o erro diz o que já aconteceu.
#[allow(clippy::too_many_arguments)]
pub fn apply_with_history(
    plan: &Plan,
    interactive: bool,
    input: &mut impl BufRead,
    output: &mut impl Write,
    mut exec: impl FnMut(&[String]) -> Result<()>,
    after: impl FnOnce() -> Result<Plan>,
    history_path: &Path,
    now_unix: u64,
) -> Result<Applied> {
    let mut ran: Option<(Vec<String>, ExecResult)> = None;
    let outcome = apply(plan, interactive, input, output, |command| {
        let run = exec(command);
        let result = match &run {
            Ok(()) => ExecResult::Ok,
            Err(e) => ExecResult::Failed {
                code: match e.downcast_ref::<RunnerError>() {
                    Some(RunnerError::Failed { code, .. }) => *code,
                    _ => None,
                },
            },
        };
        ran = Some((command.to_vec(), result));
        run
    });
    let Some((command, result)) = ran else {
        return outcome.map(|outcome| Applied {
            outcome,
            after: None,
        });
    };
    let failed = outcome.is_err();
    let what_happened = if failed {
        "o `sudo`/`pacman` falhou e, além disso, não consegui"
    } else {
        "a instalação foi feita, mas não consegui"
    };
    let after_plan =
        after().with_context(|| format!("{what_happened} conferir o estado depois"))?;
    let entry = entry_from(plan, &after_plan, &command, result, now_unix);
    crate::history::append(history_path, &entry)
        .with_context(|| format!("{what_happened} gravar o histórico"))?;
    Ok(Applied {
        outcome: outcome?,
        after: Some(after_plan),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodus_core::preset::Plan;
    use std::io::Cursor;

    fn plano(pacman: &[&str], flatpak: &[&str]) -> Plan {
        Plan {
            pacman_missing: pacman.iter().map(|s| s.to_string()).collect(),
            pacman_present: 3,
            flatpak_missing: flatpak.iter().map(|s| s.to_string()).collect(),
            flatpak_present: 0,
            flatpak_installed: true,
        }
    }

    /// Roda `apply` com a entrada dada e um executor falso que só registra o que receberia.
    fn rodar(
        plan: &Plan,
        interactive: bool,
        entrada: &str,
        falha: bool,
    ) -> (anyhow::Result<Outcome>, String, Vec<Vec<String>>) {
        let mut input = Cursor::new(entrada.as_bytes().to_vec());
        let mut output = Vec::new();
        let mut executados = Vec::new();
        let resultado = apply(plan, interactive, &mut input, &mut output, |cmd| {
            executados.push(cmd.to_vec());
            if falha {
                anyhow::bail!("`sudo` terminou com erro (código Some(1))")
            }
            Ok(())
        });
        (resultado, String::from_utf8(output).unwrap(), executados)
    }

    // --- confirm ---

    #[test]
    fn so_s_ou_sim_confirmam() {
        for sim in ["s\n", "S\n", "sim\n", "SIM\n", "  s  \n", "s"] {
            let mut out = Vec::new();
            assert!(
                confirm(&mut Cursor::new(sim), &mut out).unwrap(),
                "{sim:?} deveria confirmar"
            );
            assert_eq!(String::from_utf8(out).unwrap(), "Executar? [s/N] ");
        }
    }

    #[test]
    fn qualquer_outra_resposta_e_nao() {
        let recusas = [
            "\n",
            "",
            "n\n",
            "nao\n",
            "y\n",
            "yes\n",
            "sim por favor\n",
            "ss\n",
        ];
        for nao in recusas {
            assert!(
                !confirm(&mut Cursor::new(nao), &mut Vec::new()).unwrap(),
                "{nao:?} não deveria confirmar"
            );
        }
    }

    // --- apply ---

    #[test]
    fn plano_vazio_nao_pergunta_nem_executa() {
        let (r, out, exec) = rodar(&plano(&[], &[]), true, "s\n", false);
        assert_eq!(r.unwrap(), Outcome::NothingToDo);
        assert!(exec.is_empty());
        assert!(!out.contains("Executar?"));
    }

    #[test]
    fn confirmado_executa_o_comando_exato_uma_vez() {
        let (r, out, exec) = rodar(&plano(&["sl", "zsh"], &[]), true, "s\n", false);
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(
            exec,
            [["sudo", "pacman", "-S", "--needed", "--", "sl", "zsh"]]
        );
        assert_eq!(
            out,
            "Comando: sudo pacman -S --needed -- sl zsh\nExecutar? [s/N] "
        );
    }

    #[test]
    fn recusado_nao_executa() {
        let (r, out, exec) = rodar(&plano(&["sl"], &[]), true, "\n", false);
        assert_eq!(r.unwrap(), Outcome::Declined);
        assert!(exec.is_empty());
        assert!(out.ends_with("Nada foi executado.\n"));
    }

    #[test]
    fn fim_da_entrada_e_recusa() {
        let (r, _, exec) = rodar(&plano(&["sl"], &[]), true, "", false);
        assert_eq!(r.unwrap(), Outcome::Declined);
        assert!(exec.is_empty());
    }

    #[test]
    fn sem_terminal_interativo_e_erro_e_nao_executa() {
        let (r, out, exec) = rodar(&plano(&["sl"], &[]), false, "s\n", false);
        let erro = r.unwrap_err().to_string();
        assert!(erro.contains("terminal interativo"), "{erro}");
        assert!(exec.is_empty());
        assert!(!out.contains("Executar?"));
    }

    #[test]
    fn falha_do_sudo_ou_do_pacman_vira_erro() {
        let (r, _, exec) = rodar(&plano(&["sl"], &[]), true, "s\n", true);
        assert!(r.is_err());
        assert_eq!(exec.len(), 1);
    }

    #[test]
    fn so_flatpak_faltando_avisa_e_nao_executa() {
        let (r, out, exec) = rodar(&plano(&[], &["org.a.B", "org.c.D"]), true, "s\n", false);
        assert_eq!(r.unwrap(), Outcome::FlatpakOnly);
        assert!(exec.is_empty());
        assert_eq!(
            out,
            "aviso: Flatpak ainda não é instalado pelo nodus: 2 app(s) do preset ficam de fora\n"
        );
    }

    #[test]
    fn pacman_e_flatpak_faltando_avisa_e_executa_so_o_pacman() {
        let (r, out, exec) = rodar(&plano(&["sl"], &["org.a.B"]), true, "s\n", false);
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(exec.len(), 1);
        assert!(out.starts_with("aviso: Flatpak ainda não é instalado pelo nodus: 1 app(s)"));
    }

    // --- apply_with_history: registra o que de fato foi executado (SECURITY.md T5) ---

    use crate::testutil::TmpDir;
    use nodus_core::history::ExecResult;
    use nodus_core::runner::RunnerError;
    use std::cell::Cell;

    const AGORA: u64 = 1_794_626_444;

    /// Roda `apply_with_history` com executor e "Depois" falsos. Devolve o resultado, quantas vezes o
    /// executor e o "Depois" foram chamados, e o histórico gravado.
    fn rodar_com_historico(
        plan: &Plan,
        interactive: bool,
        entrada: &str,
        exec_falha: Option<anyhow::Error>,
        depois: Plan,
    ) -> (
        anyhow::Result<Applied>,
        usize,
        usize,
        Vec<nodus_core::history::HistoryEntry>,
    ) {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/history.jsonl");
        let (r, execs, depois_chamado) =
            chamar(plan, interactive, entrada, exec_falha, depois, &path);
        let gravado = crate::history::read(&path).unwrap();
        (r, execs, depois_chamado, gravado)
    }

    fn chamar(
        plan: &Plan,
        interactive: bool,
        entrada: &str,
        exec_falha: Option<anyhow::Error>,
        depois: Plan,
        path: &std::path::Path,
    ) -> (anyhow::Result<Applied>, usize, usize) {
        let mut input = Cursor::new(entrada.as_bytes().to_vec());
        let mut output = Vec::new();
        let execs = Cell::new(0);
        let depois_chamado = Cell::new(0);
        let mut falha = exec_falha;
        let r = apply_with_history(
            plan,
            interactive,
            &mut input,
            &mut output,
            |_| {
                execs.set(execs.get() + 1);
                match falha.take() {
                    Some(e) => Err(e),
                    None => Ok(()),
                }
            },
            || {
                depois_chamado.set(depois_chamado.get() + 1);
                Ok(depois)
            },
            path,
            AGORA,
        );
        (r, execs.get(), depois_chamado.get())
    }

    #[test]
    fn grava_em_sucesso_com_o_estado_de_depois() {
        let (r, execs, depois_chamado, hist) = rodar_com_historico(
            &plano(&["sl", "cowsay"], &[]),
            true,
            "s\n",
            None,
            plano(&[], &[]),
        );
        let aplicado = r.unwrap();
        assert_eq!(aplicado.outcome, Outcome::Executed);
        assert_eq!(aplicado.after, Some(plano(&[], &[])));
        assert_eq!((execs, depois_chamado), (1, 1));
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].at_unix, AGORA);
        assert_eq!(hist[0].requested, ["sl", "cowsay"]);
        assert_eq!(hist[0].installed, ["sl", "cowsay"]);
        assert!(hist[0].still_missing.is_empty());
        assert_eq!(hist[0].result, ExecResult::Ok);
        assert_eq!(
            hist[0].command,
            ["sudo", "pacman", "-S", "--needed", "--", "sl", "cowsay"]
        );
    }

    #[test]
    fn grava_em_falha_do_pacman_e_propaga_o_erro() {
        let falha = anyhow::Error::from(RunnerError::Failed {
            program: "sudo".into(),
            code: Some(1),
        });
        let (r, execs, _, hist) = rodar_com_historico(
            &plano(&["sl"], &[]),
            true,
            "s\n",
            Some(falha),
            plano(&["sl"], &[]),
        );
        assert!(r.is_err());
        assert_eq!(execs, 1);
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].result, ExecResult::Failed { code: Some(1) });
        assert_eq!(hist[0].still_missing, ["sl"]);
        assert!(hist[0].installed.is_empty());
    }

    #[test]
    fn falha_sem_codigo_de_saida_grava_code_nenhum() {
        let (r, _, _, hist) = rodar_com_historico(
            &plano(&["sl"], &[]),
            true,
            "s\n",
            Some(anyhow::anyhow!("não consegui iniciar `sudo`")),
            plano(&["sl"], &[]),
        );
        assert!(r.is_err());
        assert_eq!(hist[0].result, ExecResult::Failed { code: None });
    }

    #[test]
    fn nao_grava_na_recusa_nem_consulta_o_depois() {
        let (r, execs, depois_chamado, hist) =
            rodar_com_historico(&plano(&["sl"], &[]), true, "n\n", None, plano(&[], &[]));
        assert_eq!(r.unwrap().outcome, Outcome::Declined);
        assert_eq!((execs, depois_chamado), (0, 0));
        assert!(hist.is_empty());
    }

    #[test]
    fn nao_grava_com_plano_vazio() {
        let (r, execs, depois_chamado, hist) =
            rodar_com_historico(&plano(&[], &[]), true, "s\n", None, plano(&[], &[]));
        assert_eq!(r.unwrap().outcome, Outcome::NothingToDo);
        assert_eq!((execs, depois_chamado), (0, 0));
        assert!(hist.is_empty());
    }

    #[test]
    fn nao_grava_quando_so_falta_flatpak() {
        let (r, execs, _, hist) = rodar_com_historico(
            &plano(&[], &["org.a.B"]),
            true,
            "s\n",
            None,
            plano(&[], &[]),
        );
        assert_eq!(r.unwrap().outcome, Outcome::FlatpakOnly);
        assert_eq!(execs, 0);
        assert!(hist.is_empty());
    }

    #[test]
    fn nao_grava_sem_terminal() {
        let (r, execs, _, hist) =
            rodar_com_historico(&plano(&["sl"], &[]), false, "s\n", None, plano(&[], &[]));
        assert!(r.is_err());
        assert_eq!(execs, 0);
        assert!(hist.is_empty());
    }

    #[test]
    fn falha_ao_gravar_avisa_que_a_instalacao_aconteceu() {
        // O pai do arquivo de histórico é um arquivo comum: criar o diretório falha.
        let tmp = TmpDir::novo();
        let bloqueio = tmp.path().join("bloqueio");
        std::fs::write(&bloqueio, "").unwrap();
        let path = bloqueio.join("nodus/history.jsonl");
        let (r, execs, _) = chamar(
            &plano(&["sl"], &[]),
            true,
            "s\n",
            None,
            plano(&[], &[]),
            &path,
        );
        assert_eq!(execs, 1);
        let erro = format!("{:#}", r.unwrap_err());
        assert!(erro.contains("instalação foi feita"), "{erro}");
        assert!(erro.contains("histórico"), "{erro}");
    }
}
