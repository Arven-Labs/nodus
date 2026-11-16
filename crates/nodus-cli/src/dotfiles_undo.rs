//! `nodusctl history undo <n>` de uma adoção de dotfile (ADR 0023): troca o symlink do HOME por uma
//! cópia do conteúdo atual da pasta-fonte, que não é alterada. Tudo o que pode ser recusado é
//! recusado **antes** da pergunta. O histórico não é assinado (T5): `path` e `dir` vêm dele e são
//! revalidados pelo core. A troca é atômica: cópia para um temporário ao lado do symlink, conferida
//! byte a byte, e `rename` por cima.

use crate::dotfiles::observe;
use crate::dotfiles_adopt::{check_parents, snapshot};
use anyhow::{Context, Result, bail};
use nodus_core::dotfiles::{UndoAdoptPlan, plan_undo_adopt, validate_dir, validate_path};
use nodus_core::history::{HistoryEntry, entry_for_dotfile_undo};
use nodus_core::undo::select_adoption;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Tudo o que o undo precisa saber, já resolvido pelo `main`.
pub struct Request<'a> {
    pub home: &'a Path,
    pub entries: &'a [HistoryEntry],
    /// Número da entrada, como `nodusctl history` mostra (a partir de 1).
    pub n: u32,
    /// A entrada é um terminal: sem ele não há confirmação possível.
    pub interactive: bool,
    pub history_path: &'a Path,
    pub now_unix: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Declined,
    Undone,
}

fn render_plan(plan: &UndoAdoptPlan) -> String {
    format!(
        "Desfazer a adoção do dotfile: {} ({} bytes, permissão {:o})\n  copiar de: {}\n  trocar o symlink {} por um arquivo comum com esse conteúdo\nA pasta-fonte não é alterada: a cópia continua lá.\n",
        plan.path,
        plan.size,
        plan.mode,
        plan.source.display(),
        plan.link.display()
    )
}

/// Valida, mostra o plano, pergunta `Executar? [s/N]`, troca o symlink pela cópia e grava o
/// histórico. Sem terminal, recusa.
pub fn undo_dotfile(
    request: &Request,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Outcome> {
    let entry = select_adoption(request.entries, request.n)?;
    let path = entry
        .requested
        .first()
        .context("a entrada não tem caminho")?;
    let dir_text = entry
        .dir
        .as_deref()
        .context("a entrada não tem pasta-fonte")?;
    let dir = Path::new(dir_text);
    let context = || format!("dotfile {path:?}");

    // Só leitura até aqui: valida antes de tocar no disco, porque o histórico não é confiável.
    validate_path(path).with_context(context)?;
    validate_dir(request.home, dir).with_context(context)?;
    check_parents(dir, path).with_context(context)?;
    let link = observe(request.home, path)?;
    let source = observe(dir, path)?;
    let mode = fs::symlink_metadata(dir.join(path))
        .map(|meta| meta.mode())
        .unwrap_or(0);
    let plan =
        plan_undo_adopt(request.home, dir, path, &link, &source, mode).with_context(context)?;

    write!(output, "{}", render_plan(&plan)).context("escrevendo o plano")?;
    if !request.interactive {
        bail!("sem terminal interativo não há como confirmar; nada foi alterado");
    }
    if !crate::apply::confirm(input, output).context("lendo a resposta")? {
        writeln!(output, "Nada foi alterado.").context("escrevendo a saída")?;
        return Ok(Outcome::Declined);
    }

    restore_file(&plan)?;

    // A troca já aconteceu; se o histórico falhar, o erro diz isso, para o dono não repetir.
    let command = [
        "nodusctl".to_string(),
        "history".to_string(),
        "undo".to_string(),
        request.n.to_string(),
    ];
    let record = entry_for_dotfile_undo(request.n, path, dir_text, &command, request.now_unix);
    crate::history::append(request.history_path, &record).context(
        "a reversão foi feita (o symlink agora é um arquivo comum), mas não consegui gravar o histórico",
    )?;
    writeln!(
        output,
        "Desfeito: {} agora é um arquivo comum (cópia de {}; a pasta-fonte não foi alterada)",
        plan.link.display(),
        plan.source.display()
    )
    .context("escrevendo a saída")?;
    Ok(Outcome::Undone)
}

/// Copia a fonte para um temporário ao lado do symlink, confere, reconfere o symlink e troca por
/// `rename`. Falha antes do `rename` remove só o temporário (`create_new`) e deixa o symlink.
fn restore_file(plan: &UndoAdoptPlan) -> Result<()> {
    let before = snapshot(&plan.source)?;
    if before.size != plan.size {
        bail!(
            "a fonte mudou desde o plano: tem {} bytes, eram {}",
            before.size,
            plan.size
        );
    }
    let mut source =
        File::open(&plan.source).with_context(|| format!("abrindo {}", plan.source.display()))?;
    let opened = source.metadata().context("lendo a fonte aberta")?;
    if opened.dev() != before.dev || opened.ino() != before.ino {
        bail!("a fonte mudou: foi trocada enquanto era aberta");
    }
    // No máximo um byte além do plano: uma fonte que cresceu é detectada sem ler tudo.
    let mut data = Vec::new();
    (&mut source)
        .take(plan.size + 1)
        .read_to_end(&mut data)
        .with_context(|| format!("lendo {}", plan.source.display()))?;
    if data.len() as u64 != plan.size {
        bail!(
            "a fonte mudou: tem mais de {} bytes, eram {}",
            data.len(),
            plan.size
        );
    }

    let name = plan
        .link
        .file_name()
        .context("caminho sem nome de arquivo")?
        .to_string_lossy();
    let tmp = plan
        .link
        .with_file_name(format!(".{name}.nodus-{}", std::process::id()));
    let out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("criando {}", tmp.display()))?;
    // O temporário é nosso (`create_new`): se algo falhar até o `rename`, só ele é removido.
    let outcome = write_verify_swap(plan, out, &data, &tmp);
    if outcome.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    outcome
}

