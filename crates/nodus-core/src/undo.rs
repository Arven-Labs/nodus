//! Lógica pura da reversão (`nodusctl history undo`, ADR 0011): quais pacotes de uma entrada ainda
//! podem sair, os comandos do pacman, a leitura do plano simulado e a entrada que a reversão grava.
//! Sem I/O: quem consulta o pacman, pergunta e executa é a CLI. Remover pacote é classe
//! privilegiada (SECURITY.md §4) e o histórico não é assinado (T5), então tudo que vem dele ou do
//! pacman é validado antes de ir para a tela ou para um comando.

use crate::history::{Action, ENTRY_VERSION, ExecResult, HistoryEntry, Manager};
use crate::preset::is_valid_pacman_name;
use std::collections::HashSet;
use std::fmt;
use thiserror::Error;

/// Por que um pacote da entrada não entra na remoção.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    NotInstalled,
    /// Está instalado, mas como dependência de outro pacote (`pacman -Qe` não o lista).
    NotExplicit,
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SkipReason::NotInstalled => "não está instalado",
            SkipReason::NotExplicit => "instalado como dependência, não explícito",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub name: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidates {
    /// O que pode ser removido, na ordem da entrada e sem repetição.
    pub remove: Vec<String>,
    pub skipped: Vec<Skipped>,
}

/// Do `installed` da entrada, fica só o que segue instalado (`pacman -Q`) e foi instalado de propósito
/// (`pacman -Qe`). Os outros campos da entrada nunca viram alvo: `still_missing` nunca foi instalado.
pub fn undo_candidates(
    entry: &HistoryEntry,
    installed: &[String],
    explicit: &[String],
) -> Candidates {
    let installed: HashSet<&str> = installed.iter().map(String::as_str).collect();
    let explicit: HashSet<&str> = explicit.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    let mut remove = Vec::new();
    let mut skipped = Vec::new();
    for name in &entry.installed {
        if !seen.insert(name.as_str()) {
            continue;
        }
        if !installed.contains(name.as_str()) {
            skipped.push(Skipped {
                name: name.clone(),
                reason: SkipReason::NotInstalled,
            });
        } else if !explicit.contains(name.as_str()) {
            skipped.push(Skipped {
                name: name.clone(),
                reason: SkipReason::NotExplicit,
            });
        } else {
            remove.push(name.clone());
        }
    }
    Candidates { remove, skipped }
}

/// Simulação: roda sem root e lista tudo o que sairia, inclusive dependências arrastadas (medido).
pub fn remove_print_command(candidates: &[String]) -> Option<Vec<String>> {
    command(
        &["pacman", "-Rs", "--print", "--print-format", "%n %v"],
        candidates,
    )
}

/// Remoção real. Nunca `-n` (apaga os `.pacsave`), `-c`, `-d` nem `--noconfirm` (ADR 0011).
pub fn remove_command(candidates: &[String]) -> Option<Vec<String>> {
    command(&["sudo", "pacman", "-Rs"], candidates)
}

fn command(head: &[&str], candidates: &[String]) -> Option<Vec<String>> {
    if candidates.is_empty() {
        return None;
    }
    let mut command: Vec<String> = head.iter().map(|s| s.to_string()).collect();
    command.push("--".to_string());
    command.extend(candidates.iter().cloned());
    Some(command)
}

/// Uma linha do plano simulado (`%n %v`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UndoError {
    #[error("entrada {n} fora do intervalo (o histórico tem {len})")]
    OutOfRange { n: u32, len: usize },
    #[error("a entrada {0} não é uma instalação; só se desfaz instalação")]
    NotAnInstall(u32),
    #[error(
        "a entrada {0} é a adoção de um dotfile; ela se desfaz pelo caminho de dotfile, não pelo do pacman (nada foi alterado)"
    )]
    Dotfile(u32),
    #[error("a entrada {0} não é a adoção nem a ligação de um dotfile")]
    NotADotfileEntry(u32),
    #[error("a entrada {0} já foi desfeita")]
    AlreadyUndone(u32),
    #[error(
        "a entrada {0} é de Flatpak; ela se desfaz pelo caminho do Flatpak, não pelo do pacman (nada foi alterado)"
    )]
    NotPacman(u32),
    #[error("a entrada {0} não é de Flatpak; esta reversão só desfaz instalação de app Flatpak")]
    NotFlatpak(u32),
    #[error("a simulação do pacman não listou nada")]
    EmptyPlan,
    #[error("linha do plano fora do formato `nome versão`: {0:?}")]
    InvalidPlanLine(String),
    #[error("{0} não aparece no plano da simulação")]
    MissingFromPlan(String),
}

