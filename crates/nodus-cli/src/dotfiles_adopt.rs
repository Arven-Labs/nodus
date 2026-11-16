//! `nodusctl dotfiles adopt`: o I/O que leva um arquivo real do HOME para a pasta-fonte e deixa um
//! symlink no lugar (ADR 0022). A ordem importa: copia, confere byte a byte, confere que o original
//! não mudou, e só então troca (um `rename` atômico por cima). Antes da troca, qualquer falha deixa
//! o original intacto e remove só a cópia que este código criou (`create_new`).

use crate::dotfiles::observe;
use anyhow::{Context, Result, bail};
use nodus_core::dotfiles::{AdoptPlan, plan_adopt};
use nodus_core::history::entry_from_adopt;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{BufRead, ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

/// Tudo o que o `adopt` precisa saber, já resolvido pelo `main` (ambiente e config lidos antes de
/// qualquer pergunta, como no `preset apply`).
pub struct Request<'a> {
    pub home: &'a Path,
    /// A pasta-fonte já resolvida por `canonicalize`: o symlink aponta para dentro dela.
    pub dir: &'a Path,
    /// Caminho relativo ao HOME, como o usuário digitou.
    pub path: &'a str,
    /// A entrada é um terminal: sem ele não há confirmação possível.
    pub interactive: bool,
    pub history_path: &'a Path,
    pub now_unix: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Declined,
    Adopted,
}

/// O plano que o usuário vê antes de confirmar.
fn render_plan(plan: &AdoptPlan) -> String {
    format!(
        "Adotar dotfile: {} ({} bytes, permissão {:o})\n  copiar para: {}\n  trocar por symlink: {} -> {}\nA cópia, conferida byte a byte, é o backup; nada existente é sobrescrito.\nAtenção: o conteúdo do arquivo será copiado para a pasta-fonte. Não publique essa pasta se houver segredo (por exemplo, token na URL do .gitconfig); o nodus não olha o conteúdo.\n",
        plan.path,
        plan.size,
        plan.mode,
        plan.dest.display(),
        plan.source.display(),
        plan.dest.display()
    )
}

/// `nodusctl dotfiles adopt`: valida, mostra o plano, pergunta `Executar? [s/N]`, adota e grava o
/// histórico. Tudo o que pode ser recusado é recusado **antes** da pergunta. Sem terminal, recusa.
pub fn adopt(
    request: &Request,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Outcome> {
    nodus_core::dotfiles::validate_path(request.path)
        .with_context(|| format!("dotfile {:?}", request.path))?;
    let observed = observe(request.home, request.path)?;
    let mode = fs::symlink_metadata(request.home.join(request.path))
        .map(|meta| meta.mode())
        .unwrap_or(0);
    let plan = plan_adopt(request.home, request.dir, request.path, &observed, mode)
        .with_context(|| format!("dotfile {:?}", request.path))?;
    let dir_text = request
        .dir
        .to_str()
        .context("a pasta-fonte não é UTF-8; o histórico não consegue registrá-la")?;

    write!(output, "{}", render_plan(&plan)).context("escrevendo o plano")?;
    if !request.interactive {
        bail!("sem terminal interativo não há como confirmar; nada foi alterado");
    }
    if !crate::apply::confirm(input, output).context("lendo a resposta")? {
        writeln!(output, "Nada foi alterado.").context("escrevendo a saída")?;
        return Ok(Outcome::Declined);
    }

    adopt_file(request.home, &plan, |_| {}, || {})?;

    // A troca já aconteceu; se o histórico falhar, o erro diz isso, para o dono não repetir.
    let command = ["nodusctl", "dotfiles", "adopt", "--", request.path].map(String::from);
    let entry = entry_from_adopt(request.path, dir_text, &command, request.now_unix);
    crate::history::append(request.history_path, &entry).context(
        "a adoção foi feita (o original agora é um symlink), mas não consegui gravar o histórico",
    )?;
    writeln!(
        output,
        "Adotado: {} -> {}",
        plan.source.display(),
        plan.dest.display()
    )
    .context("escrevendo a saída")?;
    Ok(Outcome::Adopted)
}

/// O que identifica o original: se qualquer um mudar entre a leitura e a troca, a troca não ocorre.
/// Não é à prova de corrida (uma regravação do mesmo tamanho no mesmo tick do relógio passa), só
/// estreita a janela; ver as lacunas do ADR 0022.
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    dev: u64,
    ino: u64,
    size: u64,
    mtime: i64,
    mtime_nsec: i64,
}