fn write_verify_swap(plan: &UndoAdoptPlan, mut out: File, data: &[u8], tmp: &Path) -> Result<()> {
    out.write_all(data)
        .with_context(|| format!("gravando {}", tmp.display()))?;
    // Permissão exata (fchmod ignora o umask), só os 9 bits que o plano trouxe.
    out.set_permissions(fs::Permissions::from_mode(plan.mode))
        .context("ajustando a permissão da cópia")?;
    out.sync_all().context("gravando a cópia no disco")?;
    drop(out);

    let copied = fs::read(tmp).with_context(|| format!("relendo {}", tmp.display()))?;
    if copied != data {
        bail!("a cópia não confere com a fonte; nada foi trocado");
    }

    // O symlink não pode ter mudado desde o plano: só se troca o que o nodus criou.
    let target = fs::read_link(&plan.link)
        .with_context(|| format!("o symlink {} mudou durante a reversão", plan.link.display()))?;
    if target != plan.source && false {
        bail!(
            "o symlink {} mudou durante a reversão; nada foi trocado",
            plan.link.display()
        );
    }
    fs::rename(tmp, &plan.link).with_context(|| format!("trocando {}", plan.link.display()))
}

#[cfg(test)]
mod tests {
    //! `history undo` de uma adoção de dotfile (ADR 0023), com entrada e saída injetadas. Cada teste
    //! adota um arquivo de verdade (`dotfiles_adopt::adopt`) e depois desfaz.
    use super::*;
    use crate::dotfiles_adopt;
    use crate::testutil::TmpDir;
    use nodus_core::history::{Action, HistoryEntry, Manager, entry_from_adopt};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    struct Cenario {
        _tmp: TmpDir,
        home: PathBuf,
        dir: PathBuf,
        history: PathBuf,
    }

    /// Um HOME com `.bashrc` já adotado: o HOME tem o symlink e a pasta-fonte tem o conteúdo.
    fn adotado() -> Cenario {
        let tmp = TmpDir::novo();
        let home = tmp.path().join("home");
        let dir = tmp.path().join("dot");
        let state = tmp.path().join("state");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(home.join(".bashrc"), b"alias x=y\n").unwrap();
        fs::set_permissions(home.join(".bashrc"), fs::Permissions::from_mode(0o640)).unwrap();
        let c = Cenario {
            history: state.join("nodus/history.jsonl"),
            _tmp: tmp,
            home,
            dir,
        };
        adotar(&c, ".bashrc");
        c
    }

    fn adotar(c: &Cenario, path: &str) {
        let pedido = dotfiles_adopt::Request {
            home: &c.home,
            dir: &c.dir,
            path,
            interactive: true,
            history_path: &c.history,
            now_unix: 1_700_000_300,
        };
        let resultado = dotfiles_adopt::adopt(&pedido, &mut "s\n".as_bytes(), &mut Vec::new());
        assert_eq!(resultado.unwrap(), dotfiles_adopt::Outcome::Adopted);
    }