fn is_valid_version(version: &str) -> bool {
    !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '_' | ':' | '~' | '-'))
}

/// A saída da simulação é entrada externa e vai para a tela: cada linha é exatamente `nome versão`,
/// com o nome pela regra do pacman e a versão em caracteres de versão (a época usa `:`).
pub fn parse_remove_plan(stdout: &str) -> Result<Vec<Removal>, UndoError> {
    if stdout.trim().is_empty() {
        return Err(UndoError::EmptyPlan);
    }
    let body = stdout.strip_suffix('\n').unwrap_or(stdout);
    body.split('\n')
        .map(|line| {
            let invalid = || UndoError::InvalidPlanLine(line.to_string());
            let mut parts = line.split(' ');
            match (parts.next(), parts.next(), parts.next()) {
                (Some(name), Some(version), None)
                    if is_valid_pacman_name(name) && is_valid_version(version) =>
                {
                    Ok(Removal {
                        name: name.to_string(),
                        version: version.to_string(),
                    })
                }
                _ => Err(invalid()),
            }
        })
        .collect()
}

/// Todo candidato tem de estar no plano; se não estiver, algo está errado e não se chama o `sudo`.
pub fn check_plan_covers(candidates: &[String], plan: &[Removal]) -> Result<(), UndoError> {
    for name in candidates {
        if !plan.iter().any(|removal| &removal.name == name) {
            return Err(UndoError::MissingFromPlan(name.clone()));
        }
    }
    Ok(())
}

/// A entrada escolhida pelo número que `nodusctl history` mostra (a partir de 1).
pub fn select_entry(entries: &[HistoryEntry], n: u32) -> Result<&HistoryEntry, UndoError> {
    let entry = usize::try_from(n)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|index| entries.get(index))
        .ok_or(UndoError::OutOfRange {
            n,
            len: entries.len(),
        })?;
    if entry.action == Action::Adopt {
        return Err(UndoError::Dotfile(n));
    }
    if entry.action != Action::Install {
        return Err(UndoError::NotAnInstall(n));
    }
    if entry.manager != Manager::Pacman {
        return Err(UndoError::NotPacman(n));
    }
    Ok(entry)
}

/// A instalação Flatpak escolhida pelo número. Recusa o que não é instalação (reversão, dotfile) e o
/// que é do pacman. Não há "já desfeita": como no pacman, o estado decide (o app que saiu deixa de
/// ser candidato).
pub fn select_flatpak_entry(entries: &[HistoryEntry], n: u32) -> Result<&HistoryEntry, UndoError> {
    let entry = usize::try_from(n)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|index| entries.get(index))
        .ok_or(UndoError::OutOfRange {
            n,
            len: entries.len(),
        })?;
    if entry.action == Action::Adopt || entry.action == Action::Link {
        return Err(UndoError::Dotfile(n));
    }
    if entry.action != Action::Install {
        return Err(UndoError::NotAnInstall(n));
    }
    if entry.manager != Manager::Flatpak {
        return Err(UndoError::NotFlatpak(n));
    }
    Ok(entry)
}

/// Do `installed` da entrada, fica só o que segue instalado como app (`flatpak list --app --user`).
/// O Flatpak não tem "instalado como dependência" entre apps (os runtimes nem aparecem em `--app`),
/// então o critério de "explícito" do pacman não existe aqui: o motivo de exclusão é só `NotInstalled`.
pub fn flatpak_undo_candidates(entry: &HistoryEntry, installed_apps: &[String]) -> Candidates {
    undo_candidates(entry, installed_apps, installed_apps)
}

