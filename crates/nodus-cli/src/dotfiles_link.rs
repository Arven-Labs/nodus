//! `nodusctl dotfiles link <caminho>` (ADR 0024): cria no HOME o symlink para um arquivo que já está
//! na pasta-fonte (máquina nova com o repo clonado). Nunca sobrescreve: o destino não pode existir
//! e o `symlink(2)` direto falha com `EEXIST` se algo aparecer no meio (um `rename` sobrescreveria).
//! Tudo o que pode ser recusado é recusado **antes** da pergunta; as pastas-pai só são criadas depois
//! da confirmação.

use crate::dotfiles::observe;
use crate::dotfiles_adopt::{check_parents, ensure_dirs};
use anyhow::{Context, Result, bail};
use nodus_core::dotfiles::{LinkPlan, plan_link, validate_path};
use nodus_core::history::entry_from_link;
use std::fs;
use std::io::{BufRead, ErrorKind, Write};
use std::os::unix::fs::symlink;
use std::path::Path;

/// Tudo o que o `link` precisa saber, já resolvido pelo `main` (como no `adopt`).
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
    Linked,
}

fn render_plan(plan: &LinkPlan, missing_dirs: &[String]) -> String {
    let dirs = if missing_dirs.is_empty() {
        String::new()
    } else {
        format!("  criar pastas: {}\n", missing_dirs.join(", "))
    };
    format!(
        "Ligar dotfile: {} ({} bytes)\n{dirs}  criar symlink: {} -> {}\nO destino não existe e o nodus nunca sobrescreve; a pasta-fonte não é alterada.\nAtenção: ligar um arquivo de shell (.bashrc, .profile) faz o shell executar o conteúdo dele no próximo login. Só ligue de uma pasta-fonte em que você confia; o nodus não olha o conteúdo.\n",
        plan.path,
        plan.size,
        plan.link.display(),
        plan.target.display()
    )
}

/// Confere as pastas do caminho dentro do HOME sem seguir symlink e devolve as que faltam (a
/// partir da primeira ausente, tudo abaixo dela também falta). Pasta que é symlink ou não é pasta
/// recusa (T2): o link sairia do HOME.
fn missing_home_dirs(home: &Path, path: &str) -> Result<Vec<String>> {
    let parts: Vec<&str> = path.split('/').collect();
    let mut current = home.to_path_buf();
    let mut relative = String::new();
    let mut missing = Vec::new();
    for part in &parts[..parts.len() - 1] {
        current.push(part);
        if !relative.is_empty() {
            relative.push('/');
        }
        relative.push_str(part);
        if !missing.is_empty() {
            missing.push(relative.clone());
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => bail!(
                "{} é um symlink; o nodus só liga em pastas comuns do HOME",
                current.display()
            ),
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => bail!("{} existe e não é uma pasta", current.display()),
            Err(erro) if erro.kind() == ErrorKind::NotFound => missing.push(relative.clone()),
            Err(erro) => {
                return Err(erro).with_context(|| format!("lendo {}", current.display()));
            }
        }
    }
    Ok(missing)
}