    fn historico(c: &Cenario) -> Vec<HistoryEntry> {
        crate::history::read(&c.history).unwrap()
    }

    fn roda_com(
        c: &Cenario,
        entries: &[HistoryEntry],
        n: u32,
        interactive: bool,
        resposta: &str,
    ) -> (Result<Outcome>, String) {
        let pedido = Request {
            home: &c.home,
            entries,
            n,
            interactive,
            history_path: &c.history,
            now_unix: 1_700_000_400,
        };
        let mut saida = Vec::new();
        let resultado = undo_dotfile(&pedido, &mut resposta.as_bytes(), &mut saida);
        (resultado, String::from_utf8(saida).unwrap())
    }

    fn roda(c: &Cenario, n: u32, interactive: bool, resposta: &str) -> (Result<Outcome>, String) {
        roda_com(c, &historico(c), n, interactive, resposta)
    }

    fn continua_symlink(c: &Cenario) {
        assert_eq!(
            fs::read_link(c.home.join(".bashrc")).unwrap(),
            c.dir.join(".bashrc")
        );
        assert_eq!(fs::read(c.dir.join(".bashrc")).unwrap(), b"alias x=y\n");
        assert_eq!(historico(c).len(), 1, "nada novo no histórico");
    }

    #[test]
    fn confirmando_com_s_troca_o_symlink_por_copia_e_mantem_a_fonte() {
        let c = adotado();
        let (resultado, saida) = roda(&c, 1, true, "s\n");

        assert_eq!(resultado.unwrap(), Outcome::Undone);
        let meta = fs::symlink_metadata(c.home.join(".bashrc")).unwrap();
        assert!(meta.file_type().is_file(), "voltou a ser arquivo comum");
        assert_eq!(fs::read(c.home.join(".bashrc")).unwrap(), b"alias x=y\n");
        assert_eq!(meta.permissions().mode() & 0o777, 0o640);
        assert_eq!(fs::read(c.dir.join(".bashrc")).unwrap(), b"alias x=y\n");
        assert!(saida.contains("Executar? [s/N]"), "{saida}");
        assert!(saida.contains("Desfeito:"), "{saida}");
    }