/// Remoção real de apps Flatpak do `--user` (ADR da reversão de Flatpak). Sem simulação (o Flatpak
/// não tem): o prompt dele mostra o que sai. Nunca `-y`, `--noninteractive`, `--unused`,
/// `--delete-data`, `--system` nem `--all`.
pub fn flatpak_remove_command(candidates: &[String]) -> Option<Vec<String>> {
    command(&["flatpak", "uninstall", "--user", "-y"], candidates)
}

/// A entrada que a reversão de Flatpak acrescenta ao histórico: igual à do pacman, com
/// `manager = flatpak`. `still_installed` é o `flatpak list --app --user` de depois.
pub fn entry_for_flatpak_undo(
    undoes: u32,
    requested: &[String],
    still_installed: &[String],
    command: &[String],
    result: ExecResult,
    at_unix: u64,
) -> HistoryEntry {
    HistoryEntry {
        manager: Manager::Flatpak,
        ..entry_for_undo(
            undoes,
            requested,
            still_installed,
            command,
            result,
            at_unix,
        )
    }
}

/// O que uma entrada de dotfile desfazível registrou (ADRs 0023 e 0025).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotfileKind {
    /// `dotfiles adopt`: desfazer troca o symlink por uma cópia da fonte.
    Adopt,
    /// `dotfiles link`: desfazer remove o symlink.
    Link,
}

/// A adoção ou ligação de dotfile escolhida pelo número. Recusa o que não é uma das duas e a que já
/// tem uma reversão no histórico (`undoes` apontando para ela).
pub fn select_dotfile_entry(
    entries: &[HistoryEntry],
    n: u32,
) -> Result<(&HistoryEntry, DotfileKind), UndoError> {
    let entry = usize::try_from(n)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|index| entries.get(index))
        .ok_or(UndoError::OutOfRange {
            n,
            len: entries.len(),
        })?;
    let kind = match entry.action {
        Action::Adopt => DotfileKind::Adopt,
        Action::Link => DotfileKind::Link,
        _ => return Err(UndoError::NotADotfileEntry(n)),
    };
    let undone = entries.iter().any(|other| {
        other.action == Action::Remove
            && other.manager == Manager::Dotfiles
            && other.undoes == Some(n)
    });
    if undone {
        return Err(UndoError::AlreadyUndone(n));
    }
    Ok((entry, kind))
}

