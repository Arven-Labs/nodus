#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    // --- allowlist: a GUI só roda comandos de leitura, com argumentos fixos (ADR 0026) ---

    #[test]
    fn allowlist_aceita_so_inventory() {
        assert_eq!(Allowed::parse("inventory"), Some(Allowed::Inventory));
    }

    #[test]
    fn allowlist_recusa_tudo_o_que_nao_esta_na_lista() {
        for nome in [
            "", " ", "Inventory", "INVENTORY", "inventory ", " inventory", "inventory --json", "inventory;ls",
            "inventory\n", "inventory\0", "apply", "undo", "adopt", "link", "preset", "history", "workspace",
            "dotfiles", "--version", "--help", "../inventory", "/bin/sh", "-c", "sudo",
        ] {
            assert_eq!(Allowed::parse(nome), None, "{nome:?} não pode passar");
        }
    }

    #[test]
    fn argumentos_sao_fixos_e_terminam_em_json() {
        assert_eq!(Allowed::Inventory.args(), ["inventory", "--json"]);
    }

    // --- versão: `nodusctl --version` tem que ser o mesmo major.minor da GUI ---

    #[test]
    fn le_a_versao_do_nodusctl() {
        assert_eq!(parse_version("nodusctl 0.1.0\n"), Some((0, 1, 0)));
        assert_eq!(parse_version("nodusctl 12.34.56"), Some((12, 34, 56)));
    }

    #[test]
    fn versao_malformada_nao_e_lida() {
        for texto in [
            "", "nodusctl", "nodusctl 0.1", "nodusctl 0.1.0.0", "0.1.0", "nodus 0.1.0", "NODUSCTL 0.1.0",
            "nodusctl 0.1.0-dev", "nodusctl v0.1.0", "nodusctl 0.1.x", "nodusctl -1.0.0",
            "nodusctl 99999999999999999999.0.0", "nodusctl 0.1.0 extra", "hello",
        ] {
            assert_eq!(parse_version(texto), None, "{texto:?} não é uma versão válida");
        }
    }

    #[test]
    fn compativel_so_com_mesmo_major_e_minor() {
        assert!(compatible((0, 1, 0), (0, 1, 0)));
        assert!(compatible((0, 1, 0), (0, 1, 9)), "patch pode diferir");
        assert!(compatible((1, 2, 7), (1, 2, 0)));
        assert!(!compatible((0, 1, 0), (0, 2, 0)), "minor diferente quebra em 0.x");
        assert!(!compatible((0, 1, 0), (1, 1, 0)), "major diferente");
        assert!(!compatible((1, 2, 0), (1, 3, 0)));
        assert!(!compatible((1, 2, 0), (2, 2, 0)));
    }

    // --- `read`: roda o binário de verdade (um script de mentira) ---

    static CONTADOR: AtomicU32 = AtomicU32::new(0);

    /// Cria uma pasta temporária com um `nodusctl` de mentira. O script deixa um arquivo `chamadas` com uma linha por execução.
    fn falso(corpo: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "nodus-desktop-teste-{}-{}",
            std::process::id(),
            CONTADOR.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("nodusctl");
        fs::write(&bin, format!("#!/bin/sh\necho \"$*\" >> '{}/chamadas'\n{corpo}\n", dir.display())).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        (dir, bin)
    }

    fn chamadas(dir: &Path) -> Vec<String> {
        fs::read_to_string(dir.join("chamadas")).map(|t| t.lines().map(String::from).collect()).unwrap_or_default()
    }

    const BOM: &str = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.4"; exit 0; fi
if [ "$1" = "inventory" ] && [ "$2" = "--json" ]; then echo '{"ok":true}'; exit 0; fi
echo "argumentos inesperados: $*" >&2; exit 64
"#;

    #[test]
    fn le_o_json_do_inventario() {
        let (dir, bin) = falso(BOM);
        let saida = read(&bin, "inventory", (0, 1, 0)).unwrap();
        assert_eq!(saida.trim(), r#"{"ok":true}"#);
        assert_eq!(chamadas(&dir), ["--version", "inventory --json"], "versão primeiro, depois o comando, nada mais");
    }

    #[test]
    fn comando_fora_da_lista_nao_executa_nada() {
        let (dir, bin) = falso(BOM);
        for nome in ["apply", "inventory --json", "", "sudo", "undo 1"] {
            let erro = read(&bin, nome, (0, 1, 0)).unwrap_err();
            assert_eq!(erro.kind, "not_allowed", "{nome:?}");
        }
        assert!(chamadas(&dir).is_empty(), "o binário não pode nem ser executado");
    }

    #[test]
    fn versao_incompativel_bloqueia_e_nao_roda_o_comando() {
        let (dir, bin) = falso(&BOM.replace("0.1.4", "0.2.0"));
        let erro = read(&bin, "inventory", (0, 1, 0)).unwrap_err();
        assert_eq!(erro.kind, "version_mismatch");
        assert!(erro.message.contains("0.2.0"), "a mensagem diz qual versão veio: {}", erro.message);
        assert_eq!(chamadas(&dir), ["--version"], "só a versão foi pedida; o inventário nunca rodou");
    }

    #[test]
    fn major_diferente_tambem_bloqueia() {
        let (_dir, bin) = falso(&BOM.replace("0.1.4", "1.1.0"));
        assert_eq!(read(&bin, "inventory", (0, 1, 0)).unwrap_err().kind, "version_mismatch");
    }

    #[test]
    fn saida_de_versao_ilegivel_e_erro_proprio() {
        let (dir, bin) = falso("echo hello; exit 0");
        let erro = read(&bin, "inventory", (0, 1, 0)).unwrap_err();
        assert_eq!(erro.kind, "version_unknown");
        assert_eq!(chamadas(&dir), ["--version"]);
    }

    #[test]
    fn binario_inexistente_e_not_found() {
        let erro = read(Path::new("/nao/existe/nodusctl"), "inventory", (0, 1, 0)).unwrap_err();
        assert_eq!(erro.kind, "not_found");
    }

    #[test]
    fn comando_que_falha_devolve_o_erro_da_cli() {
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.0"; exit 0; fi
echo "falhou de propósito" >&2; exit 3
"#;
        let (_dir, bin) = falso(corpo);
        let erro = read(&bin, "inventory", (0, 1, 0)).unwrap_err();
        assert_eq!(erro.kind, "failed");
        assert!(erro.message.contains("falhou de propósito"), "{}", erro.message);
    }

    #[test]
    fn saida_gigante_e_recusada() {
        let corpo = r#"
if [ "$1" = "--version" ]; then echo "nodusctl 0.1.0"; exit 0; fi
head -c 20000000 /dev/zero | tr '\0' 'a'
"#;
        let (_dir, bin) = falso(corpo);
        assert_eq!(read(&bin, "inventory", (0, 1, 0)).unwrap_err().kind, "output_too_large");
    }

    #[test]
    fn nenhum_argumento_vem_do_chamador() {
        // O que o frontend manda é só o nome; qualquer tentativa de embutir argumento é recusada (teste acima) e
        // o conjunto de argumentos executados é exatamente o da allowlist.
        let (dir, bin) = falso(BOM);
        read(&bin, "inventory", (0, 1, 0)).unwrap();
        for linha in chamadas(&dir) {
            assert!(linha == "--version" || linha == "inventory --json", "chamada fora do esperado: {linha}");
        }
    }
}
