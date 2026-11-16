#[cfg(test)]
mod tests {
    //! `dotfiles link` (ADR 0024): o fluxo inteiro (plano, pergunta, pastas, symlink, histórico) com
    //! entrada e saída injetadas. O HOME começa vazio e a pasta-fonte já tem os arquivos, como numa
    //! máquina nova com o repo de dotfiles clonado.
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::history::{Action, Manager};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    struct Cenario {
        tmp: TmpDir,
        home: PathBuf,
        dir: PathBuf,
        history: PathBuf,
    }

    fn cenario() -> Cenario {
        let tmp = TmpDir::novo();
        let home = tmp.path().join("home");
        let dir = tmp.path().join("dot");
        let state = tmp.path().join("state");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(dir.join(".config/git")).unwrap();
        fs::write(dir.join(".bashrc"), b"alias x=y\n").unwrap();
        fs::write(dir.join(".config/git/config"), b"[user]\n").unwrap();
        Cenario {
            history: state.join("nodus/history.jsonl"),
            tmp,
            home,
            dir,
        }
    }

    fn pedido<'a>(c: &'a Cenario, path: &'a str, interactive: bool) -> Request<'a> {
        Request {
            home: &c.home,
            dir: &c.dir,
            path,
            interactive,
            history_path: &c.history,
            now_unix: 1_700_000_500,
        }
    }

    fn roda(
        c: &Cenario,
        path: &str,
        interactive: bool,
        resposta: &str,
    ) -> (Result<Outcome>, String) {
        let mut saida = Vec::new();
        let resultado = link(
            &pedido(c, path, interactive),
            &mut resposta.as_bytes(),
            &mut saida,
        );
        (resultado, String::from_utf8(saida).unwrap())
    }

    fn home_vazio(c: &Cenario) {
        let nomes: Vec<_> = fs::read_dir(&c.home).unwrap().collect();
        assert!(nomes.is_empty(), "o HOME devia estar vazio: {nomes:?}");
        assert!(!c.history.exists());
    }

    fn fonte_intacta(c: &Cenario) {
        let meta = fs::symlink_metadata(c.dir.join(".bashrc")).unwrap();
        assert!(meta.file_type().is_file());
        assert_eq!(fs::read(c.dir.join(".bashrc")).unwrap(), b"alias x=y\n");
    }

    #[test]
    fn confirmando_com_s_liga_e_grava_uma_linha_v4_link() {
        let c = cenario();
        let (resultado, saida) = roda(&c, ".bashrc", true, "s\n");

        assert_eq!(resultado.unwrap(), Outcome::Linked);
        assert_eq!(
            fs::read_link(c.home.join(".bashrc")).unwrap(),
            c.dir.join(".bashrc")
        );
        assert_eq!(fs::read(c.home.join(".bashrc")).unwrap(), b"alias x=y\n");
        assert!(saida.contains("Executar? [s/N]"), "{saida}");
        assert!(saida.contains("Ligado:"), "{saida}");
        fonte_intacta(&c);

        let historico = crate::history::read(&c.history).unwrap();
        assert_eq!(historico.len(), 1);
        let e = &historico[0];
        assert_eq!(e.version, 4);
        assert_eq!(e.action, Action::Link);
        assert_eq!(e.manager, Manager::Dotfiles);
        assert_eq!(e.requested, [".bashrc"]);
        assert_eq!(e.installed, [".bashrc"]);
        assert_eq!(e.dir.as_deref(), c.dir.to_str());
        assert_eq!(e.at_unix, 1_700_000_500);
    }

    #[test]
    fn o_plano_mostra_destino_alvo_tamanho_e_o_aviso_de_execucao_de_codigo() {
        let c = cenario();
        let (_, saida) = roda(&c, ".bashrc", true, "n\n");
        assert!(
            saida.contains("Ligar dotfile: .bashrc (10 bytes"),
            "{saida}"
        );
        assert!(
            saida.contains(&format!(
                "criar symlink: {} -> {}",
                c.home.join(".bashrc").display(),
                c.dir.join(".bashrc").display()
            )),
            "{saida}"
        );
        assert!(saida.contains("confia"), "{saida}");
        assert!(saida.contains("nunca sobrescreve"), "{saida}");
        assert!(!saida.contains("criar pastas"), "{saida}");
    }

    #[test]
    fn caminho_aninhado_cria_as_pastas_do_home_com_0700_e_avisa_no_plano() {
        let c = cenario();
        let (resultado, saida) = roda(&c, ".config/git/config", true, "s\n");

        assert_eq!(resultado.unwrap(), Outcome::Linked);
        assert!(saida.contains("criar pastas"), "{saida}");
        for pasta in [".config", ".config/git"] {
            let meta = fs::symlink_metadata(c.home.join(pasta)).unwrap();
            assert!(meta.is_dir(), "{pasta}");
            assert_eq!(meta.permissions().mode() & 0o777, 0o700, "{pasta}");
        }
        assert_eq!(
            fs::read_link(c.home.join(".config/git/config")).unwrap(),
            c.dir.join(".config/git/config")
        );
    }

