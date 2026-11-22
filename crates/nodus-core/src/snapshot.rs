#[cfg(test)]
mod tests {
    //! Snapshot da máquina (ADR 0031, fatia 1): formato v1 que vai para o repositório git do dono.
    //! Só o que o `compare` da fatia 2 precisa (SO, capacidades, pacotes de propósito, apps
    //! Flatpak); sem hora, versão de pacote, hardware ou conteúdo de arquivo.
    use super::*;
    use crate::inventory::OsInfo;
    use crate::preset::{MachinePreset, Preset};

    fn os() -> OsInfo {
        OsInfo {
            name: "Arch Linux".into(),
            version: None,
        }
    }

    fn maquina(pacman: &[&str], flatpak: &[&str], flatpak_installed: bool) -> MachinePreset {
        MachinePreset {
            preset: Preset {
                pacman: pacman.iter().map(|s| s.to_string()).collect(),
                flatpak: flatpak.iter().map(|s| s.to_string()).collect(),
            },
            flatpak_installed,
        }
    }

    fn montar(nome: &str, pacman: &[&str], flatpak: &[&str]) -> Result<Snapshot, SnapshotError> {
        build(nome, &os(), &maquina(pacman, flatpak, true))
    }

    #[test]
    fn build_ordena_as_listas_e_a_mesma_coisa_em_outra_ordem_da_os_mesmos_bytes() {
        let a = montar("meu-pc", &["vim", "git", "base"], &["org.b.B", "org.a.A"]).unwrap();
        let b = montar("meu-pc", &["base", "vim", "git"], &["org.a.A", "org.b.B"]).unwrap();
        assert_eq!(a.pacman, ["base", "git", "vim"]);
        assert_eq!(a.flatpak, ["org.a.A", "org.b.B"]);
        // O `git diff` só pode mostrar mudança real.
        assert_eq!(to_json(&a), to_json(&b));
    }