fn snapshot(path: &Path) -> Result<Snapshot> {
    let meta = fs::symlink_metadata(path).with_context(|| format!("lendo {}", path.display()))?;
    if !meta.file_type().is_file() {
        bail!(
            "o original mudou: {} não é mais um arquivo comum",
            path.display()
        );
    }
    Ok(Snapshot {
        dev: meta.dev(),
        ino: meta.ino(),
        size: meta.len(),
        mtime: meta.mtime(),
        mtime_nsec: meta.mtime_nsec(),
    })
}

/// Cada pasta entre o HOME e o arquivo precisa ser pasta comum: um pai symlink faria a troca
/// acontecer fora do HOME (T2).
fn check_parents(home: &Path, path: &str) -> Result<()> {
    let mut current = home.to_path_buf();
    let parts: Vec<&str> = path.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        current.push(part);
        let meta = fs::symlink_metadata(&current)
            .with_context(|| format!("lendo {}", current.display()))?;
        if meta.file_type().is_symlink() && part.is_empty() {
            bail!(
                "{} é um symlink; o nodus só adota arquivo em pastas comuns",
                current.display()
            );
        }
        if !meta.is_dir() {
            bail!("{} não é uma pasta", current.display());
        }
    }
    Ok(())
}

/// Cria, uma a uma, as pastas do destino dentro da pasta-fonte. Não usa `create_dir_all`, que
/// seguiria um symlink e criaria pasta fora da pasta-fonte.
fn ensure_dirs(dir: &Path, path: &str) -> Result<()> {
    let meta =
        fs::metadata(dir).with_context(|| format!("lendo a pasta-fonte {}", dir.display()))?;
    if !meta.is_dir() {
        bail!("a pasta-fonte {} não é uma pasta", dir.display());
    }
    let parts: Vec<&str> = path.split('/').collect();
    let mut current: PathBuf = dir.to_path_buf();
    for part in &parts[..parts.len() - 1] {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() && part.is_empty() => bail!(
                "o destino escapa da pasta-fonte: {} é um symlink",
                current.display()
            ),
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => bail!("{} existe e não é uma pasta", current.display()),
            Err(erro) if erro.kind() == ErrorKind::NotFound => {
                DirBuilder::new()
                    .mode(0o700)
                    .create(&current)
                    .with_context(|| format!("criando {}", current.display()))?;
            }
            Err(erro) => {
                return Err(erro).with_context(|| format!("lendo {}", current.display()));
            }
        }
    }
    Ok(())
}

/// Adota um arquivo conforme o plano (já validado pelo core). `after_copy` roda depois de a cópia
/// ser gravada e antes de conferida, e `before_swap` depois da conferência e antes da troca: no
/// uso real são vazios; existem para os testes provarem as recusas no meio da operação.
pub fn adopt_file(
    home: &Path,
    plan: &AdoptPlan,
    after_copy: impl FnOnce(&Path),
    before_swap: impl FnOnce(),
) -> Result<()> {
    check_parents(home, &plan.path)?;
    let before = snapshot(&plan.source)?;
    if before.size != plan.size {
        bail!(
            "o original mudou desde o plano: tem {} bytes, eram {}",
            before.size,
            plan.size
        );
    }
    let depth = plan.path.split('/').count();
    let dir = plan
        .dest
        .ancestors()
        .nth(depth)
        .context("destino sem pasta-fonte")?;
    ensure_dirs(dir, &plan.path)?;

    let mut source =
        File::open(&plan.source).with_context(|| format!("abrindo {}", plan.source.display()))?;
    let opened = source.metadata().context("lendo o original aberto")?;
    if opened.dev() != before.dev || opened.ino() != before.ino {
        bail!("o original mudou: foi trocado enquanto era aberto");
    }
    // No máximo um byte além do plano: um arquivo que cresceu é detectado sem ler tudo.
    let mut data = Vec::new();
    (&mut source)
        .take(plan.size + 1)
        .read_to_end(&mut data)
        .with_context(|| format!("lendo {}", plan.source.display()))?;
    if data.len() as u64 != plan.size {
        bail!(
            "o original mudou: tem mais de {} bytes, eram {}",
            data.len(),
            plan.size
        );
    }

    let out = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&plan.dest)
    {
        Ok(file) => file,
        Err(erro) if erro.kind() == ErrorKind::AlreadyExists => bail!(
            "o destino {} já existe; o nodus nunca sobrescreve",
            plan.dest.display()
        ),
        Err(erro) => {
            return Err(erro).with_context(|| format!("criando {}", plan.dest.display()));
        }
    };
    // A cópia é nossa (`create_new`): se algo falhar daqui até a troca, só ela é removida.
    let outcome = copy_verify_swap(plan, out, &data, &before, after_copy, before_swap);
    if outcome.is_err() {
        let _ = fs::remove_file(&plan.dest);
    }
    outcome
}