    #[test]
    fn pasta_que_ja_existe_no_home_e_reaproveitada_sem_mexer_no_resto() {
        let c = cenario();
        fs::create_dir_all(c.home.join(".config")).unwrap();
        fs::set_permissions(c.home.join(".config"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(c.home.join(".config/outro"), b"meu\n").unwrap();

        roda(&c, ".config/git/config", true, "s\n").0.unwrap();

        assert_eq!(fs::read(c.home.join(".config/outro")).unwrap(), b"meu\n");
        let modo = fs::metadata(c.home.join(".config")).unwrap().permissions();
        assert_eq!(modo.mode() & 0o777, 0o755);
    }

    #[test]
    fn qualquer_resposta_que_nao_seja_s_ou_sim_nao_cria_nada_nem_pasta() {
        for resposta in ["n\n", "\n", "", "talvez\n", "y\n", "ss\n"] {
            let c = cenario();
            let (resultado, saida) = roda(&c, ".config/git/config", true, resposta);
            assert_eq!(resultado.unwrap(), Outcome::Declined, "{resposta:?}");
            assert!(saida.contains("Nada foi alterado."), "{saida}");
            home_vazio(&c);
        }
    }

    #[test]
    fn sem_terminal_recusa_depois_de_mostrar_o_plano_e_sem_criar_nada() {
        let c = cenario();
        let (resultado, saida) = roda(&c, ".config/git/config", false, "s\n");
        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("terminal"), "{erro}");
        assert!(!saida.contains("Executar?"), "{saida}");
        home_vazio(&c);
    }

    #[test]
    fn destino_que_existe_e_recusado_antes_da_pergunta_e_nada_muda() {
        // arquivo comum (o `.bashrc` padrão da distro), pasta e symlink para outro lugar
        let preparos: [(&str, fn(&Cenario)); 3] = [
            ("arquivo", |c| {
                fs::write(c.home.join(".bashrc"), b"da distro\n").unwrap()
            }),
            ("pasta", |c| fs::create_dir(c.home.join(".bashrc")).unwrap()),
            ("symlink", |c| {
                symlink("/etc/profile", c.home.join(".bashrc")).unwrap()
            }),
        ];
        for (nome, prepara) in preparos {
            let c = cenario();
            prepara(&c);
            let (resultado, saida) = roda(&c, ".bashrc", true, "s\n");
            assert!(resultado.is_err(), "{nome}");
            assert!(!saida.contains("Executar?"), "{nome}: {saida}");
            assert!(!c.history.exists(), "{nome}");
            fonte_intacta(&c);
        }
        let c = cenario();
        fs::write(c.home.join(".bashrc"), b"da distro\n").unwrap();
        roda(&c, ".bashrc", true, "s\n").0.unwrap_err();
        assert_eq!(fs::read(c.home.join(".bashrc")).unwrap(), b"da distro\n");
    }

    #[test]
    fn ligar_duas_vezes_diz_que_ja_esta_ligado_e_nao_grava_de_novo() {
        let c = cenario();
        roda(&c, ".bashrc", true, "s\n").0.unwrap();
        let (resultado, saida) = roda(&c, ".bashrc", true, "s\n");
        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("já está ligado"), "{erro}");
        assert!(!saida.contains("Executar?"), "{saida}");
        assert_eq!(crate::history::read(&c.history).unwrap().len(), 1);
    }

    #[test]
    fn fonte_ausente_pasta_symlink_ou_acima_de_1_mib_e_recusada() {
        let preparos: [(&str, fn(&Cenario)); 4] = [
            ("ausente", |c| {
                fs::remove_file(c.dir.join(".bashrc")).unwrap()
            }),
            ("pasta", |c| {
                fs::remove_file(c.dir.join(".bashrc")).unwrap();
                fs::create_dir(c.dir.join(".bashrc")).unwrap();
            }),
            ("symlink", |c| {
                fs::remove_file(c.dir.join(".bashrc")).unwrap();
                symlink("/etc/passwd", c.dir.join(".bashrc")).unwrap();
            }),
            ("grande", |c| {
                fs::write(c.dir.join(".bashrc"), vec![b'a'; (1 << 20) + 1]).unwrap()
            }),
        ];
        for (nome, prepara) in preparos {
            let c = cenario();
            prepara(&c);
            let (resultado, saida) = roda(&c, ".bashrc", true, "s\n");
            assert!(resultado.is_err(), "{nome}");
            assert!(!saida.contains("Executar?"), "{nome}: {saida}");
            home_vazio(&c);
        }
    }

