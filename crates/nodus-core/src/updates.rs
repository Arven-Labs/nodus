#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryError, ParseError};
    use crate::runner::{CommandRunner, RunnerError};
    use std::cell::RefCell;

    const CHECKUPDATES: &str = include_str!("../tests/fixtures/checkupdates.txt");
    const PACMAN_QU: &str = include_str!("../tests/fixtures/pacman_qu.txt");
    const FLATPAK_UPDATES: &str = include_str!("../tests/fixtures/flatpak_remote_ls_updates.tsv");
    const FLATPAK_COLUNAS: &str = "--columns=application,version,branch";

    fn ausente(programa: &str) -> RunnerError {
        RunnerError::NotFound {
            program: programa.into(),
        }
    }

    fn falhou(programa: &str, code: i32) -> RunnerError {
        RunnerError::Failed {
            program: programa.into(),
            code: Some(code),
        }
    }

    /// Runner falso: responde por programa e guarda as chamadas. Sem Flatpak por padrão.
    struct Falso {
        checkupdates: Result<String, RunnerError>,
        pacman: Result<String, RunnerError>,
        flatpak: Result<String, RunnerError>,
        chamadas: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Falso {
        fn com_checkupdates(resposta: Result<String, RunnerError>) -> Self {
            Self {
                checkupdates: resposta,
                pacman: Err(ausente("pacman")),
                flatpak: Err(ausente("flatpak")),
                chamadas: RefCell::new(vec![]),
            }
        }
        fn com_pacman(mut self, resposta: Result<String, RunnerError>) -> Self {
            self.pacman = resposta;
            self
        }
        fn com_flatpak(mut self, resposta: Result<String, RunnerError>) -> Self {
            self.flatpak = resposta;
            self
        }
        fn programas(&self) -> Vec<String> {
            self.chamadas
                .borrow()
                .iter()
                .map(|(p, _)| p.clone())
                .collect()
        }
    }

    impl CommandRunner for Falso {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError> {
            self.chamadas.borrow_mut().push((
                program.to_string(),
                args.iter().map(|a| a.to_string()).collect(),
            ));
            match program {
                "checkupdates" => self.checkupdates.clone(),
                "pacman" => self.pacman.clone(),
                "flatpak" => self.flatpak.clone(),
                outro => panic!("programa inesperado no teste: {outro}"),
            }
        }
    }

    fn pacote(nome: &str, velha: &str, nova: &str) -> PacmanUpdate {
        PacmanUpdate {
            name: nome.into(),
            old_version: velha.into(),
            new_version: nova.into(),
        }
    }

    // --- parse_pacman_updates ---

    #[test]
    fn pacman_amostra_real_do_checkupdates() {
        let lista = parse_pacman_updates(CHECKUPDATES).unwrap();
        assert_eq!(lista.len(), 6);
        assert_eq!(
            lista[0],
            pacote("alsa-card-profiles", "1:1.6.10-1", "1:1.6.11-1")
        );
    }

    #[test]
    fn pacman_amostra_real_do_pacman_qu() {
        let lista = parse_pacman_updates(PACMAN_QU).unwrap();
        assert_eq!(lista.len(), 6);
        assert_eq!(lista[1], pacote("bluez", "5.87-2", "5.88-1"));
    }

    #[test]
    fn pacman_saida_vazia_e_lista_vazia() {
        assert_eq!(parse_pacman_updates("").unwrap(), vec![]);
        assert_eq!(parse_pacman_updates("\n\n").unwrap(), vec![]);
    }

    #[test]
    fn pacman_linha_sem_seta_e_erro() {
        let erro = parse_pacman_updates("bash 5.2-1 5.3-1\n").unwrap_err();
        assert_eq!(
            erro,
            ParseError::InvalidLine {
                number: 1,
                content: "bash 5.2-1 5.3-1".into()
            }
        );
    }

    #[test]
    fn pacman_linha_com_campo_a_menos_ou_a_mais_e_erro() {
        assert!(parse_pacman_updates("bash 5.2-1 ->\n").is_err());
        assert!(parse_pacman_updates("bash 5.2-1 -> 5.3-1 extra\n").is_err());
        assert!(parse_pacman_updates("bash\n").is_err());
    }

    #[test]
    fn pacman_caractere_de_controle_e_erro() {
        assert!(parse_pacman_updates("ba\x1bsh 5.2-1 -> 5.3-1\n").is_err());
    }

    #[test]
    fn pacman_erro_aponta_a_linha_certa() {
        let erro = parse_pacman_updates("bash 5.2-1 -> 5.3-1\nlixo\n").unwrap_err();
        assert!(matches!(erro, ParseError::InvalidLine { number: 2, .. }));
    }

    // --- parse_flatpak_updates ---

    #[test]
    fn flatpak_amostra_real() {
        let lista = parse_flatpak_updates(FLATPAK_UPDATES).unwrap();
        assert_eq!(
            lista,
            vec![FlatpakUpdate {
                application: "org.gtk.Gtk3theme.Breeze".into(),
                version: Some("6.7.5".into()),
                branch: "3.22".into(),
            }]
        );
    }

    #[test]
    fn flatpak_versao_vazia_vira_none() {
        let lista = parse_flatpak_updates("org.exemplo.App\t\tstable\n").unwrap();
        assert_eq!(lista[0].version, None);
    }

    #[test]
    fn flatpak_saida_vazia_e_lista_vazia() {
        assert_eq!(parse_flatpak_updates("").unwrap(), vec![]);
    }

    #[test]
    fn flatpak_numero_de_colunas_errado_e_erro() {
        assert!(parse_flatpak_updates("org.exemplo.App\tstable\n").is_err());
        assert!(parse_flatpak_updates("org.exemplo.App\t1.0\tstable\tflathub\n").is_err());
    }

    #[test]
    fn flatpak_id_ou_ramo_vazio_e_erro() {
        assert!(parse_flatpak_updates("\t1.0\tstable\n").is_err());
        assert!(parse_flatpak_updates("org.exemplo.App\t1.0\t\n").is_err());
    }

    #[test]
    fn flatpak_caractere_de_controle_e_erro() {
        assert!(parse_flatpak_updates("org.ex\x07emplo.App\t1.0\tstable\n").is_err());
    }

    // --- collect_updates ---

    #[test]
    fn usa_checkupdates_sem_argumentos_e_nao_chama_pacman() {
        let runner = Falso::com_checkupdates(Ok(CHECKUPDATES.into()));
        let updates = collect_updates(&runner).unwrap();
        assert_eq!(updates.pacman_source, PacmanSource::Checkupdates);
        assert_eq!(updates.pacman.len(), 6);
        assert_eq!(runner.programas(), vec!["checkupdates", "flatpak"]);
        assert_eq!(runner.chamadas.borrow()[0].1, Vec::<String>::new());
    }

    #[test]
    fn checkupdates_codigo_2_significa_sem_atualizacoes() {
        let runner = Falso::com_checkupdates(Err(falhou("checkupdates", 2)));
        let updates = collect_updates(&runner).unwrap();
        assert_eq!(updates.pacman_source, PacmanSource::Checkupdates);
        assert!(updates.pacman.is_empty());
        assert!(!runner.programas().contains(&"pacman".to_string()));
    }

    #[test]
    fn checkupdates_com_outro_codigo_e_erro_e_nao_cai_para_pacman() {
        let runner = Falso::com_checkupdates(Err(falhou("checkupdates", 1)));
        let erro = collect_updates(&runner).unwrap_err();
        assert_eq!(
            erro,
            InventoryError::Runner(falhou("checkupdates", 1))
        );
        assert!(!runner.programas().contains(&"pacman".to_string()));
    }

    #[test]
    fn sem_checkupdates_cai_para_pacman_qu_e_marca_a_fonte() {
        let runner = Falso::com_checkupdates(Err(ausente("checkupdates")))
            .com_pacman(Ok(PACMAN_QU.into()));
        let updates = collect_updates(&runner).unwrap();
        assert_eq!(updates.pacman_source, PacmanSource::PacmanQu);
        assert_eq!(updates.pacman.len(), 6);
        assert_eq!(runner.chamadas.borrow()[1].0, "pacman");
        assert_eq!(runner.chamadas.borrow()[1].1, vec!["-Qu".to_string()]);
    }

    #[test]
    fn pacman_qu_codigo_1_significa_sem_atualizacoes() {
        let runner = Falso::com_checkupdates(Err(ausente("checkupdates")))
            .com_pacman(Err(falhou("pacman", 1)));
        let updates = collect_updates(&runner).unwrap();
        assert_eq!(updates.pacman_source, PacmanSource::PacmanQu);
        assert!(updates.pacman.is_empty());
    }

    #[test]
    fn pacman_qu_com_outro_codigo_e_erro() {
        let runner = Falso::com_checkupdates(Err(ausente("checkupdates")))
            .com_pacman(Err(falhou("pacman", 2)));
        assert!(collect_updates(&runner).is_err());
    }

    #[test]
    fn sem_checkupdates_e_sem_pacman_e_erro() {
        let runner = Falso::com_checkupdates(Err(ausente("checkupdates")));
        assert_eq!(
            collect_updates(&runner).unwrap_err(),
            InventoryError::Runner(ausente("pacman"))
        );
    }

    #[test]
    fn saida_invalida_do_checkupdates_e_erro_de_parse() {
        let runner = Falso::com_checkupdates(Ok("lixo\n".into()));
        assert!(matches!(
            collect_updates(&runner).unwrap_err(),
            InventoryError::Parse(_)
        ));
    }

    #[test]
    fn flatpak_ausente_vira_none() {
        let runner = Falso::com_checkupdates(Ok(CHECKUPDATES.into()));
        assert_eq!(collect_updates(&runner).unwrap().flatpak, None);
    }

    #[test]
    fn flatpak_presente_e_chamado_com_colunas_fixas() {
        let runner = Falso::com_checkupdates(Ok(CHECKUPDATES.into()))
            .com_flatpak(Ok(FLATPAK_UPDATES.into()));
        let updates = collect_updates(&runner).unwrap();
        assert_eq!(updates.flatpak.as_ref().map(Vec::len), Some(1));
        let chamadas = runner.chamadas.borrow();
        let (programa, args) = chamadas.last().unwrap();
        assert_eq!(programa, "flatpak");
        assert_eq!(
            args,
            &vec![
                "remote-ls".to_string(),
                "--updates".to_string(),
                FLATPAK_COLUNAS.to_string()
            ]
        );
    }

    #[test]
    fn flatpak_sem_atualizacoes_e_some_vazio_e_nao_none() {
        let runner =
            Falso::com_checkupdates(Ok(CHECKUPDATES.into())).com_flatpak(Ok(String::new()));
        assert_eq!(collect_updates(&runner).unwrap().flatpak, Some(vec![]));
    }

    #[test]
    fn flatpak_com_falha_real_e_erro() {
        let runner = Falso::com_checkupdates(Ok(CHECKUPDATES.into()))
            .com_flatpak(Err(falhou("flatpak", 1)));
        assert_eq!(
            collect_updates(&runner).unwrap_err(),
            InventoryError::Runner(falhou("flatpak", 1))
        );
    }

    #[test]
    fn fonte_do_pacman_serializa_em_kebab_case() {
        assert_eq!(
            serde_json::to_string(&PacmanSource::Checkupdates).unwrap(),
            "\"checkupdates\""
        );
        assert_eq!(
            serde_json::to_string(&PacmanSource::PacmanQu).unwrap(),
            "\"pacman-qu\""
        );
    }
}