/// A entrada que a reversão acrescenta ao histórico. `still_installed` é o `pacman -Q` de depois:
/// dos pedidos, o que segue instalado vai em `installed` e o que saiu em `still_missing`.
pub fn entry_for_undo(
    undoes: u32,
    requested: &[String],
    still_installed: &[String],
    command: &[String],
    result: ExecResult,
    at_unix: u64,
) -> HistoryEntry {
    let present: HashSet<&str> = still_installed.iter().map(String::as_str).collect();
    let (installed, still_missing): (Vec<String>, Vec<String>) = requested
        .iter()
        .cloned()
        .partition(|name| present.contains(name.as_str()));
    HistoryEntry {
        version: ENTRY_VERSION,
        at_unix,
        command: command.to_vec(),
        requested: requested.to_vec(),
        installed,
        still_missing,
        result,
        action: Action::Remove,
        undoes: Some(undoes),
        manager: Manager::Pacman,
        dir: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{Action, ExecResult, HistoryEntry, parse_line, to_line};

    fn v(itens: &[&str]) -> Vec<String> {
        itens.iter().map(|s| s.to_string()).collect()
    }

    fn instalacao(installed: &[&str]) -> HistoryEntry {
        let mut comando = v(&["sudo", "pacman", "-S", "--needed", "--"]);
        comando.extend(v(installed));
        HistoryEntry {
            version: 3,
            at_unix: 1_700_000_000,
            command: comando,
            requested: v(installed),
            installed: v(installed),
            still_missing: vec![],
            result: ExecResult::Ok,
            action: Action::Install,
            undoes: None,
            manager: Manager::Pacman,
            dir: None,
        }
    }

    #[test]
    fn entrada_flatpak_nao_se_desfaz() {
        let mut e = instalacao(&["org.gnome.Gedit"]);
        e.manager = Manager::Flatpak;
        let erro = select_entry(&[e], 1).unwrap_err();
        assert_eq!(erro, UndoError::NotPacman(1));
    }

    #[test]
    fn select_entry_do_pacman_nao_atende_dotfile_e_o_erro_nao_diz_que_a_reversao_nao_existe() {
        let adocao = crate::history::entry_from_adopt(
            ".bashrc",
            "/home/u/dotfiles",
            &["nodusctl".to_string()],
            1_700_000_300,
        );
        let erro = select_entry(&[adocao], 1).unwrap_err();
        assert_eq!(erro, UndoError::Dotfile(1));
        let texto = erro.to_string();
        assert!(
            texto.contains("dotfile") && !texto.contains("ainda não existe"),
            "{texto}"
        );
    }

    // --- desfazer a adoção de dotfile (ADR 0023) ---

    fn adocao(path: &str) -> HistoryEntry {
        crate::history::entry_from_adopt(
            path,
            "/home/u/dotfiles",
            &v(&["nodusctl", "dotfiles", "adopt", "--", path]),
            1_700_000_300,
        )
    }

    fn reversao_dotfile(undoes: u32, path: &str) -> HistoryEntry {
        crate::history::entry_for_dotfile_undo(
            undoes,
            path,
            "/home/u/dotfiles",
            &v(&["nodusctl", "history", "undo", &undoes.to_string()]),
            1_700_000_400,
        )
    }

    fn ligacao(path: &str) -> HistoryEntry {
        crate::history::entry_from_link(
            path,
            "/home/u/dotfiles",
            &v(&["nodusctl", "dotfiles", "link", "--", path]),
            1_700_000_500,
        )
    }

    #[test]
    fn select_dotfile_entry_devolve_a_entrada_e_diz_se_e_adocao_ou_ligacao() {
        let entradas = [instalacao(&["sl"]), adocao(".bashrc"), ligacao(".profile")];
        let (e, tipo) = select_dotfile_entry(&entradas, 2).unwrap();
        assert_eq!(e.requested, v(&[".bashrc"]));
        assert_eq!(tipo, DotfileKind::Adopt);
        let (e, tipo) = select_dotfile_entry(&entradas, 3).unwrap();
        assert_eq!(e.requested, v(&[".profile"]));
        assert_eq!(tipo, DotfileKind::Link);
    }

    #[test]
    fn select_dotfile_entry_fora_do_intervalo() {
        let entradas = [adocao(".bashrc")];
        for n in [0, 2] {
            assert_eq!(
                select_dotfile_entry(&entradas, n).unwrap_err(),
                UndoError::OutOfRange { n, len: 1 }
            );
        }
    }

    #[test]
    fn select_dotfile_entry_recusa_o_que_nao_e_adocao_nem_ligacao() {
        let entradas = [
            instalacao(&["sl"]),
            remocao(1),
            reversao_dotfile(1, ".bashrc"),
        ];
        for n in 1..=3 {
            assert_eq!(
                select_dotfile_entry(&entradas, n).unwrap_err(),
                UndoError::NotADotfileEntry(n)
            );
        }
    }

    #[test]
    fn select_dotfile_entry_recusa_adocao_ja_desfeita() {
        let entradas = [adocao(".bashrc"), reversao_dotfile(1, ".bashrc")];
        assert_eq!(
            select_dotfile_entry(&entradas, 1).unwrap_err(),
            UndoError::AlreadyUndone(1)
        );
    }

    #[test]
    fn select_dotfile_entry_recusa_ligacao_ja_desfeita() {
        let entradas = [ligacao(".bashrc"), reversao_dotfile(1, ".bashrc")];
        assert_eq!(
            select_dotfile_entry(&entradas, 1).unwrap_err(),
            UndoError::AlreadyUndone(1)
        );
    }

    #[test]
    fn reversao_de_outra_entrada_nao_conta_como_desfeita() {
        let entradas = [
            adocao(".bashrc"),
            ligacao(".profile"),
            reversao_dotfile(2, ".profile"),
        ];
        assert!(select_dotfile_entry(&entradas, 1).is_ok());
    }

    #[test]
    fn a_mensagem_de_entrada_que_nao_e_de_dotfile_cita_a_entrada() {
        let texto = UndoError::NotADotfileEntry(4).to_string();
        assert!(texto.contains('4') && texto.contains("dotfile"), "{texto}");
    }

    #[test]
    fn a_mensagem_de_ja_desfeita_cita_a_entrada() {
        let texto = UndoError::AlreadyUndone(3).to_string();
        assert!(texto.contains('3') && texto.contains("desfeita"), "{texto}");
        // vale para adoção e ligação: não pode dizer só "adoção"
        assert!(!texto.contains("adoção"), "{texto}");
    }

    fn remocao(undoes: u32) -> HistoryEntry {
        HistoryEntry {
            version: 3,
            at_unix: 1_700_000_100,
            command: v(&["sudo", "pacman", "-Rs", "--", "sl"]),
            requested: v(&["sl"]),
            installed: vec![],
            still_missing: v(&["sl"]),
            result: ExecResult::Ok,
            action: Action::Remove,
            undoes: Some(undoes),
            manager: Manager::Pacman,
            dir: None,
        }
    }

    // --- candidatos: o que da entrada ainda pode sair (ADR 0011, item 2) ---

    #[test]
    fn candidato_e_o_que_segue_instalado_e_explicito() {
        let c = undo_candidates(&instalacao(&["sl"]), &v(&["sl", "zsh"]), &v(&["sl"]));
        assert_eq!(c.remove, v(&["sl"]));
        assert!(c.skipped.is_empty());
    }

    #[test]
    fn pacote_que_ja_nao_esta_instalado_e_ignorado() {
        let c = undo_candidates(&instalacao(&["sl"]), &v(&["zsh"]), &v(&["zsh"]));
        assert!(c.remove.is_empty());
        assert_eq!(
            c.skipped,
            vec![Skipped {
                name: "sl".into(),
                reason: SkipReason::NotInstalled
            }]
        );
    }

    #[test]
    fn pacote_que_virou_dependencia_nao_e_removido() {
        // Instalado, mas `pacman -Qe` não o lista: veio como dependência de outro.
        let c = undo_candidates(&instalacao(&["libfoo"]), &v(&["libfoo"]), &v(&[]));
        assert!(c.remove.is_empty());
        assert_eq!(c.skipped[0].reason, SkipReason::NotExplicit);
    }

    #[test]
    fn nao_instalado_tem_precedencia_sobre_nao_explicito() {
        let c = undo_candidates(&instalacao(&["sl"]), &v(&[]), &v(&[]));
        assert_eq!(c.skipped[0].reason, SkipReason::NotInstalled);
    }

    #[test]
    fn so_considera_o_campo_installed_da_entrada() {
        // `still_missing` nunca foi instalado pelo nodus: não pode virar alvo de remoção.
        let mut e = instalacao(&["sl"]);
        e.requested = v(&["sl", "zsh"]);
        e.still_missing = v(&["zsh"]);
        let c = undo_candidates(&e, &v(&["sl", "zsh"]), &v(&["sl", "zsh"]));
        assert_eq!(c.remove, v(&["sl"]));
        assert!(c.skipped.is_empty());
    }

    #[test]
    fn mantem_a_ordem_e_tira_duplicatas() {
        let mut e = instalacao(&["zsh", "sl", "zsh"]);
        e.requested = v(&["zsh", "sl"]);
        let c = undo_candidates(&e, &v(&["sl", "zsh"]), &v(&["sl", "zsh"]));
        assert_eq!(c.remove, v(&["zsh", "sl"]));
    }

    #[test]
    fn entrada_sem_instalados_nao_tem_candidatos() {
        let c = undo_candidates(&instalacao(&[]), &v(&["sl"]), &v(&["sl"]));
        assert!(c.remove.is_empty());
        assert!(c.skipped.is_empty());
    }

    #[test]
    fn motivos_de_ignorado_tem_texto_para_o_dono() {
        assert_eq!(SkipReason::NotInstalled.to_string(), "não está instalado");
        assert_eq!(
            SkipReason::NotExplicit.to_string(),
            "instalado como dependência, não explícito"
        );
    }

    // --- comandos: sempre lista de argumentos, `--` antes dos nomes, nunca as opções perigosas ---

    #[test]
    fn comando_de_simulacao_e_o_do_adr_0011() {
        assert_eq!(
            remove_print_command(&v(&["sl", "zsh"])),
            Some(v(&[
                "pacman",
                "-Rs",
                "--print",
                "--print-format",
                "%n %v",
                "--",
                "sl",
                "zsh"
            ]))
        );
    }

    #[test]
    fn comando_de_remocao_e_o_do_adr_0011() {
        assert_eq!(
            remove_command(&v(&["sl", "zsh"])),
            Some(v(&["sudo", "pacman", "-Rs", "--", "sl", "zsh"]))
        );
    }

    #[test]
    fn sem_candidatos_nao_ha_comando() {
        assert_eq!(remove_print_command(&[]), None);
        assert_eq!(remove_command(&[]), None);
    }

    #[test]
    fn comandos_nunca_levam_opcao_perigosa_e_os_nomes_ficam_depois_do_separador() {
        let proibidas = [
            "-n",
            "-c",
            "-d",
            "-dd",
            "-Rns",
            "-Rc",
            "-Rd",
            "-Rdd",
            "--nosave",
            "--cascade",
            "--nodeps",
            "--noconfirm",
        ];
        let conjuntos = [
            v(&["sl"]),
            v(&["sl", "zsh", "abi-dumper"]),
            v(&["a", "lib32-foo", "python-3to2", "gtk+3-x", "c++filt"]),
        ];
        for nomes in conjuntos {
            for comando in [
                remove_print_command(&nomes).unwrap(),
                remove_command(&nomes).unwrap(),
            ] {
                let sep = comando.iter().position(|a| a == "--").unwrap();
                assert_eq!(&comando[sep + 1..], nomes.as_slice(), "{comando:?}");
                for antes in &comando[..sep] {
                    assert!(
                        !proibidas.contains(&antes.as_str()),
                        "{antes} em {comando:?}"
                    );
                }
            }
        }
    }

    // --- plano da simulação: saída do pacman é entrada externa (`%n %v`, medida em 2026-11-14) ---

    #[test]
    fn le_o_plano_medido_com_dependencias_e_epoca_na_versao() {
        let plano =
            parse_remove_plan("abi-dumper 1.4-2\nvtable-dumper 1.2-3\nctags 1:6.2.1-1\n").unwrap();
        assert_eq!(
            plano,
            vec![
                Removal {
                    name: "abi-dumper".into(),
                    version: "1.4-2".into()
                },
                Removal {
                    name: "vtable-dumper".into(),
                    version: "1.2-3".into()
                },
                Removal {
                    name: "ctags".into(),
                    version: "1:6.2.1-1".into()
                },
            ]
        );
    }

    #[test]
    fn plano_vazio_e_erro() {
        for vazio in ["", "\n", "  \n"] {
            assert_eq!(
                parse_remove_plan(vazio),
                Err(UndoError::EmptyPlan),
                "{vazio:?}"
            );
        }
    }

    #[test]
    fn linha_do_plano_fora_do_formato_e_erro() {
        for ruim in [
            "so-nome\n",
            "a 1.0 extra\n",
            "a 1.0-1\n\nb 2.0-1\n",
            "error: target not found: x\n",
        ] {
            assert!(
                matches!(parse_remove_plan(ruim), Err(UndoError::InvalidPlanLine(_))),
                "{ruim:?}"
            );
        }
    }

    #[test]
    fn nome_ou_versao_do_plano_com_escape_ou_opcao_e_erro() {
        for ruim in [
            "-Rdd 1.0-1\n",
            "Zsh 1.0-1\n",
            "\u{1b}[31mx 1.0-1\n",
            "sl \u{1b}[31m1.0\n",
            "sl 1.0;rm\n",
            "sl $(id)\n",
        ] {
            assert!(
                matches!(parse_remove_plan(ruim), Err(UndoError::InvalidPlanLine(_))),
                "{ruim:?}"
            );
        }
    }

    #[test]
    fn plano_precisa_conter_todos_os_candidatos() {
        let plano = parse_remove_plan("sl 1.0-1\nzsh 5.9-1\n").unwrap();
        assert_eq!(check_plan_covers(&v(&["sl", "zsh"]), &plano), Ok(()));
        assert_eq!(
            check_plan_covers(&v(&["sl", "cowsay"]), &plano),
            Err(UndoError::MissingFromPlan("cowsay".into()))
        );
    }

    // --- escolher a entrada pelo número mostrado em `nodusctl history` ---

    #[test]
    fn escolhe_pelo_numero_a_partir_de_1() {
        let entradas = vec![instalacao(&["sl"]), instalacao(&["zsh"])];
        assert_eq!(select_entry(&entradas, 1).unwrap().installed, v(&["sl"]));
        assert_eq!(select_entry(&entradas, 2).unwrap().installed, v(&["zsh"]));
    }

    #[test]
    fn numero_fora_do_intervalo_e_erro() {
        let entradas = vec![instalacao(&["sl"])];
        for n in [0, 2, 99] {
            assert_eq!(
                select_entry(&entradas, n).unwrap_err(),
                UndoError::OutOfRange { n, len: 1 }
            );
        }
        assert_eq!(
            select_entry(&[], 1).unwrap_err(),
            UndoError::OutOfRange { n: 1, len: 0 }
        );
    }

    #[test]
    fn nao_desfaz_uma_entrada_que_ja_e_remocao() {
        let entradas = vec![instalacao(&["sl"]), remocao(1)];
        assert_eq!(
            select_entry(&entradas, 2).unwrap_err(),
            UndoError::NotAnInstall(2)
        );
    }

    #[test]
    fn instalacao_que_falhou_mas_instalou_algo_pode_ser_desfeita() {
        let mut e = instalacao(&["sl"]);
        e.result = ExecResult::Failed { code: Some(1) };
        assert!(select_entry(&[e], 1).is_ok());
    }

    // --- a entrada nova que a reversão grava (sempre acrescenta, ADR 0011 item 6) ---

    #[test]
    fn entrada_da_reversao_separa_ainda_instalados_e_removidos() {
        let e = entry_for_undo(
            3,
            &v(&["sl", "zsh"]),
            &v(&["zsh"]),
            &v(&["sudo", "pacman", "-Rs", "--", "sl", "zsh"]),
            ExecResult::Failed { code: Some(1) },
            1_794_626_444,
        );
        assert_eq!(e.version, 3);
        assert_eq!(e.manager, Manager::Pacman);
        assert_eq!(e.action, Action::Remove);
        assert_eq!(e.undoes, Some(3));
        assert_eq!(e.requested, v(&["sl", "zsh"]));
        assert_eq!(e.installed, v(&["zsh"]));
        assert_eq!(e.still_missing, v(&["sl"]));
        assert_eq!(e.result, ExecResult::Failed { code: Some(1) });
        assert_eq!(e.at_unix, 1_794_626_444);
    }

    // --- reversão de Flatpak (ADR da reversão de Flatpak): candidatos, comando, seleção e a entrada nova ---

    fn instalacao_flatpak(installed: &[&str]) -> HistoryEntry {
        let mut comando = v(&["flatpak", "install", "--user", "--", "flathub"]);
        comando.extend(v(installed));
        let mut e = instalacao(installed);
        e.command = comando;
        e.manager = Manager::Flatpak;
        e
    }

    #[test]
    fn select_flatpak_entry_aceita_so_instalacao_flatpak() {
        let e = instalacao_flatpak(&["org.gnome.Calculator"]);
        assert!(select_flatpak_entry(std::slice::from_ref(&e), 1).is_ok());
        // Do pacman: não é desta reversão.
        assert_eq!(
            select_flatpak_entry(&[instalacao(&["sl"])], 1).unwrap_err(),
            UndoError::NotFlatpak(1)
        );
        // Fora do intervalo (0 e além do fim).
        for n in [0, 2] {
            assert!(matches!(
                select_flatpak_entry(std::slice::from_ref(&e), n).unwrap_err(),
                UndoError::OutOfRange { .. }
            ));
        }
        // Reversão não se desfaz.
        let mut remocao = e.clone();
        remocao.action = Action::Remove;
        remocao.undoes = Some(1);
        assert_eq!(
            select_flatpak_entry(&[remocao], 1).unwrap_err(),
            UndoError::NotAnInstall(1)
        );
        // Dotfile tem o caminho próprio.
        assert_eq!(
            select_flatpak_entry(&[adocao(".bashrc")], 1).unwrap_err(),
            UndoError::Dotfile(1)
        );
    }

    #[test]
    fn o_erro_do_pacman_para_entrada_flatpak_aponta_o_caminho_certo() {
        let mut e = instalacao(&["org.gnome.Gedit"]);
        e.manager = Manager::Flatpak;
        let texto = select_entry(&[e], 1).unwrap_err().to_string();
        assert!(
            !texto.contains("ainda não existe"),
            "a reversão de Flatpak passou a existir: {texto}"
        );
    }

    #[test]
    fn candidatos_flatpak_sao_so_os_que_seguem_instalados() {
        let e = instalacao_flatpak(&["org.gnome.Calculator", "org.a.B", "org.gnome.Calculator"]);
        let c = flatpak_undo_candidates(&e, &v(&["org.gnome.Calculator", "org.outro.App"]));
        assert_eq!(c.remove, v(&["org.gnome.Calculator"]));
        assert_eq!(
            c.skipped,
            vec![Skipped {
                name: "org.a.B".into(),
                reason: SkipReason::NotInstalled
            }]
        );
    }

    #[test]
    fn candidatos_flatpak_nunca_incluem_o_que_nao_esta_em_installed_da_entrada() {
        // `still_missing` nunca foi instalado; um app instalado que não está na entrada não vira alvo.
        let mut e = instalacao_flatpak(&["org.a.B"]);
        e.still_missing = v(&["org.c.D"]);
        let c = flatpak_undo_candidates(&e, &v(&["org.a.B", "org.c.D", "org.e.F"]));
        assert_eq!(c.remove, v(&["org.a.B"]));
    }

    #[test]
    fn comando_de_remocao_flatpak_e_exato() {
        assert_eq!(
            flatpak_remove_command(&v(&["org.a.B", "org.c.D"])),
            Some(v(&["flatpak", "uninstall", "--user", "--", "org.a.B", "org.c.D"]))
        );
        assert_eq!(flatpak_remove_command(&[]), None);
    }

    #[test]
    fn comando_de_remocao_flatpak_nunca_leva_flag_perigosa() {
        let c = flatpak_remove_command(&v(&["org.a.B"])).unwrap();
        for proibida in [
            "-y",
            "--assumeyes",
            "--noninteractive",
            "--unused",
            "--delete-data",
            "--system",
            "--all",
            "--force-remove",
            "--no-related",
        ] {
            assert!(!c.iter().any(|a| a == proibida), "{proibida} em {c:?}");
        }
        // O `--` vem antes de qualquer ID (um ID não vira opção).
        let separador = c.iter().position(|a| a == "--").unwrap();
        assert_eq!(c[..separador], v(&["flatpak", "uninstall", "--user"]));
        assert_eq!(c[separador + 1], "org.a.B");
    }

    #[test]
    fn entrada_da_reversao_flatpak_e_flatpak_e_separa_instalados_e_removidos() {
        let e = entry_for_flatpak_undo(
            3,
            &v(&["org.a.B", "org.c.D"]),
            &v(&["org.c.D"]),
            &v(&["flatpak", "uninstall", "--user", "--", "org.a.B", "org.c.D"]),
            ExecResult::Failed { code: Some(1) },
            1_794_626_444,
        );
        assert_eq!(e.version, 3);
        assert_eq!(e.manager, Manager::Flatpak);
        assert_eq!(e.action, Action::Remove);
        assert_eq!(e.undoes, Some(3));
        assert_eq!(e.requested, v(&["org.a.B", "org.c.D"]));
        assert_eq!(e.installed, v(&["org.c.D"]));
        assert_eq!(e.still_missing, v(&["org.a.B"]));
        assert_eq!(e.result, ExecResult::Failed { code: Some(1) });
        assert_eq!(e.dir, None);
        assert_eq!(parse_line(&to_line(&e)).unwrap(), e, "a linha gravada é legível");
    }

    #[test]
    fn entrada_da_reversao_volta_igual_pelo_parser() {
        let e = entry_for_undo(
            1,
            &v(&["sl"]),
            &v(&[]),
            &v(&["sudo", "pacman", "-Rs", "--", "sl"]),
            ExecResult::Ok,
            5,
        );
        assert_eq!(parse_line(&to_line(&e)).unwrap(), e);
    }
}