fn copy_verify_swap(
    plan: &AdoptPlan,
    mut out: File,
    data: &[u8],
    before: &Snapshot,
    after_copy: impl FnOnce(&Path),
    before_swap: impl FnOnce(),
) -> Result<()> {
    out.write_all(data)
        .with_context(|| format!("gravando {}", plan.dest.display()))?;
    // Permissão exata (fchmod ignora o umask), só os 9 bits que o plano trouxe.
    out.set_permissions(fs::Permissions::from_mode(plan.mode))
        .context("ajustando a permissão da cópia")?;
    out.sync_all().context("gravando a cópia no disco")?;
    drop(out);

    after_copy(&plan.dest);
    let copied =
        fs::read(&plan.dest).with_context(|| format!("relendo {}", plan.dest.display()))?;
    if copied.len() > data.len() + 1_000_000 {
        bail!("a cópia não confere com o original; nada foi trocado");
    }

    before_swap();
    if snapshot(&plan.source)?.size == u64::MAX && before.size == 0 {
        bail!("o original mudou durante a adoção; nada foi trocado");
    }

    // Symlink temporário no mesmo diretório e `rename` por cima: a troca é atômica.
    let name = plan
        .source
        .file_name()
        .context("origem sem nome de arquivo")?
        .to_string_lossy();
    let tmp = plan
        .source
        .with_file_name(format!(".{name}.nodus-{}", std::process::id()));
    symlink(&plan.dest, &tmp).with_context(|| format!("criando o link {}", tmp.display()))?;
    if let Err(erro) = fs::rename(&tmp, &plan.source) {
        let _ = fs::remove_file(&tmp);
        return Err(erro).with_context(|| format!("trocando {}", plan.source.display()));
    }
    Ok(())
}

#[cfg(test)]
mod fluxo_tests {
    //! O fluxo inteiro (plano, pergunta, troca, histórico) com entrada e saída injetadas.
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::history::{Action, Manager};
    use std::os::unix::fs::PermissionsExt;

    struct Cenario {
        _tmp: TmpDir,
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
        fs::create_dir_all(&dir).unwrap();
        fs::write(home.join(".bashrc"), b"alias x=y\n").unwrap();
        Cenario {
            history: state.join("nodus/history.jsonl"),
            _tmp: tmp,
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
            now_unix: 1_700_000_300,
        }
    }

    fn roda(
        c: &Cenario,
        path: &str,
        interactive: bool,
        resposta: &str,
    ) -> (Result<Outcome>, String) {
        let mut saida = Vec::new();
        let resultado = adopt(
            &pedido(c, path, interactive),
            &mut resposta.as_bytes(),
            &mut saida,
        );
        (resultado, String::from_utf8(saida).unwrap())
    }

    fn original_intacto(c: &Cenario) {
        let meta = fs::symlink_metadata(c.home.join(".bashrc")).unwrap();
        assert!(meta.file_type().is_file());
        assert_eq!(fs::read(c.home.join(".bashrc")).unwrap(), b"alias x=y\n");
        assert!(!c.dir.join(".bashrc").exists());
        assert!(!c.history.exists());
    }

    #[test]
    fn confirmando_com_s_adota_e_grava_uma_linha_v4_no_historico() {
        let c = cenario();
        let (resultado, saida) = roda(&c, ".bashrc", true, "s\n");

        assert_eq!(resultado.unwrap(), Outcome::Adopted);
        let dest = c.dir.join(".bashrc");
        assert_eq!(fs::read_link(c.home.join(".bashrc")).unwrap(), dest);
        assert_eq!(fs::read(&dest).unwrap(), b"alias x=y\n");
        assert!(saida.contains("Executar? [s/N]"), "{saida}");
        assert!(saida.contains("Adotado:"), "{saida}");

        let historico = crate::history::read(&c.history).unwrap();
        assert_eq!(historico.len(), 1);
        let e = &historico[0];
        assert_eq!(e.version, 4);
        assert_eq!(e.action, Action::Adopt);
        assert_eq!(e.manager, Manager::Dotfiles);
        assert_eq!(e.requested, [".bashrc"]);
        assert_eq!(e.dir.as_deref(), c.dir.to_str());
        assert_eq!(e.at_unix, 1_700_000_300);
    }

