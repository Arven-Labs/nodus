#[cfg(test)]
mod tests {
    use super::*;
    use crate::dotfiles::observe;
    use crate::testutil::TmpDir;
    use nodus_core::dotfiles::{AdoptPlan, plan_adopt};
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::path::{Path, PathBuf};

    /// HOME e pasta-fonte lado a lado numa pasta temporária (o `$HOME` real nunca é tocado).
    struct Cenario {
        _tmp: TmpDir,
        home: PathBuf,
        dir: PathBuf,
    }

    fn cenario() -> Cenario {
        let tmp = TmpDir::novo();
        let home = tmp.path().join("home");
        let dir = tmp.path().join("dot");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&dir).unwrap();
        Cenario {
            _tmp: tmp,
            home,
            dir,
        }
    }

    fn escreve(path: &Path, conteudo: &[u8], modo: u32) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, conteudo).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(modo)).unwrap();
    }

    /// O plano calculado do jeito que o CLI faz: observa o disco e passa pelo core.
    fn plano(c: &Cenario, path: &str) -> AdoptPlan {
        let observed = observe(&c.home, path).unwrap();
        let modo = fs::symlink_metadata(c.home.join(path)).unwrap().mode();
        plan_adopt(&c.home, &c.dir, path, &observed, modo).unwrap()
    }

    fn nada() -> impl FnOnce(&Path) {
        |_| {}
    }

    fn nada2() -> impl FnOnce() {
        || {}
    }

    /// Nomes dentro de uma pasta, ordenados.
    fn nomes(dir: &Path) -> Vec<String> {
        let mut n: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        n.sort();
        n
    }

    fn erro_de(r: Result<()>) -> String {
        format!("{:#}", r.unwrap_err())
    }

    /// O original continua um arquivo comum, com o conteúdo de antes.
    fn original_intacto(c: &Cenario, path: &str, conteudo: &[u8]) {
        let meta = fs::symlink_metadata(c.home.join(path)).unwrap();
        assert!(
            meta.file_type().is_file(),
            "o original deixou de ser arquivo"
        );
        assert_eq!(fs::read(c.home.join(path)).unwrap(), conteudo);
    }

    // ---- caminho feliz ----

    #[test]
    fn adota_copia_confere_e_troca_o_original_por_symlink() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"alias x=y\n", 0o644);
        let plan = plano(&c, ".bashrc");

        adopt_file(&c.home, &plan, nada(), nada2()).unwrap();

        let dest = c.dir.join(".bashrc");
        assert_eq!(fs::read(&dest).unwrap(), b"alias x=y\n");
        let meta = fs::symlink_metadata(c.home.join(".bashrc")).unwrap();
        assert!(meta.file_type().is_symlink());
        assert_eq!(fs::read_link(c.home.join(".bashrc")).unwrap(), dest);
        // lendo pelo link, o conteúdo é o mesmo
        assert_eq!(fs::read(c.home.join(".bashrc")).unwrap(), b"alias x=y\n");
    }

    #[test]
    fn nao_deixa_temporario_no_home_nem_na_pasta_fonte() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"x", 0o644);
        let plan = plano(&c, ".bashrc");
        adopt_file(&c.home, &plan, nada(), nada2()).unwrap();
        assert_eq!(nomes(&c.home), [".bashrc"]);
        assert_eq!(nomes(&c.dir), [".bashrc"]);
    }

    #[test]
    fn caminho_aninhado_cria_as_pastas_na_fonte_e_troca_so_o_arquivo() {
        let c = cenario();
        escreve(&c.home.join(".config/git/config"), b"[user]\n", 0o600);
        let plan = plano(&c, ".config/git/config");

        adopt_file(&c.home, &plan, nada(), nada2()).unwrap();

        let dest = c.dir.join(".config/git/config");
        assert_eq!(fs::read(&dest).unwrap(), b"[user]\n");
        assert_eq!(
            fs::read_link(c.home.join(".config/git/config")).unwrap(),
            dest
        );
        // a pasta `.config/git` do HOME continua pasta de verdade
        assert!(
            fs::symlink_metadata(c.home.join(".config/git"))
                .unwrap()
                .is_dir()
        );
    }

    #[test]
    fn preserva_so_os_9_bits_de_permissao_na_copia() {
        for modo in [0o600, 0o644, 0o755] {
            let c = cenario();
            escreve(&c.home.join(".bashrc"), b"x", modo);
            let plan = plano(&c, ".bashrc");
            adopt_file(&c.home, &plan, nada(), nada2()).unwrap();
            let copiado = fs::metadata(c.dir.join(".bashrc")).unwrap().mode() & 0o7777;
            assert_eq!(copiado, modo, "{modo:o}");
        }
    }

    #[test]
    fn copia_conteudo_binario_vazio_e_do_tamanho_maximo() {
        let grande: Vec<u8> = (0..nodus_core::dotfiles::MAX_ADOPT_BYTES)
            .map(|i| (i % 251) as u8)
            .collect();
        for conteudo in [vec![], vec![0u8, 255, 10, 13, 0], grande] {
            let c = cenario();
            escreve(&c.home.join(".bashrc"), &conteudo, 0o644);
            let plan = plano(&c, ".bashrc");
            adopt_file(&c.home, &plan, nada(), nada2()).unwrap();
            assert_eq!(fs::read(c.dir.join(".bashrc")).unwrap(), conteudo);
        }
    }

    // ---- recusas: o original fica intacto e nada novo sobra ----

    #[test]
    fn destino_existente_e_recusado_sem_sobrescrever_nada() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"do home", 0o644);
        escreve(&c.dir.join(".bashrc"), b"ja estava na fonte", 0o644);
        let plan = plano(&c, ".bashrc");

        let erro = erro_de(adopt_file(&c.home, &plan, nada(), nada2()));

        assert!(erro.contains("já existe"), "{erro}");
        original_intacto(&c, ".bashrc", b"do home");
        assert_eq!(
            fs::read(c.dir.join(".bashrc")).unwrap(),
            b"ja estava na fonte"
        );
    }

    #[test]
    fn original_que_mudou_depois_do_plano_e_recusado_antes_de_copiar() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"curto", 0o644);
        let plan = plano(&c, ".bashrc");
        fs::write(c.home.join(".bashrc"), b"agora esta maior").unwrap();

        let erro = erro_de(adopt_file(&c.home, &plan, nada(), nada2()));

        assert!(erro.contains("mudou"), "{erro}");
        original_intacto(&c, ".bashrc", b"agora esta maior");
        assert!(nomes(&c.dir).is_empty());
    }

    #[test]
    fn original_que_mudou_entre_a_copia_e_a_troca_nao_e_trocado_e_a_copia_some() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"versao 1", 0o644);
        let plan = plano(&c, ".bashrc");
        let alvo = c.home.join(".bashrc");

        let erro = erro_de(adopt_file(&c.home, &plan, nada(), || {
            // O dono salvou o arquivo no editor bem nesse instante. O tamanho muda de propósito:
            // uma regravação do mesmo tamanho no mesmo tick do relógio não muda o `mtime` e a
            // checagem não a vê (limite declarado no ADR 0022).
            fs::write(&alvo, b"versao 2, bem maior que a anterior").unwrap();
        }));

        assert!(erro.contains("mudou"), "{erro}");
        original_intacto(&c, ".bashrc", b"versao 2, bem maior que a anterior");
        assert!(
            nomes(&c.dir).is_empty(),
            "sobrou cópia: {:?}",
            nomes(&c.dir)
        );
        assert_eq!(nomes(&c.home), [".bashrc"]);
    }

    #[test]
    fn original_trocado_por_symlink_entre_a_copia_e_a_troca_e_recusado() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"x", 0o644);
        escreve(&c.home.join("outro"), b"y", 0o644);
        let plan = plano(&c, ".bashrc");
        let alvo = c.home.join(".bashrc");
        let outro = c.home.join("outro");

        let erro = erro_de(adopt_file(&c.home, &plan, nada(), || {
            fs::remove_file(&alvo).unwrap();
            symlink(&outro, &alvo).unwrap();
        }));

        assert!(erro.contains("mudou"), "{erro}");
        // o que o dono colocou lá continua lá
        assert_eq!(fs::read_link(&alvo).unwrap(), outro);
        assert!(nomes(&c.dir).is_empty());
    }

    #[test]
    fn copia_que_nao_confere_byte_a_byte_aborta_e_some() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"conteudo bom", 0o644);
        let plan = plano(&c, ".bashrc");

        let erro = erro_de(adopt_file(
            &c.home,
            &plan,
            |dest| fs::write(dest, b"conteudo ruim").unwrap(),
            nada2(),
        ));

        assert!(erro.contains("não confere"), "{erro}");
        original_intacto(&c, ".bashrc", b"conteudo bom");
        assert!(nomes(&c.dir).is_empty());
    }

    #[test]
    fn copia_truncada_com_o_mesmo_prefixo_tambem_nao_confere() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"abcdef", 0o644);
        let plan = plano(&c, ".bashrc");

        let erro = erro_de(adopt_file(
            &c.home,
            &plan,
            |dest| fs::write(dest, b"abc").unwrap(),
            nada2(),
        ));

        assert!(erro.contains("não confere"), "{erro}");
        original_intacto(&c, ".bashrc", b"abcdef");
    }

    #[test]
    fn pai_do_original_que_e_symlink_e_recusado() {
        let c = cenario();
        let fora = c.home.parent().unwrap().join("fora");
        escreve(&fora.join("git/config"), b"x", 0o644);
        symlink(&fora, c.home.join(".config")).unwrap();
        // o plano é calculado como o CLI faz; `observe` segue o pai, e é por isso que o
        // `adopt_file` confere os pais de novo
        let plan = plano(&c, ".config/git/config");

        let erro = erro_de(adopt_file(&c.home, &plan, nada(), nada2()));

        assert!(erro.contains("symlink"), "{erro}");
        assert_eq!(fs::read(fora.join("git/config")).unwrap(), b"x");
        assert!(nomes(&c.dir).is_empty());
    }

    #[test]
    fn destino_que_escapa_da_pasta_fonte_por_symlink_e_recusado() {
        let c = cenario();
        escreve(&c.home.join(".config/git/config"), b"x", 0o644);
        let fora = c.home.parent().unwrap().join("fora");
        fs::create_dir_all(&fora).unwrap();
        symlink(&fora, c.dir.join(".config")).unwrap();
        let plan = plano(&c, ".config/git/config");

        let erro = erro_de(adopt_file(&c.home, &plan, nada(), nada2()));

        assert!(erro.contains("escapa"), "{erro}");
        original_intacto(&c, ".config/git/config", b"x");
        assert!(nomes(&fora).is_empty(), "escreveu fora da pasta-fonte");
    }

    #[test]
    fn sem_permissao_na_pasta_fonte_o_original_fica_intacto_e_nada_sobra() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"x", 0o644);
        let plan = plano(&c, ".bashrc");
        fs::set_permissions(&c.dir, fs::Permissions::from_mode(0o500)).unwrap();
        // Como root a escrita passaria; nesse caso não há o que testar.
        let probe = c.dir.join("probe");
        if fs::write(&probe, b"").is_ok() {
            fs::remove_file(&probe).unwrap();
            fs::set_permissions(&c.dir, fs::Permissions::from_mode(0o700)).unwrap();
            return;
        }

        let resultado = adopt_file(&c.home, &plan, nada(), nada2());
        fs::set_permissions(&c.dir, fs::Permissions::from_mode(0o700)).unwrap();

        assert!(resultado.is_err());
        original_intacto(&c, ".bashrc", b"x");
        assert!(nomes(&c.dir).is_empty());
        assert_eq!(nomes(&c.home), [".bashrc"]);
    }

    #[test]
    fn adotar_de_novo_recusa_porque_o_original_ja_e_symlink() {
        let c = cenario();
        escreve(&c.home.join(".bashrc"), b"x", 0o644);
        let plan = plano(&c, ".bashrc");
        adopt_file(&c.home, &plan, nada(), nada2()).unwrap();

        let erro = erro_de(adopt_file(&c.home, &plan, nada(), nada2()));

        assert!(
            erro.contains("mudou") || erro.contains("já existe"),
            "{erro}"
        );
        assert_eq!(fs::read(c.dir.join(".bashrc")).unwrap(), b"x");
        assert!(
            fs::symlink_metadata(c.home.join(".bashrc"))
                .unwrap()
                .is_symlink()
        );
    }
}
