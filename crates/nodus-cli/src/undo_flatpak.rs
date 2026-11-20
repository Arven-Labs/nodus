//! `nodusctl history undo <n>` de uma instalação de app Flatpak (ADR da reversão de Flatpak). Diferente
//! do pacman, o Flatpak não tem simulação: o nodus confere o que segue instalado, mostra o comando
//! exato e pede `Executar? [s/N]`; depois o próprio `flatpak uninstall` mostra o que remove e
//! pergunta de novo. O histórico não é assinado (T5): a validação dos IDs é da leitura, e o `--`
//! protege o argv.

use crate::apply::confirm;
use crate::undo::Outcome;
use anyhow::{Context, Result, bail};
use nodus_core::history::{ExecResult, HistoryEntry};
use nodus_core::runner::RunnerError;
use nodus_core::undo::{
    entry_for_flatpak_undo, flatpak_remove_command, flatpak_undo_candidates, select_flatpak_entry,
};
use std::io::{BufRead, Write};
use std::path::Path;

pub struct Request<'a> {
    pub entries: &'a [HistoryEntry],
    /// Número da entrada, a partir de 1.
    pub n: u32,
    /// `flatpak list --app --user` (IDs).
    pub installed: &'a [String],
    /// A entrada é um terminal (sem ele não há como confirmar, e não existe "sim para tudo").
    pub interactive: bool,
    pub now_unix: u64,
}