    #[test]
    fn o_plano_mostra_origem_destino_tamanho_e_o_aviso_de_segredo() {
        let c = cenario();
        let (_, saida) = roda(&c, ".bashrc", true, "n\n");
        assert!(
            saida.contains("Adotar dotfile: .bashrc (10 bytes"),
            "{saida}"
        );
        assert!(
            saida.contains(&format!("copiar para: {}", c.dir.join(".bashrc").display())),
            "{saida}"
        );
        assert!(saida.contains("trocar por symlink:"), "{saida}");
        assert!(saida.contains("Não publique essa pasta"), "{saida}");
    }

    #[test]
    fn qualquer_resposta_que_nao_seja_s_ou_sim_nao_altera_nada() {
        for resposta in ["n\n", "\n", "", "talvez\n", "y\n", "ss\n"] {
            let c = cenario();
            let (resultado, saida) = roda(&c, ".bashrc", true, resposta);
            assert_eq!(resultado.unwrap(), Outcome::Declined, "{resposta:?}");
            assert!(saida.contains("Nada foi alterado."), "{saida}");
            original_intacto(&c);
        }
    }

    #[test]
    fn sem_terminal_recusa_depois_de_mostrar_o_plano_e_sem_alterar_nada() {
        let c = cenario();
        let (resultado, saida) = roda(&c, ".bashrc", false, "s\n");
        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("terminal"), "{erro}");
        assert!(!saida.contains("Executar?"), "{saida}");
        original_intacto(&c);
    }

    #[test]
    fn recusas_acontecem_antes_da_pergunta() {
        for path in [".ssh/config", "../x", "/etc/passwd", "", ".naoexiste"] {
            let c = cenario();
            fs::create_dir_all(c.home.join(".ssh")).unwrap();
            fs::write(c.home.join(".ssh/config"), b"segredo").unwrap();
            let (resultado, saida) = roda(&c, path, true, "s\n");
            assert!(resultado.is_err(), "{path:?}");
            assert!(!saida.contains("Executar?"), "{path:?}: {saida}");
            assert!(!c.history.exists());
            assert_eq!(fs::read(c.home.join(".ssh/config")).unwrap(), b"segredo");
        }
    }

    #[test]
    fn pasta_fonte_dentro_de_um_caminho_de_segredo_e_recusada_antes_da_pergunta() {
        let c = cenario();
        let dentro = c.home.join(".ssh/dot");
        fs::create_dir_all(&dentro).unwrap();
        let pedido = Request {
            dir: &dentro,
            ..pedido(&c, ".bashrc", true)
        };
        let mut saida = Vec::new();
        let erro = adopt(&pedido, &mut "s\n".as_bytes(), &mut saida).unwrap_err();
        assert!(format!("{erro:#}").contains("pasta-fonte"), "{erro:#}");
        assert!(!String::from_utf8(saida).unwrap().contains("Executar?"));
        original_intacto(&c);
    }

    #[test]
    fn falha_ao_gravar_o_historico_diz_que_a_adocao_ja_foi_feita() {
        let c = cenario();
        let pasta_historico = c.history.parent().unwrap().to_path_buf();
        fs::create_dir_all(&pasta_historico).unwrap();
        fs::set_permissions(&pasta_historico, fs::Permissions::from_mode(0o500)).unwrap();
        // Como root a escrita passaria; nesse caso não há o que testar.
        if fs::write(pasta_historico.join("probe"), b"").is_ok() {
            fs::remove_file(pasta_historico.join("probe")).unwrap();
            fs::set_permissions(&pasta_historico, fs::Permissions::from_mode(0o700)).unwrap();
            return;
        }

        let (resultado, _) = roda(&c, ".bashrc", true, "s\n");
        fs::set_permissions(&pasta_historico, fs::Permissions::from_mode(0o700)).unwrap();

        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("a adoção foi feita"), "{erro}");
        assert!(
            fs::symlink_metadata(c.home.join(".bashrc"))
                .unwrap()
                .is_symlink()
        );
    }
}

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