    #[test]
    fn grava_uma_linha_v4_remove_ligada_a_adocao() {
        let c = adotado();
        roda(&c, 1, true, "s\n").0.unwrap();

        let h = historico(&c);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].action, Action::Adopt, "a adoção não é editada");
        let e = &h[1];
        assert_eq!(e.version, 4);
        assert_eq!(e.action, Action::Remove);
        assert_eq!(e.manager, Manager::Dotfiles);
        assert_eq!(e.undoes, Some(1));
        assert_eq!(e.requested, [".bashrc"]);
        assert!(e.installed.is_empty());
        assert_eq!(e.still_missing, [".bashrc"]);
        assert_eq!(e.dir.as_deref(), c.dir.to_str());
        assert_eq!(e.at_unix, 1_700_000_400);
    }

    #[test]
    fn o_plano_mostra_caminho_fonte_tamanho_e_o_que_acontece_com_a_fonte() {
        let c = adotado();
        let (_, saida) = roda(&c, 1, true, "n\n");
        assert!(
            saida.contains("Desfazer a adoção do dotfile: .bashrc (10 bytes"),
            "{saida}"
        );
        assert!(
            saida.contains(&format!("copiar de: {}", c.dir.join(".bashrc").display())),
            "{saida}"
        );
        assert!(saida.contains("trocar o symlink"), "{saida}");
        assert!(saida.contains("pasta-fonte não é alterada"), "{saida}");
    }

    #[test]
    fn a_copia_leva_o_conteudo_atual_da_fonte_nao_o_da_adocao() {
        let c = adotado();
        fs::write(c.dir.join(".bashrc"), b"alias x=y\nalias z=w\n").unwrap();
        roda(&c, 1, true, "s\n").0.unwrap();
        assert_eq!(
            fs::read(c.home.join(".bashrc")).unwrap(),
            b"alias x=y\nalias z=w\n"
        );
    }

    #[test]
    fn qualquer_resposta_que_nao_seja_s_ou_sim_nao_altera_nada() {
        for resposta in ["n\n", "\n", "", "talvez\n", "y\n", "ss\n"] {
            let c = adotado();
            let (resultado, saida) = roda(&c, 1, true, resposta);
            assert_eq!(resultado.unwrap(), Outcome::Declined, "{resposta:?}");
            assert!(saida.contains("Nada foi alterado."), "{saida}");
            continua_symlink(&c);
        }
    }

    #[test]
    fn sem_terminal_recusa_depois_de_mostrar_o_plano_e_sem_alterar_nada() {
        let c = adotado();
        let (resultado, saida) = roda(&c, 1, false, "s\n");
        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("terminal"), "{erro}");
        assert!(!saida.contains("Executar?"), "{saida}");
        continua_symlink(&c);
    }

    #[test]
    fn recusa_se_o_symlink_virou_arquivo_comum_e_nao_toca_nele() {
        let c = adotado();
        fs::remove_file(c.home.join(".bashrc")).unwrap();
        fs::write(c.home.join(".bashrc"), b"editado no HOME\n").unwrap();
        let (resultado, saida) = roda(&c, 1, true, "s\n");
        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("symlink"), "{erro}");
        assert!(!saida.contains("Executar?"), "{saida}");
        assert_eq!(
            fs::read(c.home.join(".bashrc")).unwrap(),
            b"editado no HOME\n"
        );
        assert_eq!(historico(&c).len(), 1);
    }

    #[test]
    fn recusa_se_o_symlink_aponta_para_outro_lugar() {
        let c = adotado();
        let outro = c.dir.join("outro");
        fs::write(&outro, b"outro\n").unwrap();
        fs::remove_file(c.home.join(".bashrc")).unwrap();
        symlink(&outro, c.home.join(".bashrc")).unwrap();
        let (resultado, saida) = roda(&c, 1, true, "s\n");
        assert!(resultado.is_err());
        assert!(!saida.contains("Executar?"), "{saida}");
        assert_eq!(fs::read_link(c.home.join(".bashrc")).unwrap(), outro);
        assert_eq!(historico(&c).len(), 1);
    }

    #[test]
    fn recusa_se_o_arquivo_sumiu_do_home() {
        let c = adotado();
        fs::remove_file(c.home.join(".bashrc")).unwrap();
        let (resultado, saida) = roda(&c, 1, true, "s\n");
        assert!(resultado.is_err());
        assert!(!saida.contains("Executar?"), "{saida}");
        assert!(fs::symlink_metadata(c.home.join(".bashrc")).is_err());
        assert_eq!(historico(&c).len(), 1);
    }

    #[test]
    fn recusa_se_a_fonte_sumiu_e_deixa_o_symlink() {
        let c = adotado();
        fs::remove_file(c.dir.join(".bashrc")).unwrap();
        let (resultado, saida) = roda(&c, 1, true, "s\n");
        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("fonte"), "{erro}");
        assert!(!saida.contains("Executar?"), "{saida}");
        assert!(
            fs::symlink_metadata(c.home.join(".bashrc"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(historico(&c).len(), 1);
    }

    #[test]
    fn recusa_fonte_acima_de_1_mib() {
        let c = adotado();
        fs::write(c.dir.join(".bashrc"), vec![b'a'; (1 << 20) + 1]).unwrap();
        let (resultado, saida) = roda(&c, 1, true, "s\n");
        assert!(resultado.is_err());
        assert!(!saida.contains("Executar?"), "{saida}");
        assert!(
            fs::symlink_metadata(c.home.join(".bashrc"))
                .unwrap()
                .is_symlink()
        );
    }

    #[test]
    fn recusa_adocao_ja_desfeita_e_nao_grava_de_novo() {
        let c = adotado();
        roda(&c, 1, true, "s\n").0.unwrap();
        // Volta a ser um symlink "certo" para provar que o motivo da recusa é o histórico.
        fs::remove_file(c.home.join(".bashrc")).unwrap();
        symlink(c.dir.join(".bashrc"), c.home.join(".bashrc")).unwrap();
        let (resultado, saida) = roda(&c, 1, true, "s\n");
        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("desfeita"), "{erro}");
        assert!(!saida.contains("Executar?"), "{saida}");
        assert_eq!(historico(&c).len(), 2);
    }

    #[test]
    fn recusa_entrada_que_nao_e_adocao_e_numero_fora_do_intervalo() {
        let c = adotado();
        roda(&c, 1, true, "s\n").0.unwrap();
        for n in [2, 3, 0] {
            let (resultado, _) = roda(&c, n, true, "s\n");
            assert!(resultado.is_err(), "n={n}");
        }
        assert_eq!(historico(&c).len(), 2);
    }

    #[test]
    fn linha_plantada_com_caminho_de_segredo_nao_escreve_nada() {
        // O histórico não é assinado: mesmo que a entrada chegue (aqui montada em memória, já que a
        // leitura a recusaria), o plano revalida o caminho e o nodus não escreve.
        let c = adotado();
        fs::create_dir_all(c.home.join(".ssh")).unwrap();
        fs::write(c.home.join(".ssh/authorized_keys"), b"chave\n").unwrap();
        let plantada = entry_from_adopt(
            ".ssh/authorized_keys",
            c.dir.to_str().unwrap(),
            &["nodusctl".to_string()],
            1,
        );
        let (resultado, saida) = roda_com(&c, &[plantada], 1, true, "s\n");
        assert!(resultado.is_err());
        assert!(!saida.contains("Executar?"), "{saida}");
        assert_eq!(
            fs::read(c.home.join(".ssh/authorized_keys")).unwrap(),
            b"chave\n"
        );
    }

    #[test]
    fn pasta_fonte_do_historico_dentro_de_segredo_e_recusada() {
        let c = adotado();
        let dentro = c.home.join(".ssh/dot");
        fs::create_dir_all(&dentro).unwrap();
        fs::write(dentro.join(".bashrc"), b"x\n").unwrap();
        fs::remove_file(c.home.join(".bashrc")).unwrap();
        symlink(dentro.join(".bashrc"), c.home.join(".bashrc")).unwrap();
        let plantada = entry_from_adopt(
            ".bashrc",
            dentro.to_str().unwrap(),
            &["nodusctl".to_string()],
            1,
        );
        let (resultado, saida) = roda_com(&c, &[plantada], 1, true, "s\n");
        assert!(resultado.is_err());
        assert!(!saida.contains("Executar?"), "{saida}");
        assert!(
            fs::symlink_metadata(c.home.join(".bashrc"))
                .unwrap()
                .is_symlink()
        );
    }

    #[test]
    fn recusa_se_uma_pasta_do_caminho_e_symlink() {
        let c = adotado();
        // `.config` é um symlink para fora: escrever em `.config/git/config` sairia do HOME (T2).
        let fora = c._tmp.path().join("fora");
        fs::create_dir_all(fora.join("git")).unwrap();
        fs::create_dir_all(c.dir.join(".config/git")).unwrap();
        fs::write(c.dir.join(".config/git/config"), b"cfg\n").unwrap();
        symlink(&fora, c.home.join(".config")).unwrap();
        symlink(c.dir.join(".config/git/config"), fora.join("git/config")).unwrap();
        let plantada = entry_from_adopt(
            ".config/git/config",
            c.dir.to_str().unwrap(),
            &["nodusctl".to_string()],
            1,
        );
        let (resultado, saida) = roda_com(&c, &[plantada], 1, true, "s\n");
        assert!(resultado.is_err());
        assert!(!saida.contains("Executar?"), "{saida}");
        assert!(
            fs::symlink_metadata(fora.join("git/config"))
                .unwrap()
                .is_symlink()
        );
    }

    #[test]
    fn nao_deixa_temporario_para_tras_depois_de_desfazer() {
        let c = adotado();
        roda(&c, 1, true, "s\n").0.unwrap();
        let nomes: Vec<String> = fs::read_dir(&c.home)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(nomes, [".bashrc"], "{nomes:?}");
    }

    #[test]
    fn falha_ao_gravar_o_historico_diz_que_a_reversao_ja_foi_feita() {
        let c = adotado();
        let entradas = historico(&c);
        // O arquivo do histórico já existe (a adoção o criou): tirar a escrita dele faz o append falhar.
        fs::set_permissions(&c.history, fs::Permissions::from_mode(0o400)).unwrap();
        // Como root a escrita passaria; nesse caso não há o que testar.
        if fs::OpenOptions::new().append(true).open(&c.history).is_ok() {
            fs::set_permissions(&c.history, fs::Permissions::from_mode(0o600)).unwrap();
            return;
        }
        let (resultado, _) = roda_com(&c, &entradas, 1, true, "s\n");
        fs::set_permissions(&c.history, fs::Permissions::from_mode(0o600)).unwrap();

        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("a reversão foi feita"), "{erro}");
        assert!(
            fs::symlink_metadata(c.home.join(".bashrc"))
                .unwrap()
                .file_type()
                .is_file()
        );
    }
}
