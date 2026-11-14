#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{Action, ExecResult, HistoryEntry, parse_line, to_line};

    fn v(itens: &[&str]) -> Vec<String> {
        itens.iter().map(|s| s.to_string()).collect()
    }

    fn instalacao(installed: &[&str]) -> HistoryEntry {
        let mut comando = v(&["sudo", "pacman", "-S", "--needed", "--"]);
        comando.extend(v(installed));
        HistoryEntry {
            version: 2,
            at_unix: 1_700_000_000,
            command: comando,
            requested: v(installed),
            installed: v(installed),
            still_missing: vec![],
            result: ExecResult::Ok,
            action: Action::Install,
            undoes: None,
        }
    }

    fn remocao(undoes: u32) -> HistoryEntry {
        HistoryEntry {
            version: 2,
            at_unix: 1_700_000_100,
            command: v(&["sudo", "pacman", "-Rs", "--", "sl"]),
            requested: v(&["sl"]),
            installed: vec![],
            still_missing: v(&["sl"]),
            result: ExecResult::Ok,
            action: Action::Remove,
            undoes: Some(undoes),
        }
    }

    // --- candidatos: o que da entrada ainda pode sair (ADR 0011, item 2) ---

    #[test]
    fn candidato_e_o_que_segue_instalado_e_explicito() {
        let c = undo_candidates(&instalacao(&["sl"]), &v(&["sl", "zsh"]), &v(&["sl"]));
        assert_eq!(c.remove, v(&["sl"]));
        assert!(c.skipped.is_empty());
    }

    #[test]
    fn pacote_que_ja_nao_esta_instalado_e_ignorado() {
        let c = undo_candidates(&instalacao(&["sl"]), &v(&["zsh"]), &v(&["zsh"]));
        assert!(c.remove.is_empty());
        assert_eq!(
            c.skipped,
            vec![Skipped {
                name: "sl".into(),
                reason: SkipReason::NotInstalled
            }]
        );
    }

    #[test]
    fn pacote_que_virou_dependencia_nao_e_removido() {
        // Instalado, mas `pacman -Qe` não o lista: veio como dependência de outro.
        let c = undo_candidates(&instalacao(&["libfoo"]), &v(&["libfoo"]), &v(&[]));
        assert!(c.remove.is_empty());
        assert_eq!(c.skipped[0].reason, SkipReason::NotExplicit);
    }

    #[test]
    fn nao_instalado_tem_precedencia_sobre_nao_explicito() {
        let c = undo_candidates(&instalacao(&["sl"]), &v(&[]), &v(&[]));
        assert_eq!(c.skipped[0].reason, SkipReason::NotInstalled);
    }

    #[test]
    fn so_considera_o_campo_installed_da_entrada() {
        // `still_missing` nunca foi instalado pelo nodus: não pode virar alvo de remoção.
        let mut e = instalacao(&["sl"]);
        e.requested = v(&["sl", "zsh"]);
        e.still_missing = v(&["zsh"]);
        let c = undo_candidates(&e, &v(&["sl", "zsh"]), &v(&["sl", "zsh"]));
        assert_eq!(c.remove, v(&["sl"]));
        assert!(c.skipped.is_empty());
    }

    #[test]
    fn mantem_a_ordem_e_tira_duplicatas() {
        let mut e = instalacao(&["zsh", "sl", "zsh"]);
        e.requested = v(&["zsh", "sl"]);
        let c = undo_candidates(&e, &v(&["sl", "zsh"]), &v(&["sl", "zsh"]));
        assert_eq!(c.remove, v(&["zsh", "sl"]));
    }

    #[test]
    fn entrada_sem_instalados_nao_tem_candidatos() {
        let c = undo_candidates(&instalacao(&[]), &v(&["sl"]), &v(&["sl"]));
        assert!(c.remove.is_empty());
        assert!(c.skipped.is_empty());
    }

    #[test]
    fn motivos_de_ignorado_tem_texto_para_o_dono() {
        assert_eq!(SkipReason::NotInstalled.to_string(), "não está instalado");
        assert_eq!(
            SkipReason::NotExplicit.to_string(),
            "instalado como dependência, não explícito"
        );
    }

    // --- comandos: sempre lista de argumentos, `--` antes dos nomes, nunca as opções perigosas ---

    #[test]
    fn comando_de_simulacao_e_o_do_adr_0011() {
        assert_eq!(
            remove_print_command(&v(&["sl", "zsh"])),
            Some(v(&[
                "pacman",
                "-Rs",
                "--print",
                "--print-format",
                "%n %v",
                "--",
                "sl",
                "zsh"
            ]))
        );
    }

    #[test]
    fn comando_de_remocao_e_o_do_adr_0011() {
        assert_eq!(
            remove_command(&v(&["sl", "zsh"])),
            Some(v(&["sudo", "pacman", "-Rs", "--", "sl", "zsh"]))
        );
    }

    #[test]
    fn sem_candidatos_nao_ha_comando() {
        assert_eq!(remove_print_command(&[]), None);
        assert_eq!(remove_command(&[]), None);
    }

    #[test]
    fn comandos_nunca_levam_opcao_perigosa_e_os_nomes_ficam_depois_do_separador() {
        let proibidas = [
            "-n",
            "-c",
            "-d",
            "-dd",
            "-Rns",
            "-Rc",
            "-Rd",
            "-Rdd",
            "--nosave",
            "--cascade",
            "--nodeps",
            "--noconfirm",
        ];
        let conjuntos = [
            v(&["sl"]),
            v(&["sl", "zsh", "abi-dumper"]),
            v(&["a", "lib32-foo", "python-3to2", "gtk+3-x", "c++filt"]),
        ];
        for nomes in conjuntos {
            for comando in [
                remove_print_command(&nomes).unwrap(),
                remove_command(&nomes).unwrap(),
            ] {
                let sep = comando.iter().position(|a| a == "--").unwrap();
                assert_eq!(&comando[sep + 1..], nomes.as_slice(), "{comando:?}");
                for antes in &comando[..sep] {
                    assert!(
                        !proibidas.contains(&antes.as_str()),
                        "{antes} em {comando:?}"
                    );
                }
            }
        }
    }

    // --- plano da simulação: saída do pacman é entrada externa (`%n %v`, medida em 2026-11-14) ---

    #[test]
    fn le_o_plano_medido_com_dependencias_e_epoca_na_versao() {
        let plano =
            parse_remove_plan("abi-dumper 1.4-2\nvtable-dumper 1.2-3\nctags 1:6.2.1-1\n").unwrap();
        assert_eq!(
            plano,
            vec![
                Removal {
                    name: "abi-dumper".into(),
                    version: "1.4-2".into()
                },
                Removal {
                    name: "vtable-dumper".into(),
                    version: "1.2-3".into()
                },
                Removal {
                    name: "ctags".into(),
                    version: "1:6.2.1-1".into()
                },
            ]
        );
    }

    #[test]
    fn plano_vazio_e_erro() {
        for vazio in ["", "\n", "  \n"] {
            assert_eq!(
                parse_remove_plan(vazio),
                Err(UndoError::EmptyPlan),
                "{vazio:?}"
            );
        }
    }

    #[test]
    fn linha_do_plano_fora_do_formato_e_erro() {
        for ruim in [
            "so-nome\n",
            "a 1.0 extra\n",
            "a 1.0-1\n\nb 2.0-1\n",
            "error: target not found: x\n",
        ] {
            assert!(
                matches!(parse_remove_plan(ruim), Err(UndoError::InvalidPlanLine(_))),
                "{ruim:?}"
            );
        }
    }

    #[test]
    fn nome_ou_versao_do_plano_com_escape_ou_opcao_e_erro() {
        for ruim in [
            "-Rdd 1.0-1\n",
            "Zsh 1.0-1\n",
            "\u{1b}[31mx 1.0-1\n",
            "sl \u{1b}[31m1.0\n",
            "sl 1.0;rm\n",
            "sl $(id)\n",
        ] {
            assert!(
                matches!(parse_remove_plan(ruim), Err(UndoError::InvalidPlanLine(_))),
                "{ruim:?}"
            );
        }
    }

    #[test]
    fn plano_precisa_conter_todos_os_candidatos() {
        let plano = parse_remove_plan("sl 1.0-1\nzsh 5.9-1\n").unwrap();
        assert_eq!(check_plan_covers(&v(&["sl", "zsh"]), &plano), Ok(()));
        assert_eq!(
            check_plan_covers(&v(&["sl", "cowsay"]), &plano),
            Err(UndoError::MissingFromPlan("cowsay".into()))
        );
    }

    // --- escolher a entrada pelo número mostrado em `nodus history` ---

    #[test]
    fn escolhe_pelo_numero_a_partir_de_1() {
        let entradas = vec![instalacao(&["sl"]), instalacao(&["zsh"])];
        assert_eq!(select_entry(&entradas, 1).unwrap().installed, v(&["sl"]));
        assert_eq!(select_entry(&entradas, 2).unwrap().installed, v(&["zsh"]));
    }

    #[test]
    fn numero_fora_do_intervalo_e_erro() {
        let entradas = vec![instalacao(&["sl"])];
        for n in [0, 2, 99] {
            assert_eq!(
                select_entry(&entradas, n).unwrap_err(),
                UndoError::OutOfRange { n, len: 1 }
            );
        }
        assert_eq!(
            select_entry(&[], 1).unwrap_err(),
            UndoError::OutOfRange { n: 1, len: 0 }
        );
    }

    #[test]
    fn nao_desfaz_uma_entrada_que_ja_e_remocao() {
        let entradas = vec![instalacao(&["sl"]), remocao(1)];
        assert_eq!(
            select_entry(&entradas, 2).unwrap_err(),
            UndoError::NotAnInstall(2)
        );
    }

    #[test]
    fn instalacao_que_falhou_mas_instalou_algo_pode_ser_desfeita() {
        let mut e = instalacao(&["sl"]);
        e.result = ExecResult::Failed { code: Some(1) };
        assert!(select_entry(&[e], 1).is_ok());
    }

    // --- a entrada nova que a reversão grava (sempre acrescenta, ADR 0011 item 6) ---

    #[test]
    fn entrada_da_reversao_separa_ainda_instalados_e_removidos() {
        let e = entry_for_undo(
            3,
            &v(&["sl", "zsh"]),
            &v(&["zsh"]),
            &v(&["sudo", "pacman", "-Rs", "--", "sl", "zsh"]),
            ExecResult::Failed { code: Some(1) },
            1_794_626_444,
        );
        assert_eq!(e.version, 2);
        assert_eq!(e.action, Action::Remove);
        assert_eq!(e.undoes, Some(3));
        assert_eq!(e.requested, v(&["sl", "zsh"]));
        assert_eq!(e.installed, v(&["zsh"]));
        assert_eq!(e.still_missing, v(&["sl"]));
        assert_eq!(e.result, ExecResult::Failed { code: Some(1) });
        assert_eq!(e.at_unix, 1_794_626_444);
    }

    #[test]
    fn entrada_da_reversao_volta_igual_pelo_parser() {
        let e = entry_for_undo(
            1,
            &v(&["sl"]),
            &v(&[]),
            &v(&["sudo", "pacman", "-Rs", "--", "sl"]),
            ExecResult::Ok,
            5,
        );
        assert_eq!(parse_line(&to_line(&e)).unwrap(), e);
    }
}