    #[test]
    fn o_formato_v1_e_exatamente_este() {
        let s = montar("meu-pc", &["git", "base"], &["org.a.A"]).unwrap();
        let esperado = r#"{
  "version": 1,
  "name": "meu-pc",
  "os": {
    "name": "Arch Linux",
    "version": null
  },
  "capabilities": {
    "pacman": true,
    "flatpak": true
  },
  "pacman": [
    "base",
    "git"
  ],
  "flatpak": [
    "org.a.A"
  ]
}
"#;
        assert_eq!(to_json(&s), esperado);
    }

    #[test]
    fn sem_flatpak_instalado_a_capacidade_e_falsa_e_a_lista_vazia() {
        let s = build("meu-pc", &os(), &maquina(&["base"], &[], false)).unwrap();
        assert!(s.capabilities.pacman);
        assert!(!s.capabilities.flatpak);
        assert!(s.flatpak.is_empty());
        // Flatpak instalado e sem apps é diferente de Flatpak ausente.
        let com = build("meu-pc", &os(), &maquina(&["base"], &[], true)).unwrap();
        assert!(com.capabilities.flatpak);
    }

    #[test]
    fn o_snapshot_nao_tem_hora_nem_versao_de_pacote_nem_hardware() {
        let json = to_json(&montar("meu-pc", &["base"], &[]).unwrap());
        let proibidos = [
            "at_unix", "time", "hardware", "cpu", "memory", "disk", "kernel", "hostname",
        ];
        for proibido in proibidos {
            assert!(!json.contains(proibido), "{proibido}");
        }
    }

    #[test]
    fn rotulo_valido_e_so_minusculas_digitos_ponto_traco_e_sublinhado() {
        let longo_ok = format!("a{}", "b".repeat(62));
        let validos = [
            "meu-pc",
            "a",
            "0",
            "notebook.2",
            "a_b-c.d",
            longo_ok.as_str(),
        ];
        for ok in validos {
            assert!(validate_name(ok).is_ok(), "{ok:?}");
        }
    }

    #[test]
    fn rotulo_que_escaparia_do_diretorio_ou_quebraria_o_nome_do_arquivo_e_recusado() {
        let longo_ruim = "a".repeat(64);
        let ruins = [
            "",
            "-x",
            ".x",
            "_x",
            "..",
            ".",
            "a/b",
            "../x",
            "/etc/passwd",
            "A",
            "Meu-PC",
            "a b",
            "a\0b",
            "a\nb",
            "é",
            "a:b",
            "a\\b",
            "a;b",
            "~",
            longo_ruim.as_str(),
        ];
        for ruim in ruins {
            let erro = validate_name(ruim);
            assert!(
                matches!(erro, Err(SnapshotError::InvalidName(_))),
                "{ruim:?}"
            );
        }
    }

    #[test]
    fn build_recusa_rotulo_invalido_e_nome_de_pacote_ou_app_invalido() {
        let erro = montar("../x", &["base"], &[]);
        assert!(matches!(erro, Err(SnapshotError::InvalidName(_))));
        // A máquina já valida, mas o `build` não confia: um `-Rns` no arquivo seria lido como
        // opção por quem usa a lista.
        let erro = montar("meu-pc", &["-Rns"], &[]);
        assert!(matches!(erro, Err(SnapshotError::InvalidItem { .. })));
        let erro = montar("meu-pc", &[], &["não é id"]);
        assert!(matches!(erro, Err(SnapshotError::InvalidItem { .. })));
    }

    #[test]
    fn build_recusa_item_duplicado() {
        let erro = montar("meu-pc", &["git", "git"], &[]);
        assert!(matches!(erro, Err(SnapshotError::Duplicate { .. })));
    }

    #[test]
    fn parse_devolve_o_que_to_json_escreveu() {
        let s = montar("meu-pc", &["git", "base"], &["org.a.A"]).unwrap();
        assert_eq!(parse(&to_json(&s)).unwrap(), s);
    }

    fn valido() -> String {
        to_json(&montar("meu-pc", &["base"], &["org.a.A"]).unwrap())
    }

    #[test]
    fn parse_recusa_versao_desconhecida_sem_adivinhar() {
        for v in ["0", "2", "99", "-1", "1.5", "\"1\"", "null"] {
            let texto = valido().replacen("\"version\": 1", &format!("\"version\": {v}"), 1);
            assert!(parse(&texto).is_err(), "versão {v}");
        }
        let texto = valido().replacen("\"version\": 1", "\"version\": 2", 1);
        assert_eq!(
            parse(&texto).unwrap_err(),
            SnapshotError::UnsupportedVersion(2)
        );
    }

    #[test]
    fn parse_recusa_campo_desconhecido_ausente_e_lixo() {
        let com_extra = valido().replacen("\"name\"", "\"extra\": 1,\n  \"name\"", 1);
        assert!(matches!(parse(&com_extra), Err(SnapshotError::Json(_))));
        let sem_os = valido().replacen("\"os\"", "\"os_\"", 1);
        assert!(parse(&sem_os).is_err());
        let nix = valido().replacen(
            "\"pacman\": true",
            "\"pacman\": true,\n    \"nix\": true",
            1,
        );
        assert!(parse(&nix).is_err(), "capacidade desconhecida");
        for lixo in ["", "não é json", "[]", "{}", "null", "{\"version\":1}"] {
            assert!(parse(lixo).is_err(), "{lixo:?}");
        }
    }

    #[test]
    fn parse_revalida_rotulo_e_itens_do_arquivo_de_outra_maquina() {
        // O arquivo vem de outra máquina (e pode ser editado à mão): a leitura valida como a escrita.
        let erro = parse(&valido().replacen("meu-pc", "../x", 1));
        assert!(matches!(erro, Err(SnapshotError::InvalidName(_))));
        let erro = parse(&valido().replacen("\"base\"", "\"-Rns\"", 1));
        assert!(matches!(erro, Err(SnapshotError::InvalidItem { .. })));
        let erro = parse(&valido().replacen("\"org.a.A\"", "\"x y\"", 1));
        assert!(matches!(erro, Err(SnapshotError::InvalidItem { .. })));
    }
}