/// Desfaz a instalação Flatpak `n` e registra a reversão. `exec` roda o comando real com o terminal
/// herdado e `after_installed` devolve o `flatpak list --app --user` de depois. Só grava no
/// histórico quando o comando chegou a rodar, mesmo que tenha falhado.
pub fn undo(
    req: &Request,
    input: &mut impl BufRead,
    output: &mut impl Write,
    mut exec: impl FnMut(&[String]) -> Result<()>,
    after_installed: impl FnOnce() -> Result<Vec<String>>,
    history_path: &Path,
) -> Result<Outcome> {
    let entry = select_flatpak_entry(req.entries, req.n)?;
    let candidates = flatpak_undo_candidates(entry, req.installed);
    for skipped in &candidates.skipped {
        writeln!(output, "ignorado: {} ({})", skipped.name, skipped.reason)?;
    }
    let Some(command) = flatpak_remove_command(&candidates.remove) else {
        writeln!(
            output,
            "nada a desfazer: nenhum app da entrada pode ser removido"
        )?;
        return Ok(Outcome::NothingToDo);
    };

    writeln!(output, "Comando: {}", command.join(" "))?;
    writeln!(
        output,
        "O Flatpak mostra o que remove e pergunta de novo."
    )?;
    if !req.interactive {
        bail!(
            "`history undo` precisa de terminal interativo para confirmar (não há modo automático)"
        );
    }
    if !confirm(input, output)? {
        writeln!(output, "Nada foi executado.")?;
        return Ok(Outcome::Declined);
    }

    let run = exec(&command);
    let result = match &run {
        Ok(()) => ExecResult::Ok,
        Err(e) => ExecResult::Failed {
            code: match e.downcast_ref::<RunnerError>() {
                Some(RunnerError::Failed { code, .. }) => *code,
                _ => None,
            },
        },
    };
    let what_happened = if run.is_err() {
        "o `flatpak` falhou e, além disso, não consegui"
    } else {
        "a remoção foi feita, mas não consegui"
    };
    let after =
        after_installed().with_context(|| format!("{what_happened} conferir o estado depois"))?;
    let entry = entry_for_flatpak_undo(
        req.n,
        &candidates.remove,
        &after,
        &command,
        result,
        req.now_unix,
    );
    crate::history::append(history_path, &entry)
        .with_context(|| format!("{what_happened} gravar o histórico"))?;
    run?;
    Ok(Outcome::Executed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TmpDir;
    use crate::undo::Outcome;
    use nodus_core::history::{Action, ExecResult, HistoryEntry, Manager};
    use nodus_core::runner::RunnerError;
    use std::io::Cursor;

    fn v(itens: &[&str]) -> Vec<String> {
        itens.iter().map(|s| s.to_string()).collect()
    }

    fn instalacao(installed: &[&str]) -> HistoryEntry {
        let mut comando = v(&["flatpak", "install", "--user", "--", "flathub"]);
        comando.extend(v(installed));
        HistoryEntry {
            version: 3,
            at_unix: 1_700_000_000,
            command: comando,
            requested: v(installed),
            installed: v(installed),
            still_missing: vec![],
            result: ExecResult::Ok,
            action: Action::Install,
            undoes: None,
            manager: Manager::Flatpak,
            dir: None,
        }
    }

    fn instalacao_pacman(installed: &[&str]) -> HistoryEntry {
        let mut e = instalacao(installed);
        e.command = v(&["sudo", "pacman", "-S", "--needed", "--"]);
        e.manager = Manager::Pacman;
        e
    }

    struct Cenario {
        entradas: Vec<HistoryEntry>,
        n: u32,
        instalados: Vec<String>,
        interativo: bool,
        resposta: &'static str,
        comando_falha: bool,
        depois: Option<Vec<String>>,
    }

    fn cenario() -> Cenario {
        Cenario {
            entradas: vec![instalacao(&["org.gnome.Calculator"])],
            n: 1,
            instalados: v(&["org.gnome.Calculator", "org.outro.App"]),
            interativo: true,
            resposta: "s\n",
            comando_falha: false,
            depois: Some(v(&["org.outro.App"])),
        }
    }

    struct Saida {
        resultado: anyhow::Result<Outcome>,
        texto: String,
        executou: Vec<Vec<String>>,
        arquivo_existe: bool,
        gravadas: Vec<HistoryEntry>,
    }

    fn rodar(c: Cenario) -> Saida {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/history.jsonl");
        let req = Request {
            entries: &c.entradas,
            n: c.n,
            installed: &c.instalados,
            interactive: c.interativo,
            now_unix: 1_794_626_444,
        };
        let mut input = Cursor::new(c.resposta.as_bytes().to_vec());
        let mut output = Vec::new();
        let mut executou = Vec::new();
        let resultado = undo(
            &req,
            &mut input,
            &mut output,
            |cmd| {
                executou.push(cmd.to_vec());
                if c.comando_falha {
                    return Err(RunnerError::Failed {
                        program: "flatpak".into(),
                        code: Some(1),
                    }
                    .into());
                }
                Ok(())
            },
            || {
                c.depois
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("flatpak list falhou"))
            },
            &path,
        );
        Saida {
            resultado,
            texto: String::from_utf8(output).unwrap(),
            executou,
            arquivo_existe: path.exists(),
            gravadas: crate::history::read(&path).unwrap(),
        }
    }

    fn erro(s: &Saida) -> String {
        format!("{:#}", s.resultado.as_ref().unwrap_err())
    }

    // --- o caminho feliz ---

    #[test]
    fn fluxo_completo_mostra_confirma_executa_e_grava() {
        let s = rodar(cenario());
        assert_eq!(s.resultado.as_ref().unwrap(), &Outcome::Executed);
        assert_eq!(
            s.executou,
            vec![v(&[
                "flatpak",
                "uninstall",
                "--user",
                "--",
                "org.gnome.Calculator"
            ])]
        );
        assert!(
            s.texto
                .contains("Comando: flatpak uninstall --user -- org.gnome.Calculator"),
            "{}",
            s.texto
        );
        assert!(s.texto.contains("Executar? [s/N] "), "{}", s.texto);
        // Sem simulação (o Flatpak não tem): o texto avisa que o prompt dele é quem mostra o que sai.
        assert!(
            s.texto.contains("O Flatpak mostra o que remove e pergunta de novo"),
            "{}",
            s.texto
        );
        assert_eq!(s.gravadas.len(), 1);
        let e = &s.gravadas[0];
        assert_eq!(e.version, 3);
        assert_eq!(e.manager, Manager::Flatpak);
        assert_eq!(e.action, Action::Remove);
        assert_eq!(e.undoes, Some(1));
        assert_eq!(e.requested, v(&["org.gnome.Calculator"]));
        assert!(e.installed.is_empty());
        assert_eq!(e.still_missing, v(&["org.gnome.Calculator"]));
        assert_eq!(e.result, ExecResult::Ok);
        assert_eq!(
            e.command,
            v(&["flatpak", "uninstall", "--user", "--", "org.gnome.Calculator"])
        );
        assert_eq!(e.at_unix, 1_794_626_444);
    }

    #[test]
    fn o_comando_mostrado_vem_antes_da_pergunta() {
        let s = rodar(cenario());
        let comando = s.texto.find("Comando: ").expect("comando na tela");
        let pergunta = s.texto.find("Executar? [s/N]").expect("pergunta na tela");
        assert!(comando < pergunta, "{}", s.texto);
    }

    #[test]
    fn o_comando_nunca_leva_flag_perigosa_e_o_separador_vem_antes_dos_ids() {
        let mut c = cenario();
        c.entradas = vec![instalacao(&["org.a.B", "org.c.D"])];
        c.instalados = v(&["org.a.B", "org.c.D"]);
        c.depois = Some(vec![]);
        let s = rodar(c);
        let cmd = &s.executou[0];
        assert_eq!(cmd[..4], v(&["flatpak", "uninstall", "--user", "--"]));
        assert_eq!(cmd[4..], v(&["org.a.B", "org.c.D"]));
        for proibida in [
            "-y",
            "--assumeyes",
            "--noninteractive",
            "--unused",
            "--delete-data",
            "--system",
            "--all",
        ] {
            assert!(!cmd.iter().any(|a| a == proibida), "{proibida} em {cmd:?}");
        }
    }

    #[test]
    fn so_os_candidatos_vao_para_o_comando_e_os_ignorados_aparecem_na_tela() {
        let mut c = cenario();
        c.entradas = vec![instalacao(&["org.a.B", "org.c.D"])];
        c.instalados = v(&["org.a.B", "org.x.Y"]);
        c.depois = Some(v(&["org.x.Y"]));
        let s = rodar(c);
        assert_eq!(
            s.executou,
            vec![v(&["flatpak", "uninstall", "--user", "--", "org.a.B"])]
        );
        assert!(
            s.texto.contains("ignorado: org.c.D (não está instalado)"),
            "{}",
            s.texto
        );
    }

    #[test]
    fn app_instalado_que_nao_esta_na_entrada_nunca_e_alvo() {
        let mut c = cenario();
        c.entradas = vec![instalacao(&["org.a.B"])];
        c.instalados = v(&["org.a.B", "org.gnome.Calculator", "org.mozilla.firefox"]);
        c.depois = Some(v(&["org.gnome.Calculator", "org.mozilla.firefox"]));
        let s = rodar(c);
        assert_eq!(
            s.executou,
            vec![v(&["flatpak", "uninstall", "--user", "--", "org.a.B"])]
        );
    }

    #[test]
    fn instalacao_que_falhou_mas_instalou_algo_pode_ser_desfeita() {
        let mut c = cenario();
        c.entradas[0].result = ExecResult::Failed { code: Some(1) };
        let s = rodar(c);
        assert_eq!(s.resultado.as_ref().unwrap(), &Outcome::Executed);
    }

    // --- nada a fazer: sem comando, sem registro ---

    #[test]
    fn sem_candidatos_diz_nada_a_desfazer_e_nao_chama_nada() {
        let mut c = cenario();
        c.instalados = v(&["org.outro.App"]);
        let s = rodar(c);
        assert_eq!(s.resultado.as_ref().unwrap(), &Outcome::NothingToDo);
        assert!(s.texto.contains("nada a desfazer"), "{}", s.texto);
        assert!(
            s.texto
                .contains("ignorado: org.gnome.Calculator (não está instalado)")
        );
        assert!(s.executou.is_empty());
        assert!(!s.arquivo_existe);
    }

    #[test]
    fn desfazer_de_novo_a_mesma_entrada_diz_nada_a_desfazer() {
        // Depois da remoção o app já não está instalado: a segunda tentativa não tem candidato.
        let mut c = cenario();
        c.instalados = v(&["org.outro.App"]);
        c.entradas.push({
            let mut r = instalacao(&[]);
            r.action = Action::Remove;
            r.undoes = Some(1);
            r.requested = v(&["org.gnome.Calculator"]);
            r.still_missing = v(&["org.gnome.Calculator"]);
            r
        });
        let s = rodar(c);
        assert_eq!(s.resultado.as_ref().unwrap(), &Outcome::NothingToDo);
        assert!(s.executou.is_empty());
    }

    // --- confirmação do nodus: só `s`/`sim`; sem terminal não há como confirmar ---

    #[test]
    fn recusa_nao_executa_e_nao_grava() {
        for resposta in ["n\n", "\n", "", "nao\n", "y\n", "sim por favor\n"] {
            let mut c = cenario();
            c.resposta = resposta;
            let s = rodar(c);
            assert_eq!(
                s.resultado.as_ref().unwrap(),
                &Outcome::Declined,
                "{resposta:?}"
            );
            assert!(s.executou.is_empty(), "{resposta:?}");
            assert!(!s.arquivo_existe, "{resposta:?}");
            assert!(s.texto.contains("Nada foi executado."), "{resposta:?}");
        }
    }

    #[test]
    fn so_s_e_sim_confirmam() {
        for resposta in ["s\n", "sim\n", "S\n", "SIM\n", " s \n"] {
            let mut c = cenario();
            c.resposta = resposta;
            let s = rodar(c);
            assert_eq!(
                s.resultado.as_ref().unwrap(),
                &Outcome::Executed,
                "{resposta:?}"
            );
        }
    }

    #[test]
    fn sem_terminal_mostra_o_comando_mas_nao_executa_nem_grava() {
        let mut c = cenario();
        c.interativo = false;
        c.resposta = "s\n";
        let s = rodar(c);
        assert!(erro(&s).contains("terminal interativo"), "{}", erro(&s));
        assert!(s.texto.contains("Comando: flatpak uninstall"), "{}", s.texto);
        assert!(s.executou.is_empty());
        assert!(!s.arquivo_existe);
    }

    // --- falhas depois de confirmar: o registro mais importa nesse momento ---

    #[test]
    fn falha_do_comando_e_gravada_com_o_codigo() {
        let mut c = cenario();
        c.comando_falha = true;
        c.depois = Some(v(&["org.gnome.Calculator", "org.outro.App"]));
        let s = rodar(c);
        assert!(s.resultado.is_err());
        assert_eq!(s.gravadas.len(), 1);
        let e = &s.gravadas[0];
        assert_eq!(e.action, Action::Remove);
        assert_eq!(e.manager, Manager::Flatpak);
        assert_eq!(e.result, ExecResult::Failed { code: Some(1) });
        assert_eq!(e.installed, v(&["org.gnome.Calculator"]));
        assert!(e.still_missing.is_empty());
    }

    #[test]
    fn recusar_no_prompt_do_flatpak_grava_o_que_ficou_instalado() {
        // O Flatpak saiu com erro (recusa dele) e o app segue instalado: o registro diz isso.
        let mut c = cenario();
        c.comando_falha = true;
        c.depois = Some(v(&["org.gnome.Calculator"]));
        let s = rodar(c);
        let e = &s.gravadas[0];
        assert_eq!(e.installed, v(&["org.gnome.Calculator"]));
        assert!(matches!(e.result, ExecResult::Failed { .. }));
    }

    #[test]
    fn remocao_parcial_separa_o_que_ficou_do_que_saiu() {
        let mut c = cenario();
        c.entradas = vec![instalacao(&["org.a.B", "org.c.D"])];
        c.instalados = v(&["org.a.B", "org.c.D"]);
        c.depois = Some(v(&["org.c.D"]));
        let s = rodar(c);
        let e = &s.gravadas[0];
        assert_eq!(e.requested, v(&["org.a.B", "org.c.D"]));
        assert_eq!(e.installed, v(&["org.c.D"]));
        assert_eq!(e.still_missing, v(&["org.a.B"]));
    }

    #[test]
    fn nao_conseguir_conferir_o_estado_depois_diz_o_que_ja_aconteceu() {
        let mut c = cenario();
        c.depois = None;
        let s = rodar(c);
        let e = erro(&s);
        assert!(e.contains("conferir o estado"), "{e}");
        assert_eq!(s.executou.len(), 1);
    }

    // --- escolha da entrada: só instalação Flatpak ---

    #[test]
    fn entrada_invalida_ou_de_outro_tipo_nao_chama_nada() {
        let mut remocao = instalacao(&[]);
        remocao.action = Action::Remove;
        remocao.undoes = Some(1);
        for (entradas, n, trecho) in [
            (vec![instalacao(&["org.a.B"])], 0, "fora"),
            (vec![instalacao(&["org.a.B"])], 2, "fora"),
            (vec![], 1, "fora"),
            (vec![instalacao_pacman(&["sl"])], 1, "Flatpak"),
            (
                vec![instalacao(&["org.a.B"]), remocao],
                2,
                "não é uma instalação",
            ),
        ] {
            let mut c = cenario();
            c.entradas = entradas;
            c.n = n;
            let s = rodar(c);
            assert!(erro(&s).contains(trecho), "n={n}: {}", erro(&s));
            assert!(s.executou.is_empty());
            assert!(!s.arquivo_existe);
        }
    }
}
