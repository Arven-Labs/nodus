//! Preset declarativo de pacotes (fatia 1: só leitura). O preset é entrada não confiável (T1, T7).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{FlatpakApp, Package};
    use crate::runner::{CommandRunner, RunnerError};

    fn pacote(nome: &str) -> Package {
        Package {
            name: nome.into(),
            version: "1.0-1".into(),
        }
    }

    fn app(id: &str) -> FlatpakApp {
        FlatpakApp {
            application: id.into(),
            version: None,
            branch: "stable".into(),
            origin: "flathub".into(),
            installation: "user".into(),
        }
    }

    fn preset(pacman: &[&str], flatpak: &[&str]) -> Preset {
        parse_preset(&format!(
            "version = 1\npacman = {pacman:?}\nflatpak = {flatpak:?}\n"
        ))
        .unwrap()
    }

    // --- parse_preset ---

    #[test]
    fn le_um_preset_valido() {
        let p = parse_preset(
            "# meu preset\nversion = 1\npacman = [\"base\", \"git\"]\nflatpak = [\"org.nongnu.lingot\"]\n",
        )
        .unwrap();
        assert_eq!(p.pacman, ["base", "git"]);
        assert_eq!(p.flatpak, ["org.nongnu.lingot"]);
    }

    #[test]
    fn listas_ausentes_viram_vazias() {
        let p = parse_preset("version = 1\n").unwrap();
        assert!(p.pacman.is_empty());
        assert!(p.flatpak.is_empty());
    }

    #[test]
    fn version_ausente_e_erro() {
        assert!(matches!(
            parse_preset("pacman = [\"git\"]\n"),
            Err(PresetError::Toml(_))
        ));
    }

    #[test]
    fn version_diferente_de_1_e_erro() {
        assert_eq!(
            parse_preset("version = 2\n"),
            Err(PresetError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn campo_desconhecido_e_erro() {
        assert!(matches!(
            parse_preset("version = 1\npacmna = [\"git\"]\n"),
            Err(PresetError::Toml(_))
        ));
    }

    #[test]
    fn toml_quebrado_e_erro() {
        assert!(matches!(
            parse_preset("version = 1\npacman = [\"git\"\n"),
            Err(PresetError::Toml(_))
        ));
    }

    #[test]
    fn nomes_de_pacman_aceitos() {
        let nomes = [
            "7zip",
            "lib32-glibc",
            "gtk+3",
            "python3.14",
            "foo@bar",
            "a_b",
        ];
        assert_eq!(preset(&nomes, &[]).pacman, nomes);
    }

    #[test]
    fn nome_de_pacman_que_vira_opcao_ou_injecao_e_recusado() {
        let recusados = [
            "-Rns",
            "--overwrite=*",
            ".oculto",
            "a;b",
            "a b",
            "Git",
            "",
            "a\nb",
            "a/b",
        ];
        for nome in recusados {
            let toml = format!("version = 1\npacman = [\"ok\", {nome:?}]\n");
            assert_eq!(
                parse_preset(&toml),
                Err(PresetError::InvalidName {
                    list: "pacman",
                    number: 2,
                    name: nome.to_string(),
                }),
                "{nome:?} deveria ser recusado"
            );
        }
    }

    #[test]
    fn ids_de_flatpak_aceitos() {
        let ids = ["org.nongnu.lingot", "com.github.foo_bar.App-1", "io.x"];
        assert_eq!(preset(&[], &ids).flatpak, ids);
    }

    #[test]
    fn id_de_flatpak_invalido_e_recusado() {
        let recusados = [
            "lingot", "org..x", ".org.x", "org.x.", "-org.x", "org.-x", "org.x;y", "org.x y",
            "org/x.y", "",
        ];
        for id in recusados {
            let toml = format!("version = 1\nflatpak = [{id:?}]\n");
            assert_eq!(
                parse_preset(&toml),
                Err(PresetError::InvalidName {
                    list: "flatpak",
                    number: 1,
                    name: id.to_string(),
                }),
                "{id:?} deveria ser recusado"
            );
        }
    }

    #[test]
    fn nome_repetido_e_erro() {
        assert_eq!(
            parse_preset("version = 1\npacman = [\"git\", \"vim\", \"git\"]\n"),
            Err(PresetError::Duplicate {
                list: "pacman",
                name: "git".into(),
            })
        );
        assert_eq!(
            parse_preset("version = 1\nflatpak = [\"org.a.B\", \"org.a.B\"]\n"),
            Err(PresetError::Duplicate {
                list: "flatpak",
                name: "org.a.B".into(),
            })
        );
    }

    // --- to_toml ---

    #[test]
    fn to_toml_tem_um_nome_por_linha_e_volta_igual() {
        let p = preset(&["base", "git"], &["org.nongnu.lingot"]);
        let texto = to_toml(&p);
        assert_eq!(
            texto,
            "# Preset do nodus (formato 1). Gerado por `nodus preset init`; edite à vontade.\nversion = 1\npacman = [\n    \"base\",\n    \"git\",\n]\nflatpak = [\n    \"org.nongnu.lingot\",\n]\n"
        );
        assert_eq!(parse_preset(&texto).unwrap(), p);
    }

    #[test]
    fn to_toml_com_listas_vazias_volta_igual() {
        let p = preset(&[], &[]);
        assert_eq!(parse_preset(&to_toml(&p)).unwrap(), p);
    }

    // --- plan ---

    #[test]
    fn plano_vazio_quando_tudo_ja_esta_instalado() {
        let p = preset(&["git", "vim"], &["org.a.B"]);
        let instalados = [pacote("git"), pacote("vim"), pacote("glibc")];
        let plano = plan(&p, &instalados, Some(&[app("org.a.B")]));
        assert_eq!(
            plano,
            Plan {
                pacman_missing: vec![],
                pacman_present: 2,
                flatpak_missing: vec![],
                flatpak_present: 1,
                flatpak_installed: true,
            }
        );
        assert!(plano.is_empty());
    }

    #[test]
    fn plano_lista_so_o_que_falta_na_ordem_do_preset() {
        let p = preset(&["zsh", "git", "neovim"], &["org.a.B", "org.c.D"]);
        let plano = plan(&p, &[pacote("git")], Some(&[app("org.c.D")]));
        assert_eq!(plano.pacman_missing, ["zsh", "neovim"]);
        assert_eq!(plano.pacman_present, 1);
        assert_eq!(plano.flatpak_missing, ["org.a.B"]);
        assert_eq!(plano.flatpak_present, 1);
        assert!(!plano.is_empty());
    }

    #[test]
    fn pacote_instalado_fora_do_preset_nao_entra_no_plano() {
        let p = preset(&["git"], &[]);
        let instalados = [pacote("git"), pacote("steam")];
        let plano = plan(&p, &instalados, Some(&[app("org.x.Y")]));
        assert!(plano.is_empty());
    }

    #[test]
    fn sem_flatpak_instalado_todos_os_apps_do_preset_faltam() {
        let p = preset(&[], &["org.a.B"]);
        let plano = plan(&p, &[], None);
        assert!(!plano.flatpak_installed);
        assert_eq!(plano.flatpak_missing, ["org.a.B"]);
        assert_eq!(plano.flatpak_present, 0);
    }

    #[test]
    fn plano_serializa_em_snake_case() {
        let plano = plan(&preset(&["git"], &[]), &[], Some(&[]));
        let json = serde_json::to_value(&plano).unwrap();
        assert_eq!(json["pacman_missing"], serde_json::json!(["git"]));
        assert_eq!(json["flatpak_installed"], serde_json::json!(true));
    }

    // --- leitura da máquina ---

    /// Responde por programa e primeiro argumento; registra cada chamada.
    struct Fake {
        pacman_qe: Result<String, RunnerError>,
        pacman_q: Result<String, RunnerError>,
        flatpak: Result<String, RunnerError>,
        chamadas: std::cell::RefCell<Vec<Vec<String>>>,
    }

    impl Fake {
        fn novo() -> Self {
            Fake {
                pacman_qe: Ok("git 2.56.0-1\nvim 9.1-1\n".into()),
                pacman_q: Ok("git 2.56.0-1\nglibc 2.42-1\nvim 9.1-1\n".into()),
                flatpak: Ok("org.nongnu.lingot\t1.1.1\tstable\tflathub\tuser\n".into()),
                chamadas: Default::default(),
            }
        }

        /// Houve uma chamada exatamente com estes programa e argumentos?
        fn chamou(&self, comando: &[&str]) -> bool {
            self.chamadas.borrow().iter().any(|c| c[..] == *comando)
        }
    }

    impl CommandRunner for Fake {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError> {
            let mut chamada = vec![program.to_string()];
            chamada.extend(args.iter().map(|a| a.to_string()));
            self.chamadas.borrow_mut().push(chamada);
            match (program, args.first().copied()) {
                ("pacman", Some("-Qe")) => self.pacman_qe.clone(),
                ("pacman", Some("-Q")) => self.pacman_q.clone(),
                ("flatpak", Some("list")) => self.flatpak.clone(),
                _ => panic!("chamada inesperada: {program} {args:?}"),
            }
        }
    }

    #[test]
    fn preset_da_maquina_usa_so_os_pacotes_explicitos() {
        let r = Fake::novo();
        let m = preset_from_machine(&r).unwrap();
        assert_eq!(m.preset.pacman, ["git", "vim"]);
        assert_eq!(m.preset.flatpak, ["org.nongnu.lingot"]);
        assert!(m.flatpak_installed);
        assert!(r.chamou(&["pacman", "-Qe"]));
        assert!(!r.chamou(&["pacman", "-Q"]));
    }

    #[test]
    fn preset_da_maquina_sem_flatpak_fica_sem_apps_e_avisa() {
        let mut r = Fake::novo();
        r.flatpak = Err(RunnerError::NotFound {
            program: "flatpak".into(),
        });
        let m = preset_from_machine(&r).unwrap();
        assert!(m.preset.flatpak.is_empty());
        assert!(!m.flatpak_installed);
    }

    #[test]
    fn preset_da_maquina_com_flatpak_sem_apps_nao_e_o_mesmo_que_sem_flatpak() {
        let mut r = Fake::novo();
        r.flatpak = Ok(String::new());
        let m = preset_from_machine(&r).unwrap();
        assert!(m.preset.flatpak.is_empty());
        assert!(m.flatpak_installed);
    }

    #[test]
    fn preset_da_maquina_recusa_nome_invalido_vindo_do_pacman() {
        let mut r = Fake::novo();
        r.pacman_qe = Ok("git 2.56.0-1\n-Rns 1.0-1\n".into());
        assert!(matches!(
            preset_from_machine(&r),
            Err(MachineError::Preset(PresetError::InvalidName { .. }))
        ));
    }

    #[test]
    fn init_seguido_de_plan_na_mesma_maquina_da_plano_vazio() {
        let r = Fake::novo();
        let p = parse_preset(&to_toml(&preset_from_machine(&r).unwrap().preset)).unwrap();
        assert!(collect_plan(&r, &p).unwrap().is_empty());
    }

    #[test]
    fn plan_da_maquina_considera_todos_os_pacotes_instalados() {
        // `glibc` veio como dependência (não está no -Qe), mas está instalado: não pode entrar no plano.
        let r = Fake::novo();
        let plano = collect_plan(&r, &preset(&["glibc", "zsh"], &[])).unwrap();
        assert_eq!(plano.pacman_missing, ["zsh"]);
        assert!(r.chamou(&["pacman", "-Q"]));
    }

    #[test]
    fn plan_da_maquina_sem_flatpak_marca_nao_instalado() {
        let mut r = Fake::novo();
        r.flatpak = Err(RunnerError::NotFound {
            program: "flatpak".into(),
        });
        let plano = collect_plan(&r, &preset(&[], &["org.a.B"])).unwrap();
        assert!(!plano.flatpak_installed);
    }

    #[test]
    fn falha_do_pacman_vira_erro() {
        let mut r = Fake::novo();
        r.pacman_q = Err(RunnerError::Failed {
            program: "pacman".into(),
            code: Some(1),
        });
        assert!(collect_plan(&r, &preset(&["git"], &[])).is_err());
    }
}
