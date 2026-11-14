#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::Capture;
    use crate::testutil::TmpDir;
    use nodus_core::history::{Action, ExecResult, HistoryEntry};
    use nodus_core::runner::RunnerError;
    use std::io::Cursor;

    fn v(itens: &[&str]) -> Vec<String> {
        itens.iter().map(|s| s.to_string()).collect()
    }

    fn instalacao(installed: &[&str]) -> HistoryEntry {
        HistoryEntry {
            version: 2,
            at_unix: 1_700_000_000,
            command: v(&["sudo", "pacman", "-S", "--needed", "--"]),
            requested: v(installed),
            installed: v(installed),
            still_missing: vec![],
            result: ExecResult::Ok,
            action: Action::Install,
            undoes: None,
        }
    }

    fn remocao(undoes: u32) -> HistoryEntry {
        let mut e = instalacao(&[]);
        e.command = v(&["sudo", "pacman", "-Rs", "--", "sl"]);
        e.requested = v(&["sl"]);
        e.still_missing = v(&["sl"]);
        e.action = Action::Remove;
        e.undoes = Some(undoes);
        e
    }

    /// Plano que o pacman devolveu na medição de 2026-11-14 (explícito + duas dependências órfãs).
    fn plano_medido() -> Capture {
        Capture {
            success: true,
            code: Some(0),
            stdout: "abi-dumper 1.4-2\nvtable-dumper 1.2-3\nctags 1:6.2.1-1\n".into(),
        }
    }

    struct Cenario {
        entradas: Vec<HistoryEntry>,
        n: u32,
        instalados: Vec<String>,
        explicitos: Vec<String>,
        interativo: bool,
        resposta: &'static str,
        simulacao: Capture,
        sudo_falha: bool,
        depois: Option<Vec<String>>,
    }

    fn cenario() -> Cenario {
        Cenario {
            entradas: vec![instalacao(&["abi-dumper"])],
            n: 1,
            instalados: v(&["abi-dumper", "ctags", "vtable-dumper"]),
            explicitos: v(&["abi-dumper"]),
            interativo: true,
            resposta: "s\n",
            simulacao: plano_medido(),
            sudo_falha: false,
            depois: Some(vec![]),
        }
    }

    struct Saida {
        resultado: anyhow::Result<Outcome>,
        texto: String,
        simulou: Vec<Vec<String>>,
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
            explicit: &c.explicitos,
            interactive: c.interativo,
            now_unix: 1_794_626_444,
        };
        let mut input = Cursor::new(c.resposta.as_bytes().to_vec());
        let mut output = Vec::new();
        let mut simulou = Vec::new();
        let mut executou = Vec::new();
        let resultado = undo(
            &req,
            &mut input,
            &mut output,
            |cmd| {
                simulou.push(cmd.to_vec());
                Ok(c.simulacao.clone())
            },
            |cmd| {
                executou.push(cmd.to_vec());
                if c.sudo_falha {
                    return Err(RunnerError::Failed {
                        program: "sudo".into(),
                        code: Some(1),
                    }
                    .into());
                }
                Ok(())
            },
            || {
                c.depois
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("pacman -Q falhou"))
            },
            &path,
        );
        Saida {
            resultado,
            texto: String::from_utf8(output).unwrap(),
            simulou,
            executou,
            arquivo_existe: path.exists(),
            gravadas: crate::history::read(&path).unwrap(),
        }
    }

    fn erro(s: &Saida) -> String {
        format!("{:#}", s.resultado.as_ref().unwrap_err())
    }

    // --- o caminho feliz, com o plano medido (ctags e vtable-dumper arrastados) ---

    #[test]
    fn fluxo_completo_simula_mostra_confirma_executa_e_grava() {
        let s = rodar(cenario());
        assert_eq!(s.resultado.as_ref().unwrap(), &Outcome::Executed);
        assert_eq!(
            s.simulou,
            vec![v(&[
                "pacman",
                "-Rs",
                "--print",
                "--print-format",
                "%n %v",
                "--",
                "abi-dumper"
            ])]
        );
        assert_eq!(
            s.executou,
            vec![v(&["sudo", "pacman", "-Rs", "--", "abi-dumper"])]
        );
        // O plano inteiro, com as dependências arrastadas, é a defesa contra histórico adulterado.
        for linha in ["abi-dumper 1.4-2", "vtable-dumper 1.2-3", "ctags 1:6.2.1-1"] {
            assert!(s.texto.contains(linha), "{linha} faltando em:\n{}", s.texto);
        }
        assert!(s.texto.contains("Comando: sudo pacman -Rs -- abi-dumper"));
        assert!(s.texto.contains("Executar? [s/N] "));
        assert_eq!(s.gravadas.len(), 1);
        let e = &s.gravadas[0];
        assert_eq!(e.action, Action::Remove);
        assert_eq!(e.undoes, Some(1));
        assert_eq!(e.requested, v(&["abi-dumper"]));
        assert!(e.installed.is_empty());
        assert_eq!(e.still_missing, v(&["abi-dumper"]));
        assert_eq!(e.result, ExecResult::Ok);
        assert_eq!(e.command, v(&["sudo", "pacman", "-Rs", "--", "abi-dumper"]));
        assert_eq!(e.at_unix, 1_794_626_444);
    }

    #[test]
    fn so_os_candidatos_vao_para_a_simulacao_e_os_ignorados_aparecem_na_tela() {
        let mut c = cenario();
        c.entradas = vec![instalacao(&["sl", "zsh", "cowsay"])];
        c.instalados = v(&["sl", "zsh"]);
        c.explicitos = v(&["sl"]);
        c.simulacao.stdout = "sl 1.0-1\n".into();
        let s = rodar(c);
        assert_eq!(s.simulou[0][s.simulou[0].len() - 1], "sl");
        assert_eq!(s.executou, vec![v(&["sudo", "pacman", "-Rs", "--", "sl"])]);
        assert!(
            s.texto
                .contains("ignorado: zsh (instalado como dependência, não explícito)"),
            "{}",
            s.texto
        );
        assert!(
            s.texto.contains("ignorado: cowsay (não está instalado)"),
            "{}",
            s.texto
        );
    }

    #[test]
    fn instalacao_que_falhou_mas_instalou_algo_pode_ser_desfeita() {
        let mut c = cenario();
        c.entradas[0].result = ExecResult::Failed { code: Some(1) };
        let s = rodar(c);
        assert_eq!(s.resultado.as_ref().unwrap(), &Outcome::Executed);
    }

    // --- nada a fazer: sem sudo, sem simulação, sem registro ---

    #[test]
    fn sem_candidatos_diz_nada_a_desfazer_e_nao_chama_nada() {
        let mut c = cenario();
        c.entradas = vec![instalacao(&["sl"])];
        c.instalados = v(&["zsh"]);
        c.explicitos = v(&["zsh"]);
        let s = rodar(c);
        assert_eq!(s.resultado.as_ref().unwrap(), &Outcome::NothingToDo);
        assert!(s.texto.contains("nada a desfazer"), "{}", s.texto);
        assert!(s.texto.contains("ignorado: sl (não está instalado)"));
        assert!(s.simulou.is_empty());
        assert!(s.executou.is_empty());
        assert!(!s.arquivo_existe);
    }

    // --- a simulação é obrigatória e manda: falhou, não há sudo ---

    #[test]
    fn simulacao_que_falha_para_sem_sudo_e_sem_registro() {
        let mut c = cenario();
        c.simulacao = Capture {
            success: false,
            code: Some(1),
            stdout: (1..=20).map(|i| format!("linha {i}\n")).collect(),
        };
        let s = rodar(c);
        let e = erro(&s);
        assert!(e.contains("simulação"), "{e}");
        assert!(e.contains("nada foi removido"), "{e}");
        assert!(s.executou.is_empty());
        assert!(!s.arquivo_existe);
        // A lista de dependências quebradas pode ter centenas de linhas (medido: 979 com `glibc`).
        assert!(s.texto.contains("linha 1\n"), "{}", s.texto);
        assert!(s.texto.contains("linha 5\n"), "{}", s.texto);
        assert!(!s.texto.contains("linha 6\n"), "{}", s.texto);
        assert!(s.texto.contains("+15 linhas"), "{}", s.texto);
    }

    #[test]
    fn plano_que_nao_cobre_um_candidato_para_sem_sudo() {
        let mut c = cenario();
        c.simulacao.stdout = "outro-pacote 1.0-1\n".into();
        let s = rodar(c);
        assert!(erro(&s).contains("abi-dumper"), "{}", erro(&s));
        assert!(s.executou.is_empty());
        assert!(!s.arquivo_existe);
    }

    #[test]
    fn plano_ilegivel_para_sem_sudo() {
        for lixo in ["", "lixo\n", "-Rdd 1.0-1\n", "sl 1.0 extra\n"] {
            let mut c = cenario();
            c.simulacao.stdout = lixo.into();
            let s = rodar(c);
            assert!(s.resultado.is_err(), "{lixo:?}");
            assert!(s.executou.is_empty(), "{lixo:?}");
            assert!(!s.arquivo_existe, "{lixo:?}");
        }
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
    fn sem_terminal_mostra_o_plano_mas_nao_executa_nem_grava() {
        let mut c = cenario();
        c.interativo = false;
        c.resposta = "s\n";
        let s = rodar(c);
        assert!(erro(&s).contains("terminal interativo"), "{}", erro(&s));
        assert!(s.texto.contains("ctags 1:6.2.1-1"));
        assert!(s.executou.is_empty());
        assert!(!s.arquivo_existe);
    }

    // --- falhas depois de confirmar: o registro mais importa nesse momento ---

    #[test]
    fn falha_do_sudo_e_gravada_com_o_codigo() {
        let mut c = cenario();
        c.sudo_falha = true;
        c.depois = Some(v(&["abi-dumper", "ctags", "vtable-dumper"]));
        let s = rodar(c);
        assert!(s.resultado.is_err());
        assert_eq!(s.gravadas.len(), 1);
        let e = &s.gravadas[0];
        assert_eq!(e.action, Action::Remove);
        assert_eq!(e.result, ExecResult::Failed { code: Some(1) });
        assert_eq!(e.installed, v(&["abi-dumper"]));
        assert!(e.still_missing.is_empty());
    }

    #[test]
    fn remocao_parcial_separa_o_que_ficou_do_que_saiu() {
        let mut c = cenario();
        c.entradas = vec![instalacao(&["sl", "zsh"])];
        c.instalados = v(&["sl", "zsh"]);
        c.explicitos = v(&["sl", "zsh"]);
        c.simulacao.stdout = "sl 1.0-1\nzsh 5.9-1\n".into();
        c.depois = Some(v(&["zsh"]));
        let s = rodar(c);
        let e = &s.gravadas[0];
        assert_eq!(e.requested, v(&["sl", "zsh"]));
        assert_eq!(e.installed, v(&["zsh"]));
        assert_eq!(e.still_missing, v(&["sl"]));
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

    // --- escolha da entrada ---

    #[test]
    fn numero_invalido_ou_entrada_de_remocao_nao_chama_nada() {
        for (entradas, n, trecho) in [
            (vec![instalacao(&["sl"])], 0, "fora"),
            (vec![instalacao(&["sl"])], 2, "fora"),
            (vec![], 1, "fora"),
            (
                vec![instalacao(&["sl"]), remocao(1)],
                2,
                "não é uma instalação",
            ),
        ] {
            let mut c = cenario();
            c.entradas = entradas;
            c.n = n;
            let s = rodar(c);
            assert!(erro(&s).contains(trecho), "n={n}: {}", erro(&s));
            assert!(s.simulou.is_empty());
            assert!(s.executou.is_empty());
            assert!(!s.arquivo_existe);
        }
    }
}
