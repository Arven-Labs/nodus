#[cfg(test)]
mod tests {
    //! Gravar o snapshot na pasta-fonte dos dotfiles (ADR 0031, fatia 1). Tudo em pasta temporária:
    //! nunca toca o HOME real.
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::inventory::OsInfo;
    use nodus_core::preset::{MachinePreset, Preset};
    use nodus_core::snapshot::{Snapshot, build, parse, to_json};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::Path;

    fn snap(nome: &str, pacman: &[&str]) -> Snapshot {
        let os = OsInfo {
            name: "Arch Linux".into(),
            version: None,
        };
        let maquina = MachinePreset {
            preset: Preset {
                pacman: pacman.iter().map(|s| s.to_string()).collect(),
                flatpak: vec![],
            },
            flatpak_installed: false,
        };
        build(nome, &os, &maquina).unwrap()
    }

    fn modo(p: &Path) -> u32 {
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    fn quantos(p: &Path) -> usize {
        fs::read_dir(p).unwrap().count()
    }

    #[test]
    fn grava_em_dot_nodus_snapshots_com_o_conteudo_exato_e_permissoes_fechadas() {
        let tmp = TmpDir::novo();
        let s = snap("meu-pc", &["git", "base"]);
        let caminho = write(tmp.path(), &s).unwrap();
        assert_eq!(caminho, tmp.path().join(".nodus/snapshots/meu-pc.json"));
        assert_eq!(fs::read_to_string(&caminho).unwrap(), to_json(&s));
        // Arquivo do dono, sem leitura para o grupo.
        assert_eq!(modo(&caminho), 0o600);
        assert_eq!(modo(&tmp.path().join(".nodus")), 0o700);
        assert_eq!(modo(&tmp.path().join(".nodus/snapshots")), 0o700);
    }

    #[test]
    fn gravar_duas_vezes_sem_mudanca_da_os_mesmos_bytes() {
        let tmp = TmpDir::novo();
        let s = snap("meu-pc", &["git", "base"]);
        let caminho = write(tmp.path(), &s).unwrap();
        let primeira = fs::read(&caminho).unwrap();
        write(tmp.path(), &s).unwrap();
        assert_eq!(fs::read(&caminho).unwrap(), primeira);
    }

    #[test]
    fn regravar_com_mudanca_substitui_e_nao_deixa_temporario() {
        let tmp = TmpDir::novo();
        write(tmp.path(), &snap("meu-pc", &["base"])).unwrap();
        let novo = snap("meu-pc", &["base", "vim"]);
        let caminho = write(tmp.path(), &novo).unwrap();
        assert_eq!(fs::read_to_string(&caminho).unwrap(), to_json(&novo));
        let nomes: Vec<_> = fs::read_dir(tmp.path().join(".nodus/snapshots"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(nomes, ["meu-pc.json"], "nenhum temporário sobra");
    }

    #[test]
    fn dois_rotulos_dao_dois_arquivos_e_um_nao_toca_o_outro() {
        let tmp = TmpDir::novo();
        let a = write(tmp.path(), &snap("pc-a", &["base"])).unwrap();
        let antes = fs::read(&a).unwrap();
        write(tmp.path(), &snap("pc-b", &["base", "vim"])).unwrap();
        assert_eq!(fs::read(&a).unwrap(), antes);
        assert!(tmp.path().join(".nodus/snapshots/pc-b.json").exists());
    }

    #[test]
    fn pasta_fonte_que_nao_existe_ou_nao_e_pasta_e_recusada_sem_criar_nada() {
        let tmp = TmpDir::novo();
        let inexistente = tmp.path().join("nao-existe");
        assert!(write(&inexistente, &snap("meu-pc", &["base"])).is_err());
        // Não cria a pasta-fonte por conta própria.
        assert!(!inexistente.exists());
        let arquivo = tmp.path().join("arquivo");
        fs::write(&arquivo, "x").unwrap();
        assert!(write(&arquivo, &snap("meu-pc", &["base"])).is_err());
    }

    #[test]
    fn recusa_dot_nodus_ou_snapshots_que_seja_symlink_e_nada_vai_para_fora() {
        let fora = TmpDir::novo();
        // `.nodus` apontando para fora.
        let tmp = TmpDir::novo();
        symlink(fora.path(), tmp.path().join(".nodus")).unwrap();
        assert!(write(tmp.path(), &snap("meu-pc", &["base"])).is_err());
        assert_eq!(quantos(fora.path()), 0, "nada vazou pelo symlink");
        // `.nodus/snapshots` apontando para fora.
        let tmp2 = TmpDir::novo();
        fs::create_dir(tmp2.path().join(".nodus")).unwrap();
        symlink(fora.path(), tmp2.path().join(".nodus/snapshots")).unwrap();
        assert!(write(tmp2.path(), &snap("meu-pc", &["base"])).is_err());
        assert_eq!(quantos(fora.path()), 0);
    }

    #[test]
    fn recusa_destino_que_seja_symlink_e_nao_escreve_no_alvo() {
        let tmp = TmpDir::novo();
        let alvo = tmp.path().join("segredo.txt");
        fs::write(&alvo, "intacto").unwrap();
        fs::create_dir_all(tmp.path().join(".nodus/snapshots")).unwrap();
        symlink(&alvo, tmp.path().join(".nodus/snapshots/meu-pc.json")).unwrap();
        assert!(write(tmp.path(), &snap("meu-pc", &["base"])).is_err());
        assert_eq!(fs::read_to_string(&alvo).unwrap(), "intacto");
    }

    #[test]
    fn recusa_destino_que_seja_pasta() {
        let tmp = TmpDir::novo();
        fs::create_dir_all(tmp.path().join(".nodus/snapshots/meu-pc.json")).unwrap();
        assert!(write(tmp.path(), &snap("meu-pc", &["base"])).is_err());
    }

    #[test]
    fn o_arquivo_gravado_e_lido_de_volta_pela_leitura_estrita() {
        let tmp = TmpDir::novo();
        let s = snap("meu-pc", &["git", "base"]);
        let caminho = write(tmp.path(), &s).unwrap();
        assert_eq!(parse(&fs::read_to_string(caminho).unwrap()).unwrap(), s);
    }
}