    #[test]
    fn fonte_ausente_diz_que_e_a_fonte() {
        let c = cenario();
        fs::remove_file(c.dir.join(".bashrc")).unwrap();
        let erro = format!("{:#}", roda(&c, ".bashrc", true, "s\n").0.unwrap_err());
        assert!(erro.contains("fonte"), "{erro}");
    }

    #[test]
    fn pasta_do_home_que_e_symlink_ou_arquivo_e_recusada_e_nada_e_criado_fora() {
        let c = cenario();
        let fora = c.tmp.path().join("fora");
        fs::create_dir_all(&fora).unwrap();
        symlink(&fora, c.home.join(".config")).unwrap();
        let (resultado, saida) = roda(&c, ".config/git/config", true, "s\n");
        assert!(resultado.is_err());
        assert!(!saida.contains("Executar?"), "{saida}");
        assert_eq!(fs::read_dir(&fora).unwrap().count(), 0, "nada fora do HOME");

        let c = cenario();
        fs::write(c.home.join(".config"), b"sou arquivo\n").unwrap();
        let (resultado, saida) = roda(&c, ".config/git/config", true, "s\n");
        assert!(resultado.is_err());
        assert!(!saida.contains("Executar?"), "{saida}");
        assert_eq!(fs::read(c.home.join(".config")).unwrap(), b"sou arquivo\n");
    }

    #[test]
    fn pasta_da_fonte_que_e_symlink_e_recusada() {
        let c = cenario();
        let fora = c.tmp.path().join("fora");
        fs::create_dir_all(fora.join("git")).unwrap();
        fs::write(fora.join("git/config"), b"fora\n").unwrap();
        fs::remove_dir_all(c.dir.join(".config")).unwrap();
        symlink(&fora, c.dir.join(".config")).unwrap();
        let (resultado, saida) = roda(&c, ".config/git/config", true, "s\n");
        assert!(resultado.is_err());
        assert!(!saida.contains("Executar?"), "{saida}");
        home_vazio(&c);
    }

    #[test]
    fn caminhos_invalidos_e_de_segredo_sao_recusados_antes_da_pergunta() {
        for path in [".ssh/config", "../x", "/etc/passwd", ""] {
            let c = cenario();
            fs::create_dir_all(c.dir.join(".ssh")).unwrap();
            fs::write(c.dir.join(".ssh/config"), b"segredo\n").unwrap();
            let (resultado, saida) = roda(&c, path, true, "s\n");
            assert!(resultado.is_err(), "{path:?}");
            assert!(!saida.contains("Executar?"), "{path:?}: {saida}");
            home_vazio(&c);
        }
    }

    #[test]
    fn pasta_fonte_dentro_de_um_caminho_de_segredo_e_recusada() {
        let c = cenario();
        let dentro = c.home.join(".ssh/dot");
        fs::create_dir_all(&dentro).unwrap();
        fs::write(dentro.join(".bashrc"), b"x\n").unwrap();
        let pedido = Request {
            dir: &dentro,
            ..pedido(&c, ".bashrc", true)
        };
        let mut saida = Vec::new();
        let erro = link(&pedido, &mut "s\n".as_bytes(), &mut saida).unwrap_err();
        assert!(format!("{erro:#}").contains("pasta-fonte"), "{erro:#}");
        assert!(!String::from_utf8(saida).unwrap().contains("Executar?"));
        assert!(fs::symlink_metadata(c.home.join(".bashrc")).is_err());
    }

    #[test]
    fn nao_deixa_temporario_nem_mexe_na_fonte() {
        let c = cenario();
        roda(&c, ".bashrc", true, "s\n").0.unwrap();
        let nomes: Vec<String> = fs::read_dir(&c.home)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(nomes, [".bashrc"], "{nomes:?}");
        fonte_intacta(&c);
    }

    #[test]
    fn falha_ao_gravar_o_historico_diz_que_a_ligacao_ja_foi_feita() {
        let c = cenario();
        let pasta = c.history.parent().unwrap().to_path_buf();
        fs::create_dir_all(&pasta).unwrap();
        fs::set_permissions(&pasta, fs::Permissions::from_mode(0o500)).unwrap();
        // Como root a escrita passaria; nesse caso não há o que testar.
        if fs::write(pasta.join("probe"), b"").is_ok() {
            fs::remove_file(pasta.join("probe")).unwrap();
            fs::set_permissions(&pasta, fs::Permissions::from_mode(0o700)).unwrap();
            return;
        }
        let (resultado, _) = roda(&c, ".bashrc", true, "s\n");
        fs::set_permissions(&pasta, fs::Permissions::from_mode(0o700)).unwrap();

        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("a ligação foi feita"), "{erro}");
        assert!(
            fs::symlink_metadata(c.home.join(".bashrc"))
                .unwrap()
                .is_symlink()
        );
    }
}