/// Valida, mostra o plano, pergunta `Executar? [s/N]`, cria as pastas e o symlink e grava o
/// histórico. Sem terminal, recusa.
pub fn link(
    request: &Request,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Outcome> {
    let context = || format!("dotfile {:?}", request.path);
    validate_path(request.path).with_context(context)?;
    check_parents(request.dir, request.path).with_context(context)?;
    let missing_dirs = missing_home_dirs(request.home, request.path).with_context(context)?;
    let source = observe(request.dir, request.path)?;
    let dest = observe(request.home, request.path)?;
    let plan =
        plan_link(request.home, request.dir, request.path, &source, &dest).with_context(context)?;
    let dir_text = request
        .dir
        .to_str()
        .context("a pasta-fonte não é UTF-8; o histórico não consegue registrá-la")?;

    write!(output, "{}", render_plan(&plan, &missing_dirs)).context("escrevendo o plano")?;
    if !request.interactive {
        bail!("sem terminal interativo não há como confirmar; nada foi alterado");
    }
    if !crate::apply::confirm(input, output).context("lendo a resposta")? {
        writeln!(output, "Nada foi alterado.").context("escrevendo a saída")?;
        return Ok(Outcome::Declined);
    }

    ensure_dirs(request.home, request.path)?;
    if observe(request.dir, request.path)? != source {
        bail!("a fonte mudou desde o plano; nada foi ligado");
    }
    match symlink(&plan.target, &plan.link) {
        Ok(()) => {}
        Err(erro) if erro.kind() == ErrorKind::AlreadyExists => bail!(
            "o destino {} apareceu durante a ligação; o nodus nunca sobrescreve",
            plan.link.display()
        ),
        Err(erro) => {
            return Err(erro).with_context(|| format!("criando o link {}", plan.link.display()));
        }
    }

    // O link já existe; se o histórico falhar, o erro diz isso, para o dono não repetir.
    let command = ["nodusctl", "dotfiles", "link", "--", request.path].map(String::from);
    let entry = entry_from_link(request.path, dir_text, &command, request.now_unix);
    crate::history::append(request.history_path, &entry).context(
        "a ligação foi feita (o symlink foi criado), mas não consegui gravar o histórico",
    )?;
    writeln!(
        output,
        "Ligado: {} -> {}",
        plan.link.display(),
        plan.target.display()
    )
    .context("escrevendo a saída")?;
    Ok(Outcome::Linked)
}

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

    /// Prepara um estado do HOME ou da pasta-fonte antes de rodar o `link`.
    type Preparo = fn(&Cenario);

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
        let preparos: [(&str, Preparo); 3] = [
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
        let preparos: [(&str, Preparo); 4] = [
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

#[cfg(test)]
mod tests_lote {
    //! `dotfiles link --all` (ADR 0030): um plano, uma pergunta, uma linha de histórico por item.
    //! `paths` já é a lista curada que o `main` monta; aqui o lote recebe a lista e decide o que
    //! liga e o que pula.
    use super::*;
    use crate::testutil::TmpDir;
    use nodus_core::history::{Action, Manager};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    struct Cenario {
        _tmp: TmpDir,
        home: PathBuf,
        dir: PathBuf,
        history: PathBuf,
        paths: Vec<String>,
    }

    /// HOME vazio; pasta-fonte com três arquivos da lista e um segredo que nunca pode ser ligado.
    fn cenario() -> Cenario {
        let tmp = TmpDir::novo();
        let home = tmp.path().join("home");
        let dir = tmp.path().join("dot");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(dir.join(".config/git")).unwrap();
        fs::create_dir_all(dir.join(".ssh")).unwrap();
        fs::write(dir.join(".bashrc"), b"alias x=y\n").unwrap();
        fs::write(dir.join(".gitconfig"), b"[user]\n").unwrap();
        fs::write(dir.join(".config/git/config"), b"[core]\n").unwrap();
        fs::write(dir.join(".ssh/config"), b"Host x\n").unwrap();
        Cenario {
            history: tmp.path().join("state/nodus/history.jsonl"),
            _tmp: tmp,
            home,
            dir,
            paths: [".bashrc", ".gitconfig", ".config/git/config"]
                .map(String::from)
                .to_vec(),
        }
    }

    fn roda(c: &Cenario, interactive: bool, resposta: &str) -> (Result<BatchOutcome>, String) {
        let request = BatchRequest {
            home: &c.home,
            dir: &c.dir,
            paths: &c.paths,
            interactive,
            history_path: &c.history,
            now_unix: 1_700_000_600,
        };
        let mut saida = Vec::new();
        let resultado = link_all(&request, &mut resposta.as_bytes(), &mut saida);
        (resultado, String::from_utf8(saida).unwrap())
    }

    fn eh_link(c: &Cenario, path: &str) -> bool {
        fs::read_link(c.home.join(path)).is_ok_and(|alvo| alvo == c.dir.join(path))
    }

    #[test]
    fn um_s_liga_os_tres_e_grava_uma_linha_v4_link_por_arquivo() {
        let c = cenario();
        let (resultado, saida) = roda(&c, true, "s\n");

        assert_eq!(resultado.unwrap(), BatchOutcome::Linked(3));
        for path in &c.paths {
            assert!(eh_link(&c, path), "{path}");
        }
        assert_eq!(saida.matches("Executar? [s/N]").count(), 1, "{saida}");
        assert!(saida.contains("criar pastas"), "{saida}");

        let historico = crate::history::read(&c.history).unwrap();
        assert_eq!(historico.len(), 3);
        for (entrada, path) in historico.iter().zip(&c.paths) {
            assert_eq!(entrada.version, 4);
            assert_eq!(entrada.action, Action::Link);
            assert_eq!(entrada.manager, Manager::Dotfiles);
            assert_eq!(entrada.requested, [path.as_str()]);
            assert_eq!(entrada.dir.as_deref(), c.dir.to_str());
        }
    }

    #[test]
    fn o_plano_lista_o_que_liga_e_o_aviso_de_shell_aparece_uma_vez() {
        let c = cenario();
        let (_, saida) = roda(&c, true, "n\n");
        for path in &c.paths {
            assert!(saida.contains(path.as_str()), "{path}: {saida}");
        }
        assert_eq!(saida.matches("confia").count(), 1, "{saida}");
    }

    #[test]
    fn responder_n_nao_altera_nada() {
        let c = cenario();
        let (resultado, saida) = roda(&c, true, "n\n");

        assert_eq!(resultado.unwrap(), BatchOutcome::Declined);
        assert!(saida.contains("Nada foi alterado."), "{saida}");
        assert!(fs::read_dir(&c.home).unwrap().next().is_none());
        assert!(!c.history.exists());
    }

    #[test]
    fn sem_terminal_recusa_depois_do_plano_sem_alterar() {
        let c = cenario();
        let (resultado, saida) = roda(&c, false, "s\n");

        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("sem terminal"), "{erro}");
        assert!(saida.contains(".bashrc"), "{saida}");
        assert!(fs::read_dir(&c.home).unwrap().next().is_none());
        assert!(!c.history.exists());
    }

    #[test]
    fn destino_que_ja_existe_e_pulado_com_o_motivo_e_os_outros_ligam() {
        let c = cenario();
        fs::write(c.home.join(".bashrc"), b"de fabrica\n").unwrap();

        let (resultado, saida) = roda(&c, true, "s\n");

        assert_eq!(resultado.unwrap(), BatchOutcome::Linked(2));
        assert_eq!(fs::read(c.home.join(".bashrc")).unwrap(), b"de fabrica\n");
        assert!(eh_link(&c, ".gitconfig"));
        assert!(eh_link(&c, ".config/git/config"));
        assert!(saida.contains("pulado"), "{saida}");
        assert!(saida.contains("já existe"), "{saida}");
        assert_eq!(crate::history::read(&c.history).unwrap().len(), 2);
    }

    #[test]
    fn ja_ligado_e_fonte_ausente_sao_pulados_sem_erro() {
        let c = cenario();
        symlink(c.dir.join(".bashrc"), c.home.join(".bashrc")).unwrap();
        fs::remove_file(c.dir.join(".gitconfig")).unwrap();

        let (resultado, saida) = roda(&c, true, "s\n");

        assert_eq!(resultado.unwrap(), BatchOutcome::Linked(1));
        assert!(saida.contains("já está ligado"), "{saida}");
        assert!(saida.contains("não existe"), "{saida}");
        assert!(eh_link(&c, ".config/git/config"));
        assert!(!c.home.join(".gitconfig").exists());
    }

    #[test]
    fn nada_a_ligar_diz_isso_e_nao_pergunta() {
        let c = cenario();
        for path in &c.paths {
            let destino = c.home.join(path);
            fs::create_dir_all(destino.parent().unwrap()).unwrap();
            symlink(c.dir.join(path), destino).unwrap();
        }

        let (resultado, saida) = roda(&c, true, "");

        assert_eq!(resultado.unwrap(), BatchOutcome::NothingToDo);
        assert!(!saida.contains("Executar?"), "{saida}");
        assert!(saida.contains("Nada a ligar"), "{saida}");
        assert!(!c.history.exists());
    }

    #[test]
    fn caminho_da_denylist_na_lista_e_pulado_e_nunca_ligado() {
        let mut c = cenario();
        c.paths.push(".ssh/config".to_string());

        let (resultado, saida) = roda(&c, true, "s\n");

        assert_eq!(resultado.unwrap(), BatchOutcome::Linked(3));
        assert!(!c.home.join(".ssh").exists());
        assert!(saida.contains(".ssh/config"), "{saida}");
        assert!(saida.contains("pulado"), "{saida}");
    }

    #[test]
    fn erro_de_execucao_para_o_lote_e_o_item_ja_ligado_fica_com_historico() {
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

        let (resultado, _) = roda(&c, true, "s\n");
        fs::set_permissions(&pasta, fs::Permissions::from_mode(0o700)).unwrap();

        let erro = format!("{:#}", resultado.unwrap_err());
        assert!(erro.contains("a ligação foi feita"), "{erro}");
        assert!(eh_link(&c, ".bashrc"), "o primeiro item já estava ligado");
        assert!(!c.home.join(".gitconfig").exists(), "o lote devia parar");
        assert!(!c.home.join(".config").exists(), "o lote devia parar");
    }
}
