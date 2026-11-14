#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::Plan;

    fn plano(faltando: &[&str]) -> Plan {
        Plan {
            pacman_missing: v(faltando),
            pacman_present: 3,
            flatpak_missing: vec![],
            flatpak_present: 0,
            flatpak_installed: true,
        }
    }

    fn v(itens: &[&str]) -> Vec<String> {
        itens.iter().map(|s| s.to_string()).collect()
    }

    fn comando(pacotes: &[&str]) -> Vec<String> {
        let mut c = v(&["sudo", "pacman", "-S", "--needed", "--"]);
        c.extend(v(pacotes));
        c
    }

    // --- entry_from: o registro guarda o estado real, não o código de saída ---

    #[test]
    fn entry_from_separa_instalados_e_faltantes() {
        let e = entry_from(
            &plano(&["sl", "cowsay"]),
            &plano(&["cowsay"]),
            &comando(&["sl", "cowsay"]),
            ExecResult::Failed { code: Some(1) },
            1_700_000_000,
        );
        assert_eq!(e.version, 1);
        assert_eq!(e.at_unix, 1_700_000_000);
        assert_eq!(e.command, comando(&["sl", "cowsay"]));
        assert_eq!(e.requested, v(&["sl", "cowsay"]));
        assert_eq!(e.installed, v(&["sl"]));
        assert_eq!(e.still_missing, v(&["cowsay"]));
        assert_eq!(e.result, ExecResult::Failed { code: Some(1) });
    }

    #[test]
    fn entry_from_tudo_instalado() {
        let e = entry_from(
            &plano(&["sl", "zsh"]),
            &plano(&[]),
            &comando(&["sl", "zsh"]),
            ExecResult::Ok,
            0,
        );
        assert_eq!(e.installed, v(&["sl", "zsh"]));
        assert!(e.still_missing.is_empty());
    }

    #[test]
    fn entry_from_nada_instalado() {
        let e = entry_from(
            &plano(&["sl"]),
            &plano(&["sl"]),
            &comando(&["sl"]),
            ExecResult::Failed { code: None },
            0,
        );
        assert!(e.installed.is_empty());
        assert_eq!(e.still_missing, v(&["sl"]));
    }

    #[test]
    fn entry_from_so_considera_o_que_foi_pedido() {
        // "outro" não estava em `requested`: não pode aparecer como faltante nem instalado.
        let e = entry_from(
            &plano(&["sl"]),
            &plano(&["outro"]),
            &comando(&["sl"]),
            ExecResult::Ok,
            0,
        );
        assert_eq!(e.requested, v(&["sl"]));
        assert_eq!(e.installed, v(&["sl"]));
        assert!(e.still_missing.is_empty());
    }

    // --- formato da linha (contrato de armazenamento) ---

    #[test]
    fn linha_tem_campos_e_ordem_estaveis() {
        let e = entry_from(
            &plano(&["sl"]),
            &plano(&[]),
            &comando(&["sl"]),
            ExecResult::Ok,
            1_700_000_000,
        );
        assert_eq!(
            to_line(&e),
            r#"{"version":1,"at_unix":1700000000,"command":["sudo","pacman","-S","--needed","--","sl"],"requested":["sl"],"installed":["sl"],"still_missing":[],"result":{"status":"ok"}}"#
        );
    }

    #[test]
    fn resultado_de_falha_leva_o_codigo() {
        let e = entry_from(
            &plano(&["sl"]),
            &plano(&["sl"]),
            &comando(&["sl"]),
            ExecResult::Failed { code: Some(1) },
            5,
        );
        assert!(to_line(&e).ends_with(r#""result":{"status":"failed","code":1}}"#));
    }

    #[test]
    fn linha_nunca_tem_quebra_de_linha() {
        let mut e = entry_from(
            &plano(&["sl"]),
            &plano(&[]),
            &comando(&["sl"]),
            ExecResult::Ok,
            0,
        );
        e.command.push("a\nb".into());
        assert!(!to_line(&e).contains('\n'));
    }

    #[test]
    fn ida_e_volta() {
        for resultado in [
            ExecResult::Ok,
            ExecResult::Failed { code: Some(1) },
            ExecResult::Failed { code: None },
        ] {
            let e = entry_from(
                &plano(&["sl", "cowsay"]),
                &plano(&["cowsay"]),
                &comando(&["sl", "cowsay"]),
                resultado,
                1_794_626_444,
            );
            assert_eq!(parse_line(&to_line(&e)).unwrap(), e);
        }
    }

    // --- leitura é entrada externa (T5): recusa em vez de adivinhar ---

    fn linha_valida() -> String {
        to_line(&entry_from(
            &plano(&["sl"]),
            &plano(&[]),
            &comando(&["sl"]),
            ExecResult::Ok,
            1,
        ))
    }

    #[test]
    fn recusa_linha_vazia_e_lixo() {
        for lixo in ["", "   ", "nao e json", "{}", "[]", "null"] {
            assert!(parse_line(lixo).is_err(), "{lixo:?} deveria ser recusado");
        }
    }

    #[test]
    fn recusa_campo_desconhecido() {
        let linha = linha_valida().replacen('{', r#"{"extra":1,"#, 1);
        assert!(parse_line(&linha).is_err());
    }

    #[test]
    fn recusa_campo_faltando() {
        let linha = linha_valida().replace(r#""still_missing":[],"#, "");
        assert!(parse_line(&linha).is_err());
    }

    #[test]
    fn recusa_versao_diferente() {
        let linha = linha_valida().replacen(r#""version":1"#, r#""version":2"#, 1);
        assert_eq!(
            parse_line(&linha).unwrap_err(),
            HistoryError::UnsupportedVersion(2)
        );
    }

    #[test]
    fn recusa_nome_de_pacote_invalido_em_qualquer_lista() {
        // Escape de terminal, opção do pacman, maiúscula e vazio: o nome é impresso na tela.
        for ruim in ["\u{1b}[31mx", "-Rns", "Zsh", "", "a b"] {
            let base = entry_from(
                &plano(&["sl"]),
                &plano(&[]),
                &comando(&["sl"]),
                ExecResult::Ok,
                1,
            );
            let mut em_requested = base.clone();
            em_requested.requested = v(&[ruim]);
            let mut em_installed = base.clone();
            em_installed.installed = v(&[ruim]);
            let mut em_faltando = base.clone();
            em_faltando.still_missing = v(&[ruim]);
            for (campo, e) in [
                ("requested", em_requested),
                ("installed", em_installed),
                ("still_missing", em_faltando),
            ] {
                assert!(
                    parse_line(&to_line(&e)).is_err(),
                    "{ruim:?} em {campo} deveria ser recusado"
                );
            }
        }
    }

    // --- data legível (UTC, sem crate de datas) ---

    #[test]
    fn format_utc_na_epoca() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn format_utc_datas_conhecidas() {
        assert_eq!(format_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(format_utc(1_794_626_444), "2026-11-14T03:20:44Z");
    }

    #[test]
    fn format_utc_dia_bissexto_e_virada_de_seculo() {
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_utc(4_102_444_799), "2099-12-31T23:59:59Z");
        assert_eq!(format_utc(4_102_444_800), "2100-01-01T00:00:00Z");
    }
}
