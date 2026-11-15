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

    type Executado = (Manager, Vec<String>);

    /// Roda `apply` com a entrada dada, `flathub` configurado e um executor falso que só registra o
    /// que receberia. `falha_em`: o passo desse gerenciador termina com erro.
    fn rodar(
        plan: &Plan,
        interactive: bool,
        entrada: &str,
        falha_em: Option<Manager>,
    ) -> (anyhow::Result<Outcome>, String, Vec<Executado>) {
        rodar_com_flathub(plan, interactive, entrada, falha_em, || Ok(true))
    }

    fn rodar_com_flathub(
        plan: &Plan,
        interactive: bool,
        entrada: &str,
        falha_em: Option<Manager>,
        flathub: impl FnOnce() -> anyhow::Result<bool>,
    ) -> (anyhow::Result<Outcome>, String, Vec<Executado>) {
        let mut input = Cursor::new(entrada.as_bytes().to_vec());
        let mut output = Vec::new();
        let mut executados = Vec::new();
        let resultado = apply(
            plan,
            interactive,
            &mut input,
            &mut output,
            flathub,
            |manager, cmd| {
                executados.push((manager, cmd.to_vec()));
                if falha_em == Some(manager) {
                    anyhow::bail!("o comando terminou com erro (código Some(1))")
                }
                Ok(())
            },
        );
        (resultado, String::from_utf8(output).unwrap(), executados)
    }

    fn cmd_pacman(pacotes: &[&str]) -> Vec<String> {
        let mut c: Vec<String> = ["sudo", "pacman", "-S", "--needed", "--"]
            .map(String::from)
            .to_vec();
        c.extend(pacotes.iter().map(|s| s.to_string()));
        c
    }

    fn cmd_flatpak(apps: &[&str]) -> Vec<String> {
        let mut c: Vec<String> = ["flatpak", "install", "--user", "--", "flathub"]
            .map(String::from)
            .to_vec();
        c.extend(apps.iter().map(|s| s.to_string()));
        c
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
    fn plano_vazio_nao_pergunta_nem_executa_nem_consulta_o_flathub() {
        let (r, out, exec) = rodar_com_flathub(&plano(&[], &[]), true, "s\n", None, || {
            panic!("não há app Flatpak a instalar: o flathub não deveria ser consultado")
        });
        assert_eq!(r.unwrap(), Outcome::NothingToDo);
        assert!(exec.is_empty());
        assert!(!out.contains("Executar?"));
    }

    #[test]
    fn so_pacman_executa_o_comando_exato_uma_vez_sem_consultar_o_flathub() {
        let (r, out, exec) =
            rodar_com_flathub(&plano(&["sl", "zsh"], &[]), true, "s\n", None, || {
                panic!("não há app Flatpak a instalar: o flathub não deveria ser consultado")
            });
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(exec, [(Manager::Pacman, cmd_pacman(&["sl", "zsh"]))]);
        assert_eq!(
            out,
            "Comando: sudo pacman -S --needed -- sl zsh\nExecutar? [s/N] "
        );
    }

    #[test]
    fn so_flatpak_executa_o_comando_exato_uma_vez() {
        let (r, out, exec) = rodar(
            &plano(&[], &["org.gnome.Calculator", "org.a.B"]),
            true,
            "s\n",
            None,
        );
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(
            exec,
            [(
                Manager::Flatpak,
                cmd_flatpak(&["org.gnome.Calculator", "org.a.B"])
            )]
        );
        assert_eq!(
            out,
            "Comando: flatpak install --user -- flathub org.gnome.Calculator org.a.B\nExecutar? [s/N] "
        );
    }

    #[test]
    fn pacman_e_flatpak_executam_em_sequencia_cada_um_com_a_sua_confirmacao() {
        let (r, out, exec) = rodar(&plano(&["sl"], &["org.a.B"]), true, "s\ns\n", None);
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(
            exec,
            [
                (Manager::Pacman, cmd_pacman(&["sl"])),
                (Manager::Flatpak, cmd_flatpak(&["org.a.B"])),
            ]
        );
        assert_eq!(
            out,
            "Comando: sudo pacman -S --needed -- sl\nExecutar? [s/N] \
             Comando: flatpak install --user -- flathub org.a.B\nExecutar? [s/N] "
        );
    }

    #[test]
    fn recusar_o_pacman_nao_impede_oferecer_o_flatpak() {
        let (r, out, exec) = rodar(&plano(&["sl"], &["org.a.B"]), true, "n\ns\n", None);
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(exec, [(Manager::Flatpak, cmd_flatpak(&["org.a.B"]))]);
        assert!(out.contains("Nada foi executado.\n"));
    }

    #[test]
    fn recusar_o_flatpak_depois_de_instalar_o_pacman_conta_como_executado() {
        let (r, _, exec) = rodar(&plano(&["sl"], &["org.a.B"]), true, "s\nn\n", None);
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(exec, [(Manager::Pacman, cmd_pacman(&["sl"]))]);
    }

    #[test]
    fn recusar_os_dois_nao_executa_nada() {
        let (r, out, exec) = rodar(&plano(&["sl"], &["org.a.B"]), true, "n\nn\n", None);
        assert_eq!(r.unwrap(), Outcome::Declined);
        assert!(exec.is_empty());
        assert!(out.ends_with("Nada foi executado.\n"));
    }

    #[test]
    fn fim_da_entrada_e_recusa() {
        let (r, _, exec) = rodar(&plano(&["sl"], &["org.a.B"]), true, "", None);
        assert_eq!(r.unwrap(), Outcome::Declined);
        assert!(exec.is_empty());
    }

    #[test]
    fn sem_terminal_interativo_e_erro_e_nao_executa() {
        for plan in [
            plano(&["sl"], &[]),
            plano(&[], &["org.a.B"]),
            plano(&["sl"], &["org.a.B"]),
        ] {
            let (r, out, exec) = rodar(&plan, false, "s\ns\n", None);
            let erro = r.unwrap_err().to_string();
            assert!(erro.contains("terminal interativo"), "{erro}");
            assert!(exec.is_empty());
            assert!(!out.contains("Executar?"));
        }
    }

    #[test]
    fn falha_do_pacman_interrompe_e_nao_oferece_o_flatpak() {
        let (r, out, exec) = rodar(
            &plano(&["sl"], &["org.a.B"]),
            true,
            "s\ns\n",
            Some(Manager::Pacman),
        );
        assert!(r.is_err());
        assert_eq!(exec, [(Manager::Pacman, cmd_pacman(&["sl"]))]);
        assert!(!out.contains("flatpak install"), "{out}");
    }

    #[test]
    fn falha_do_flatpak_vira_erro_depois_de_instalar_o_pacman() {
        let (r, _, exec) = rodar(
            &plano(&["sl"], &["org.a.B"]),
            true,
            "s\ns\n",
            Some(Manager::Flatpak),
        );
        assert!(r.is_err());
        assert_eq!(exec.len(), 2);
    }

    #[test]
    fn sem_o_remote_flathub_para_antes_de_qualquer_pergunta() {
        // Mesmo com pacman faltando: nada pode ser instalado se o passo Flatpak já se sabe inviável.
        let (r, out, exec) =
            rodar_com_flathub(&plano(&["sl"], &["org.a.B"]), true, "s\ns\n", None, || {
                Ok(false)
            });
        let erro = r.unwrap_err().to_string();
        assert!(erro.contains("flathub"), "{erro}");
        assert!(exec.is_empty());
        assert!(!out.contains("Executar?"), "{out}");
    }

    #[test]
    fn falha_ao_consultar_o_flathub_vira_erro_sem_executar() {
        let (r, out, exec) =
            rodar_com_flathub(&plano(&["sl"], &["org.a.B"]), true, "s\ns\n", None, || {
                anyhow::bail!("não consegui listar os remotes")
            });
        assert!(r.is_err());
        assert!(exec.is_empty());
        assert!(!out.contains("Executar?"), "{out}");
    }

    #[test]
    fn flatpak_nao_instalado_com_app_no_preset_e_erro_claro_sem_consultar_o_flathub() {
        let mut plan = plano(&["sl"], &["org.a.B"]);
        plan.flatpak_installed = false;
        let (r, out, exec) = rodar_com_flathub(&plan, true, "s\ns\n", None, || {
            panic!("sem Flatpak instalado não há o que consultar")
        });
        let erro = r.unwrap_err().to_string();
        assert!(erro.contains("Flatpak não está instalado"), "{erro}");
        assert!(exec.is_empty());
        assert!(!out.contains("Executar?"), "{out}");
    }

    #[test]
    fn flatpak_nao_instalado_sem_app_flatpak_no_preset_nao_atrapalha_o_pacman() {
        let mut plan = plano(&["sl"], &[]);
        plan.flatpak_installed = false;
        let (r, _, exec) = rodar(&plan, true, "s\n", None);
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(exec, [(Manager::Pacman, cmd_pacman(&["sl"]))]);
    }

    // --- apply_with_history: registra o que de fato foi executado (SECURITY.md T5) ---

    use crate::testutil::TmpDir;
    use nodus_core::history::{ExecResult, HistoryEntry, Manager};
    use nodus_core::runner::RunnerError;
    use std::cell::Cell;

    const AGORA: u64 = 1_794_626_444;

    /// Roda `apply_with_history` com executor e "Depois" falsos. Devolve o resultado, quantas vezes o
    /// executor e o "Depois" foram chamados, e o histórico gravado. `exec_falha`: o primeiro
    /// comando executado falha com esse erro (nos testes só com pacman, é o do pacman).
    fn rodar_com_historico(
        plan: &Plan,
        interactive: bool,
        entrada: &str,
        exec_falha: Option<anyhow::Error>,
        depois: Plan,
    ) -> (anyhow::Result<Applied>, usize, usize, Vec<HistoryEntry>) {
        let falha = exec_falha.map(|e| (Manager::Pacman, e));
        rodar_em_sequencia(plan, interactive, entrada, falha, vec![depois])
    }

    /// Como `rodar_com_historico`, para planos com os dois gerenciadores. `falha`: o passo desse
    /// gerenciador termina com esse erro. `depois`: o plano que cada conferência devolve, em ordem
    /// (a última é repetida se faltar).
    fn rodar_em_sequencia(
        plan: &Plan,
        interactive: bool,
        entrada: &str,
        falha: Option<(Manager, anyhow::Error)>,
        depois: Vec<Plan>,
    ) -> (anyhow::Result<Applied>, usize, usize, Vec<HistoryEntry>) {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/history.jsonl");
        let (r, execs, depois_chamado) = chamar(plan, interactive, entrada, falha, depois, &path);
        let gravado = crate::history::read(&path).unwrap();
        (r, execs, depois_chamado, gravado)
    }

    fn chamar(
        plan: &Plan,
        interactive: bool,
        entrada: &str,
        falha: Option<(Manager, anyhow::Error)>,
        depois: Vec<Plan>,
        path: &std::path::Path,
    ) -> (anyhow::Result<Applied>, usize, usize) {
        let mut input = Cursor::new(entrada.as_bytes().to_vec());
        let mut output = Vec::new();
        let execs = Cell::new(0);
        let depois_chamado = Cell::new(0);
        let mut falha = falha;
        let r = apply_with_history(
            plan,
            interactive,
            &mut input,
            &mut output,
            || Ok(true),
            |command| {
                execs.set(execs.get() + 1);
                let manager = if command[0] == "flatpak" {
                    Manager::Flatpak
                } else {
                    Manager::Pacman
                };
                match falha.take_if(|(m, _)| *m == manager) {
                    Some((_, e)) => Err(e),
                    None => Ok(()),
                }
            },
            || {
                let i = depois_chamado.get();
                depois_chamado.set(i + 1);
                Ok(depois[i.min(depois.len() - 1)].clone())
            },
            path,
            AGORA,
        );
        (r, execs.get(), depois_chamado.get())
    }

    #[test]
    fn dois_passos_gravam_uma_entrada_por_gerenciador_na_ordem() {
        let (r, execs, depois_chamado, hist) = rodar_em_sequencia(
            &plano(&["sl"], &["org.gnome.Calculator"]),
            true,
            "s\ns\n",
            None,
            // Depois do pacman o Flatpak ainda falta; depois do Flatpak, nada falta.
            vec![plano(&[], &["org.gnome.Calculator"]), plano(&[], &[])],
        );
        let aplicado = r.unwrap();
        assert_eq!(aplicado.outcome, Outcome::Executed);
        assert_eq!(aplicado.executed, [Manager::Pacman, Manager::Flatpak]);
        assert_eq!(aplicado.after, Some(plano(&[], &[])));
        assert_eq!((execs, depois_chamado), (2, 2));
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].manager, Manager::Pacman);
        assert_eq!(hist[0].requested, ["sl"]);
        assert_eq!(hist[0].installed, ["sl"]);
        assert_eq!(hist[1].manager, Manager::Flatpak);
        assert_eq!(hist[1].requested, ["org.gnome.Calculator"]);
        assert_eq!(hist[1].installed, ["org.gnome.Calculator"]);
        assert!(hist[1].still_missing.is_empty());
        assert_eq!(hist[1].result, ExecResult::Ok);
        assert_eq!(
            hist[1].command,
            [
                "flatpak",
                "install",
                "--user",
                "--",
                "flathub",
                "org.gnome.Calculator"
            ]
        );
        assert_eq!(hist[1].at_unix, AGORA);
    }

    #[test]
    fn so_flatpak_grava_uma_entrada_flatpak() {
        let (r, _, _, hist) = rodar_em_sequencia(
            &plano(&[], &["org.a.B"]),
            true,
            "s\n",
            None,
            vec![plano(&[], &[])],
        );
        assert_eq!(r.unwrap().executed, [Manager::Flatpak]);
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].manager, Manager::Flatpak);
    }

    #[test]
    fn falha_do_flatpak_grava_a_entrada_com_o_codigo_e_propaga_o_erro() {
        let falha = anyhow::Error::from(RunnerError::Failed {
            program: "flatpak".into(),
            code: Some(1),
        });
        let (r, execs, _, hist) = rodar_em_sequencia(
            &plano(&[], &["org.a.B"]),
            true,
            "s\n",
            Some((Manager::Flatpak, falha)),
            vec![plano(&[], &["org.a.B"])],
        );
        assert!(r.is_err());
        assert_eq!(execs, 1);
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].manager, Manager::Flatpak);
        assert_eq!(hist[0].result, ExecResult::Failed { code: Some(1) });
        assert_eq!(hist[0].still_missing, ["org.a.B"]);
        assert!(hist[0].installed.is_empty());
    }

    #[test]
    fn falha_do_pacman_grava_so_a_entrada_do_pacman() {
        let falha = anyhow::Error::from(RunnerError::Failed {
            program: "sudo".into(),
            code: Some(1),
        });
        let (r, execs, _, hist) = rodar_em_sequencia(
            &plano(&["sl"], &["org.a.B"]),
            true,
            "s\ns\n",
            Some((Manager::Pacman, falha)),
            vec![plano(&["sl"], &["org.a.B"])],
        );
        assert!(r.is_err());
        assert_eq!(execs, 1);
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].manager, Manager::Pacman);
    }

    #[test]
    fn recusar_um_passo_nao_grava_entrada_dele() {
        let (r, execs, depois_chamado, hist) = rodar_em_sequencia(
            &plano(&["sl"], &["org.a.B"]),
            true,
            "n\ns\n",
            None,
            vec![plano(&["sl"], &[])],
        );
        assert_eq!(r.unwrap().executed, [Manager::Flatpak]);
        assert_eq!((execs, depois_chamado), (1, 1));
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].manager, Manager::Flatpak);
    }

    #[test]
    fn sem_flathub_nao_grava_nada() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/history.jsonl");
        let mut output = Vec::new();
        let r = apply_with_history(
            &plano(&["sl"], &["org.a.B"]),
            true,
            &mut Cursor::new(b"s\ns\n".to_vec()),
            &mut output,
            || Ok(false),
            |_| panic!("nada deveria ser executado"),
            || panic!("nada deveria ser conferido"),
            &path,
            AGORA,
        );
        assert!(r.is_err());
        assert!(crate::history::read(&path).unwrap().is_empty());
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
            vec![plano(&[], &[])],
            &path,
        );
        assert_eq!(execs, 1);
        let erro = format!("{:#}", r.unwrap_err());
        assert!(erro.contains("instalação foi feita"), "{erro}");
        assert!(erro.contains("histórico"), "{erro}");
    }
}
