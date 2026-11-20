//! Histórico do que o nodus executou (uma entrada por `preset apply`). Puro, sem I/O: quem lê e
//! grava o arquivo é a CLI. O histórico é entrada externa na leitura (SECURITY.md T5): a linha
//! é estrita e recusada em vez de interpretada.

use crate::preset::{Plan, is_valid_flatpak_id, is_valid_pacman_name};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Versão do formato que o nodus grava para pacman e Flatpak. Mudar o formato exige subir este
/// número. A leitura aceita também a versão 1 (só instalações, sem `action` nem `undoes`), a 2 (sem
/// `manager`, sempre pacman) e a 4 (`dotfiles adopt`, ADR 0022).
pub const ENTRY_VERSION: u32 = 3;

/// Versão das entradas de dotfile: só `adopt`, com `manager = "dotfiles"` e `dir`. Pacman e Flatpak
/// continuam v3; um nodus antigo recusa a linha v4 (`UnsupportedVersion`), como no ADR 0014.
pub const DOTFILES_ENTRY_VERSION: u32 = 4;

/// O que a entrada registra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// `preset apply`: instalação.
    Install,
    /// `history undo`: reversão de uma instalação anterior.
    Remove,
    /// `dotfiles adopt`: o arquivo foi para a pasta-fonte e o original virou symlink (v4).
    Adopt,
    /// `dotfiles link`: o arquivo da pasta-fonte ganhou um symlink no HOME (v4, ADR 0024).
    Link,
}

/// Qual gerenciador a entrada registra. Define a regra de validação dos nomes (pacote do pacman ou
/// ID de app Flatpak). Entradas v1 e v2 não têm o campo e são sempre `Pacman`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Manager {
    Pacman,
    Flatpak,
    /// Só na v4: os nomes são caminhos de dotfile (regra de `dotfiles::validate_path`).
    Dotfiles,
}

/// Como a execução terminou, pelo status do processo. O que de fato mudou está em
/// `installed`/`still_missing`, conferido depois (o código de saída não diz isso).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ExecResult {
    Ok,
    /// `code` é `None` quando não houve código de saída (não iniciou, ou morreu por sinal).
    Failed {
        code: Option<i32>,
    },
}

/// Só serializa: a leitura passa por `parse_line`, que valida a combinação `version`, `action` e
/// `undoes`. A ordem dos campos é o contrato do arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryEntry {
    pub version: u32,
    /// Segundos desde 1970 em UTC.
    pub at_unix: u64,
    /// O comando exato que foi executado.
    pub command: Vec<String>,
    /// Pacotes que o comando pedia (instalar ou remover).
    pub requested: Vec<String>,
    /// Dos pedidos, os que estavam instalados depois (`pacman -Q`).
    pub installed: Vec<String>,
    /// Dos pedidos, os que estavam ausentes depois: numa instalação, os que não entraram; numa
    /// reversão, os que foram removidos.
    pub still_missing: Vec<String>,
    pub result: ExecResult,
    pub action: Action,
    /// Número (a partir de 1) da entrada que esta reversão desfaz; só existe em `Action::Remove`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undoes: Option<u32>,
    /// A partir da v3. A leitura de v1 e v2 preenche `Pacman`; pacman e Flatpak gravam v3.
    pub manager: Manager,
    /// Só na v4 (`adopt`): a pasta-fonte, absoluta, onde a cópia ficou. O `undo` futuro não depende
    /// do config de hoje.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
}

/// A linha como está no arquivo, antes de validar a combinação de campos por versão.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    version: u32,
    at_unix: u64,
    command: Vec<String>,
    requested: Vec<String>,
    installed: Vec<String>,
    still_missing: Vec<String>,
    result: ExecResult,
    action: Option<Action>,
    undoes: Option<u32>,
    manager: Option<Manager>,
    dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HistoryError {
    #[error("linha de histórico inválida: {0}")]
    Invalid(String),
    #[error(
        "versão {0} do histórico não é suportada (esta versão lê da 1 à {DOTFILES_ENTRY_VERSION})"
    )]
    UnsupportedVersion(u32),
    #[error("nome de pacote inválido no histórico: {0:?}")]
    InvalidPackageName(String),
}

/// Monta a entrada do passo do pacman comparando o plano de antes com o de depois. Só considera os
/// pacotes pedidos.
pub fn entry_from(
    before: &Plan,
    after: &Plan,
    command: &[String],
    result: ExecResult,
    at_unix: u64,
) -> HistoryEntry {
    install_entry(
        Manager::Pacman,
        &before.pacman_missing,
        &after.pacman_missing,
        command,
        result,
        at_unix,
    )
}

/// Igual a `entry_from`, para o passo do Flatpak: lê só `flatpak_missing`.
pub fn entry_from_flatpak(
    before: &Plan,
    after: &Plan,
    command: &[String],
    result: ExecResult,
    at_unix: u64,
) -> HistoryEntry {
    install_entry(
        Manager::Flatpak,
        &before.flatpak_missing,
        &after.flatpak_missing,
        command,
        result,
        at_unix,
    )
}

fn install_entry(
    manager: Manager,
    missing_before: &[String],
    missing_after: &[String],
    command: &[String],
    result: ExecResult,
    at_unix: u64,
) -> HistoryEntry {
    let requested = missing_before.to_vec();
    let missing_after: HashSet<&str> = missing_after.iter().map(String::as_str).collect();
    let (still_missing, installed): (Vec<String>, Vec<String>) = requested
        .iter()
        .cloned()
        .partition(|name| missing_after.contains(name.as_str()));
    HistoryEntry {
        version: ENTRY_VERSION,
        at_unix,
        command: command.to_vec(),
        requested,
        installed,
        still_missing,
        result,
        action: Action::Install,
        undoes: None,
        manager,
        dir: None,
    }
}

/// Entrada de uma adoção concluída (v4). Só se grava depois da troca por symlink; falha antes dela
/// deixa o original intacto e não há o que registrar.
pub fn entry_from_adopt(path: &str, dir: &str, command: &[String], at_unix: u64) -> HistoryEntry {
    HistoryEntry {
        version: DOTFILES_ENTRY_VERSION,
        at_unix,
        command: command.to_vec(),
        requested: vec![path.to_string()],
        installed: vec![path.to_string()],
        still_missing: vec![],
        result: ExecResult::Ok,
        action: Action::Adopt,
        undoes: None,
        manager: Manager::Dotfiles,
        dir: Some(dir.to_string()),
    }
}

/// Entrada de uma ligação concluída (v4, ADR 0024). Só se grava depois de o symlink ser criado;
/// falha antes dele não deixa nada no HOME (além de pastas-pai) e não há o que registrar.
pub fn entry_from_link(path: &str, dir: &str, command: &[String], at_unix: u64) -> HistoryEntry {
    HistoryEntry {
        action: Action::Link,
        ..entry_from_adopt(path, dir, command, at_unix)
    }
}

/// Entrada da reversão de uma adoção (v4, ADR 0023), gravada só depois de o symlink ser trocado
/// pela cópia. `undoes` é o número da adoção desfeita; o caminho fica em `still_missing`, como o
/// pacote removido no undo do pacman.
pub fn entry_for_dotfile_undo(
    undoes: u32,
    path: &str,
    dir: &str,
    command: &[String],
    at_unix: u64,
) -> HistoryEntry {
    HistoryEntry {
        version: DOTFILES_ENTRY_VERSION,
        at_unix,
        command: command.to_vec(),
        requested: vec![path.to_string()],
        installed: vec![],
        still_missing: vec![path.to_string()],
        result: ExecResult::Ok,
        action: Action::Remove,
        undoes: Some(undoes),
        manager: Manager::Dotfiles,
        dir: Some(dir.to_string()),
    }
}

fn check_dir(dir: &str) -> Result<(), HistoryError> {
    if !dir.starts_with('/') || dir.chars().any(char::is_control) {
        return Err(HistoryError::Invalid(
            "`dir` precisa ser um caminho absoluto, sem caractere de controle".to_string(),
        ));
    }
    Ok(())
}

/// Uma linha de JSON, sem quebra de linha (o JSON escapa `\n` dentro de textos).
pub fn to_line(entry: &HistoryEntry) -> String {
    // Invariante: a struct só tem textos, números e listas, e serializar isso não falha.
    serde_json::to_string(entry).expect("HistoryEntry sempre serializa")
}

pub fn parse_line(line: &str) -> Result<HistoryEntry, HistoryError> {
    let raw: RawEntry =
        serde_json::from_str(line).map_err(|e| HistoryError::Invalid(e.to_string()))?;
    let invalid = |why: &str| HistoryError::Invalid(why.to_string());
    let (action, undoes, manager, dir) = match (
        raw.version,
        raw.action,
        raw.undoes,
        raw.manager,
        raw.dir,
    ) {
        (1, None, None, None, None) => (Action::Install, None, Manager::Pacman, None),
        (1, ..) => {
            return Err(invalid(
                "a versão 1 não tem `action`, `undoes`, `manager` nem `dir`",
            ));
        }
        (2, Some(Action::Install), None, None, None) => {
            (Action::Install, None, Manager::Pacman, None)
        }
        (2, Some(Action::Remove), Some(n), None, None) if n >= 1 => {
            (Action::Remove, Some(n), Manager::Pacman, None)
        }
        (2, ..) => {
            return Err(invalid(
                "`install` não leva `undoes`; `remove` exige `undoes` a partir de 1; a versão 2 não tem `manager` nem `dir`",
            ));
        }
        (3, Some(Action::Install), None, Some(manager), None) if manager != Manager::Dotfiles => {
            (Action::Install, None, manager, None)
        }
        (3, Some(Action::Remove), Some(n), Some(Manager::Pacman), None) if n >= 1 => {
            (Action::Remove, Some(n), Manager::Pacman, None)
        }
        (3, ..) => {
            return Err(invalid(
                "a versão 3 exige `manager` (pacman ou flatpak); `install` não leva `undoes`; `remove` exige `undoes` a partir de 1 e só existe no pacman; não tem `dir`",
            ));
        }
        (4, Some(Action::Adopt), None, Some(Manager::Dotfiles), Some(dir)) => {
            check_dir(&dir)?;
            (Action::Adopt, None, Manager::Dotfiles, Some(dir))
        }
        (4, Some(Action::Link), None, Some(Manager::Dotfiles), Some(dir)) => {
            check_dir(&dir)?;
            (Action::Link, None, Manager::Dotfiles, Some(dir))
        }
        (4, Some(Action::Remove), Some(n), Some(Manager::Dotfiles), Some(dir)) if n >= 1 => {
            check_dir(&dir)?;
            (Action::Remove, Some(n), Manager::Dotfiles, Some(dir))
        }
        (4, ..) => {
            return Err(invalid(
                "a versão 4 só tem `adopt` ou `link` (sem `undoes`) ou `remove` (com `undoes` a partir de 1), sempre com `manager` dotfiles e `dir`",
            ));
        }
        (version, ..) => return Err(HistoryError::UnsupportedVersion(version)),
    };
    let entry = HistoryEntry {
        version: raw.version,
        at_unix: raw.at_unix,
        command: raw.command,
        requested: raw.requested,
        installed: raw.installed,
        still_missing: raw.still_missing,
        result: raw.result,
        action,
        undoes,
        manager,
        dir,
    };
    if matches!(entry.action, Action::Adopt | Action::Link)
        && (entry.requested.len() != 1
            || entry.installed != entry.requested
            || !entry.still_missing.is_empty()
            || entry.result != ExecResult::Ok)
    {
        return Err(invalid(
            "`adopt` e `link` registram um caminho (`installed` igual a `requested`), sem `still_missing` e com resultado ok",
        ));
    }
    if entry.action == Action::Remove
        && entry.manager == Manager::Dotfiles
        && (entry.requested.len() != 1
            || !entry.installed.is_empty()
            || entry.still_missing != entry.requested
            || entry.result != ExecResult::Ok)
    {
        return Err(invalid(
            "a reversão de dotfile registra um caminho, em `still_missing`, sem `installed` e com resultado ok",
        ));
    }
    // Os nomes vão para a tela: nada de escape de terminal nem de opção do gerenciador.
    let is_valid_name: fn(&str) -> bool = match entry.manager {
        Manager::Pacman => is_valid_pacman_name,
        Manager::Flatpak => is_valid_flatpak_id,
        Manager::Dotfiles => |path| crate::dotfiles::validate_path(path).is_ok(),
    };
    let names = entry
        .requested
        .iter()
        .chain(&entry.installed)
        .chain(&entry.still_missing);
    for name in names {
        if !is_valid_name(name) {
            return Err(HistoryError::InvalidPackageName(name.clone()));
        }
    }
    Ok(entry)
}

/// `2026-11-14T03:20:44Z`. Conversão de dias para data civil (algoritmo de Howard Hinnant), para
/// não depender de crate de datas.
pub fn format_utc(at_unix: u64) -> String {
    let days = (at_unix / 86_400) as i64;
    let secs = at_unix % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs / 3_600,
        secs % 3_600 / 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::Plan;

    fn plano(faltando: &[&str]) -> Plan {
        Plan {
            pacman_missing: v(faltando),
            pacman_present: 3,
            pacman_unavailable: vec![],
            flatpak_missing: vec![],
            flatpak_present: 0,
            flatpak_installed: true,
        }
    }

    fn v(itens: &[&str]) -> Vec<String> {
        itens.iter().map(|s| s.to_string()).collect()
    }

    fn comando(pacotes: &[&str]) -> Vec<String> {
        let mut c = v(&["sudo", "pacman", "-S", "--needed", "--"]);
        c.extend(v(pacotes));
        c
    }

    // --- entry_from: o registro guarda o estado real, não o código de saída ---

    #[test]
    fn entry_from_separa_instalados_e_faltantes() {
        let e = entry_from(
            &plano(&["sl", "cowsay"]),
            &plano(&["cowsay"]),
            &comando(&["sl", "cowsay"]),
            ExecResult::Failed { code: Some(1) },
            1_700_000_000,
        );
        assert_eq!(e.version, 3);
        assert_eq!(e.manager, Manager::Pacman);
        assert_eq!(e.action, Action::Install);
        assert_eq!(e.undoes, None);
        assert_eq!(e.at_unix, 1_700_000_000);
        assert_eq!(e.command, comando(&["sl", "cowsay"]));
        assert_eq!(e.requested, v(&["sl", "cowsay"]));
        assert_eq!(e.installed, v(&["sl"]));
        assert_eq!(e.still_missing, v(&["cowsay"]));
        assert_eq!(e.result, ExecResult::Failed { code: Some(1) });
    }

    #[test]
    fn entry_from_tudo_instalado() {
        let e = entry_from(
            &plano(&["sl", "zsh"]),
            &plano(&[]),
            &comando(&["sl", "zsh"]),
            ExecResult::Ok,
            0,
        );
        assert_eq!(e.installed, v(&["sl", "zsh"]));
        assert!(e.still_missing.is_empty());
    }

    #[test]
    fn entry_from_nada_instalado() {
        let e = entry_from(
            &plano(&["sl"]),
            &plano(&["sl"]),
            &comando(&["sl"]),
            ExecResult::Failed { code: None },
            0,
        );
        assert!(e.installed.is_empty());
        assert_eq!(e.still_missing, v(&["sl"]));
    }

    #[test]
    fn entry_from_so_considera_o_que_foi_pedido() {
        // "outro" não estava em `requested`: não pode aparecer como faltante nem instalado.
        let e = entry_from(
            &plano(&["sl"]),
            &plano(&["outro"]),
            &comando(&["sl"]),
            ExecResult::Ok,
            0,
        );
        assert_eq!(e.requested, v(&["sl"]));
        assert_eq!(e.installed, v(&["sl"]));
        assert!(e.still_missing.is_empty());
    }

    // --- entry_from_flatpak: mesmo registro, lido do lado Flatpak do plano ---

    fn plano_flatpak(faltando: &[&str]) -> Plan {
        Plan {
            pacman_missing: v(&["sl"]),
            pacman_present: 3,
            pacman_unavailable: vec![],
            flatpak_missing: v(faltando),
            flatpak_present: 0,
            flatpak_installed: true,
        }
    }

    fn comando_flatpak(apps: &[&str]) -> Vec<String> {
        let mut c = v(&["flatpak", "install", "--user", "--", "flathub"]);
        c.extend(v(apps));
        c
    }

    #[test]
    fn entry_from_flatpak_separa_instalados_e_faltantes() {
        let e = entry_from_flatpak(
            &plano_flatpak(&["org.gnome.Calculator", "org.a.B"]),
            &plano_flatpak(&["org.a.B"]),
            &comando_flatpak(&["org.gnome.Calculator", "org.a.B"]),
            ExecResult::Failed { code: Some(1) },
            1_700_000_000,
        );
        assert_eq!(e.version, 3);
        assert_eq!(e.manager, Manager::Flatpak);
        assert_eq!(e.action, Action::Install);
        assert_eq!(e.undoes, None);
        assert_eq!(e.requested, v(&["org.gnome.Calculator", "org.a.B"]));
        assert_eq!(e.installed, v(&["org.gnome.Calculator"]));
        assert_eq!(e.still_missing, v(&["org.a.B"]));
        assert_eq!(e.result, ExecResult::Failed { code: Some(1) });
    }

    #[test]
    fn entry_from_flatpak_ignora_o_lado_do_pacman() {
        // "sl" (pacman) falta antes e depois: não pode aparecer na entrada Flatpak.
        let e = entry_from_flatpak(
            &plano_flatpak(&["org.a.B"]),
            &plano_flatpak(&[]),
            &comando_flatpak(&["org.a.B"]),
            ExecResult::Ok,
            0,
        );
        assert_eq!(e.requested, v(&["org.a.B"]));
        assert_eq!(e.installed, v(&["org.a.B"]));
        assert!(e.still_missing.is_empty());
    }

    #[test]
    fn entry_from_flatpak_passa_na_leitura_estrita_com_id_de_maiuscula() {
        let e = entry_from_flatpak(
            &plano_flatpak(&["org.gnome.Calculator"]),
            &plano_flatpak(&[]),
            &comando_flatpak(&["org.gnome.Calculator"]),
            ExecResult::Ok,
            1_700_000_000,
        );
        assert_eq!(parse_line(&to_line(&e)).unwrap(), e);
        assert!(to_line(&e).ends_with(r#""action":"install","manager":"flatpak"}"#));
    }

    // --- formato da linha (contrato de armazenamento) ---

    #[test]
    fn linha_tem_campos_e_ordem_estaveis() {
        let e = entry_from(
            &plano(&["sl"]),
            &plano(&[]),
            &comando(&["sl"]),
            ExecResult::Ok,
            1_700_000_000,
        );
        assert_eq!(
            to_line(&e),
            r#"{"version":3,"at_unix":1700000000,"command":["sudo","pacman","-S","--needed","--","sl"],"requested":["sl"],"installed":["sl"],"still_missing":[],"result":{"status":"ok"},"action":"install","manager":"pacman"}"#
        );
    }

    /// Uma reversão (`history undo`): o que se pediu para remover, o que ainda estava instalado
    /// depois e o que já não estava (foi removido).
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

    #[test]
    fn linha_de_remocao_tem_action_e_undoes_no_fim() {
        assert_eq!(
            to_line(&remocao(1)),
            r#"{"version":3,"at_unix":1700000100,"command":["sudo","pacman","-Rs","--","sl"],"requested":["sl"],"installed":[],"still_missing":["sl"],"result":{"status":"ok"},"action":"remove","undoes":1,"manager":"pacman"}"#
        );
    }

    #[test]
    fn ida_e_volta_de_remocao() {
        let e = remocao(7);
        assert_eq!(parse_line(&to_line(&e)).unwrap(), e);
    }

    // --- v1 continua legível, como instalação ---

    const LINHA_V1: &str = r#"{"version":1,"at_unix":1700000000,"command":["sudo","pacman","-S","--needed","--","sl"],"requested":["sl"],"installed":["sl"],"still_missing":[],"result":{"status":"ok"}}"#;

    #[test]
    fn le_linha_v1_como_instalacao() {
        let e = parse_line(LINHA_V1).unwrap();
        assert_eq!(e.version, 1);
        assert_eq!(e.action, Action::Install);
        assert_eq!(e.undoes, None);
        assert_eq!(e.installed, v(&["sl"]));
        assert_eq!(e.at_unix, 1_700_000_000);
    }

    #[test]
    fn v1_nao_pode_trazer_campos_da_v2() {
        for extra in [
            r#""action":"install""#,
            r#""undoes":1"#,
            r#""action":"remove""#,
        ] {
            let linha = LINHA_V1.replacen('{', &format!("{{{extra},"), 1);
            assert!(
                parse_line(&linha).is_err(),
                "{extra} em v1 deveria ser recusado"
            );
        }
    }

    // --- v2: action e undoes obrigatórios na combinação certa ---

    #[test]
    fn v2_exige_action() {
        let linha = to_line(&remocao(1)).replace(r#""action":"remove","#, "");
        assert!(parse_line(&linha).is_err());
        let linha = linha_valida().replace(r#","action":"install""#, "");
        assert!(parse_line(&linha).is_err());
    }

    #[test]
    fn recusa_action_desconhecida() {
        for ruim in ["upgrade", "", "Install", "REMOVE"] {
            let linha =
                linha_valida().replace(r#""action":"install""#, &format!(r#""action":"{ruim}""#));
            assert!(
                parse_line(&linha).is_err(),
                "action {ruim:?} deveria ser recusada"
            );
        }
    }

    #[test]
    fn v2_instalacao_nao_pode_ter_undoes() {
        let linha = linha_valida().replacen(
            r#""action":"install""#,
            r#""action":"install","undoes":1"#,
            1,
        );
        assert!(parse_line(&linha).is_err());
    }

    #[test]
    fn v2_remocao_exige_undoes_positivo() {
        let sem = to_line(&remocao(1)).replace(r#","undoes":1"#, "");
        assert!(parse_line(&sem).is_err(), "remove sem undoes");
        let zero = to_line(&remocao(1)).replace(r#""undoes":1"#, r#""undoes":0"#);
        assert!(
            parse_line(&zero).is_err(),
            "undoes = 0 (a numeração começa em 1)"
        );
        let nulo = to_line(&remocao(1)).replace(r#""undoes":1"#, r#""undoes":null"#);
        assert!(parse_line(&nulo).is_err(), "undoes nulo");
    }

    #[test]
    fn v2_remocao_aceita_undoes_no_limite_inferior_e_no_maximo() {
        for n in [1, 2, u32::MAX] {
            let e = parse_line(&to_line(&remocao(n))).unwrap();
            assert_eq!(e.undoes, Some(n));
        }
    }

    #[test]
    fn nome_invalido_tambem_e_recusado_numa_remocao() {
        let mut e = remocao(1);
        e.requested = v(&["-Rdd"]);
        assert!(parse_line(&to_line(&e)).is_err());
    }

    #[test]
    fn resultado_de_falha_leva_o_codigo() {
        let e = entry_from(
            &plano(&["sl"]),
            &plano(&["sl"]),
            &comando(&["sl"]),
            ExecResult::Failed { code: Some(1) },
            5,
        );
        assert!(to_line(&e).ends_with(
            r#""result":{"status":"failed","code":1},"action":"install","manager":"pacman"}"#
        ));
    }

    #[test]
    fn linha_nunca_tem_quebra_de_linha() {
        let mut e = entry_from(
            &plano(&["sl"]),
            &plano(&[]),
            &comando(&["sl"]),
            ExecResult::Ok,
            0,
        );
        e.command.push("a\nb".into());
        assert!(!to_line(&e).contains('\n'));
    }

    #[test]
    fn ida_e_volta() {
        for resultado in [
            ExecResult::Ok,
            ExecResult::Failed { code: Some(1) },
            ExecResult::Failed { code: None },
        ] {
            let e = entry_from(
                &plano(&["sl", "cowsay"]),
                &plano(&["cowsay"]),
                &comando(&["sl", "cowsay"]),
                resultado,
                1_794_626_444,
            );
            assert_eq!(parse_line(&to_line(&e)).unwrap(), e);
        }
    }

    // --- leitura é entrada externa (T5): recusa em vez de adivinhar ---

    fn linha_valida() -> String {
        to_line(&entry_from(
            &plano(&["sl"]),
            &plano(&[]),
            &comando(&["sl"]),
            ExecResult::Ok,
            1,
        ))
    }

    #[test]
    fn recusa_linha_vazia_e_lixo() {
        for lixo in ["", "   ", "nao e json", "{}", "[]", "null"] {
            assert!(parse_line(lixo).is_err(), "{lixo:?} deveria ser recusado");
        }
    }

    #[test]
    fn recusa_campo_desconhecido() {
        let linha = linha_valida().replacen('{', r#"{"extra":1,"#, 1);
        assert!(parse_line(&linha).is_err());
    }

    #[test]
    fn recusa_campo_faltando() {
        let linha = linha_valida().replace(r#""still_missing":[],"#, "");
        assert!(parse_line(&linha).is_err());
    }

    #[test]
    fn recusa_versao_diferente() {
        for ruim in [0, 5, 99] {
            let linha =
                linha_valida().replacen(r#""version":3"#, &format!(r#""version":{ruim}"#), 1);
            assert_eq!(
                parse_line(&linha).unwrap_err(),
                HistoryError::UnsupportedVersion(ruim)
            );
        }
    }

    #[test]
    fn recusa_nome_de_pacote_invalido_em_qualquer_lista() {
        // Escape de terminal, opção do pacman, maiúscula e vazio: o nome é impresso na tela.
        for ruim in ["\u{1b}[31mx", "-Rns", "Zsh", "", "a b"] {
            let base = entry_from(
                &plano(&["sl"]),
                &plano(&[]),
                &comando(&["sl"]),
                ExecResult::Ok,
                1,
            );
            let mut em_requested = base.clone();
            em_requested.requested = v(&[ruim]);
            let mut em_installed = base.clone();
            em_installed.installed = v(&[ruim]);
            let mut em_faltando = base.clone();
            em_faltando.still_missing = v(&[ruim]);
            for (campo, e) in [
                ("requested", em_requested),
                ("installed", em_installed),
                ("still_missing", em_faltando),
            ] {
                assert!(
                    parse_line(&to_line(&e)).is_err(),
                    "{ruim:?} em {campo} deveria ser recusado"
                );
            }
        }
    }

    // --- v3: `manager` e a regra de nomes de cada gerenciador ---

    /// Linha v2 como o nodus a gravava antes da v3 (sem `manager`).
    const LINHA_V2: &str = r#"{"version":2,"at_unix":1700000000,"command":["sudo","pacman","-S","--needed","--","sl"],"requested":["sl"],"installed":["sl"],"still_missing":[],"result":{"status":"ok"},"action":"install"}"#;

    fn flatpak(requested: &[&str]) -> HistoryEntry {
        HistoryEntry {
            version: 3,
            at_unix: 1_700_000_200,
            command: v(&["flatpak", "install", "--user", "--", "flathub"]),
            requested: v(requested),
            installed: v(requested),
            still_missing: vec![],
            result: ExecResult::Ok,
            action: Action::Install,
            undoes: None,
            manager: Manager::Flatpak,
            dir: None,
        }
    }

    #[test]
    fn le_linha_v2_como_pacman() {
        let e = parse_line(LINHA_V2).unwrap();
        assert_eq!(e.version, 2);
        assert_eq!(e.manager, Manager::Pacman);
        assert_eq!(e.action, Action::Install);
    }

    #[test]
    fn v2_nao_pode_trazer_manager() {
        let linha = LINHA_V2.replace(
            r#""action":"install""#,
            r#""action":"install","manager":"pacman""#,
        );
        assert!(parse_line(&linha).is_err());
    }

    #[test]
    fn v3_exige_manager() {
        let linha = linha_valida().replace(r#","manager":"pacman""#, "");
        assert!(parse_line(&linha).is_err());
    }

    #[test]
    fn recusa_manager_desconhecido() {
        for ruim in ["apt", "", "Pacman", "FLATPAK", "snap"] {
            let linha =
                linha_valida().replace(r#""manager":"pacman""#, &format!(r#""manager":"{ruim}""#));
            assert!(
                parse_line(&linha).is_err(),
                "manager {ruim:?} deveria ser recusado"
            );
        }
    }

    #[test]
    fn linha_flatpak_tem_manager_flatpak_e_ida_e_volta() {
        let e = flatpak(&["org.gnome.Gedit"]);
        assert!(to_line(&e).ends_with(r#""action":"install","manager":"flatpak"}"#));
        assert_eq!(parse_line(&to_line(&e)).unwrap(), e);
    }

    #[test]
    fn flatpak_aceita_id_com_maiuscula_que_o_pacman_recusaria() {
        for id in [
            "org.gnome.Gedit",
            "com.valvesoftware.Steam",
            "org.nongnu.lingot",
        ] {
            assert!(parse_line(&to_line(&flatpak(&[id]))).is_ok(), "{id}");
        }
        // o mesmo nome no pacman continua recusado (maiúscula)
        let mut e = flatpak(&["org.gnome.Gedit"]);
        e.manager = Manager::Pacman;
        assert!(parse_line(&to_line(&e)).is_err());
    }

    #[test]
    fn flatpak_recusa_nome_fora_da_regra_do_flatpak() {
        for ruim in ["\u{1b}[31mx.y", "-x.y", "semponto", "a..b", "a b.c", ""] {
            assert!(
                parse_line(&to_line(&flatpak(&[ruim]))).is_err(),
                "{ruim:?} deveria ser recusado"
            );
        }
    }

    #[test]
    fn flatpak_nao_tem_remocao() {
        let mut e = flatpak(&["org.gnome.Gedit"]);
        e.action = Action::Remove;
        e.undoes = Some(1);
        assert!(parse_line(&to_line(&e)).is_err());
    }

    // --- v4: `dotfiles adopt` (ADR 0022) ---

    const DIR: &str = "/home/u/dotfiles";

    fn adocao(path: &str) -> HistoryEntry {
        entry_from_adopt(
            path,
            DIR,
            &v(&["nodusctl", "dotfiles", "adopt", "--", path]),
            1_700_000_300,
        )
    }

    /// Linha v4 como o nodus a grava.
    fn linha_v4() -> String {
        to_line(&adocao(".bashrc"))
    }

    #[test]
    fn entry_from_adopt_registra_o_caminho_adotado_e_a_pasta() {
        let e = adocao(".bashrc");
        assert_eq!(e.version, 4);
        assert_eq!(e.action, Action::Adopt);
        assert_eq!(e.manager, Manager::Dotfiles);
        assert_eq!(e.requested, v(&[".bashrc"]));
        assert_eq!(e.installed, v(&[".bashrc"]));
        assert!(e.still_missing.is_empty());
        assert_eq!(e.result, ExecResult::Ok);
        assert_eq!(e.undoes, None);
        assert_eq!(e.dir.as_deref(), Some(DIR));
        assert_eq!(e.at_unix, 1_700_000_300);
        assert_eq!(
            e.command,
            v(&["nodusctl", "dotfiles", "adopt", "--", ".bashrc"])
        );
    }

    #[test]
    fn linha_v4_tem_a_ordem_do_contrato_e_ida_e_volta() {
        let linha = linha_v4();
        assert!(
            linha.ends_with(r#""action":"adopt","manager":"dotfiles","dir":"/home/u/dotfiles"}"#),
            "{linha}"
        );
        assert!(linha.starts_with(r#"{"version":4,"#), "{linha}");
        assert_eq!(parse_line(&linha).unwrap(), adocao(".bashrc"));
        // caminho aninhado também volta igual
        let aninhado = adocao(".config/git/config");
        assert_eq!(parse_line(&to_line(&aninhado)).unwrap(), aninhado);
    }

    #[test]
    fn v1_a_v3_nao_levam_dir_e_pacman_ou_flatpak_nao_gravam_v4() {
        // `dir` só existe na v4: acrescentado ao fim de uma linha v1, v2 ou v3 válida, é recusado.
        let v3 = linha_valida();
        for linha in [LINHA_V1, LINHA_V2, v3.as_str()] {
            assert!(parse_line(linha).is_ok(), "a linha base precisa ser válida");
            let com_dir = format!(
                "{},\"dir\":\"/home/u/dotfiles\"}}",
                linha.strip_suffix('}').unwrap()
            );
            assert!(parse_line(&com_dir).is_err(), "{com_dir}");
        }
        // v4 com pacman ou flatpak
        for manager in ["pacman", "flatpak"] {
            let linha = linha_v4().replace(
                r#""manager":"dotfiles""#,
                &format!(r#""manager":"{manager}""#),
            );
            assert!(parse_line(&linha).is_err(), "{manager}");
        }
    }

    #[test]
    fn v3_nao_aceita_manager_dotfiles_nem_action_adopt() {
        let com_dotfiles =
            linha_valida().replace(r#""manager":"pacman""#, r#""manager":"dotfiles""#);
        assert!(parse_line(&com_dotfiles).is_err());
        let com_adopt = linha_valida().replace(r#""action":"install""#, r#""action":"adopt""#);
        assert!(parse_line(&com_adopt).is_err());
    }

    #[test]
    fn v4_recusa_combinacao_de_campos_invalida() {
        let casos = [
            (
                "sem dir",
                linha_v4().replace(r#","dir":"/home/u/dotfiles""#, ""),
            ),
            (
                "dir relativo",
                linha_v4().replace(r#""dir":"/home/u/dotfiles""#, r#""dir":"dotfiles""#),
            ),
            (
                "dir vazio",
                linha_v4().replace(r#""dir":"/home/u/dotfiles""#, r#""dir":"""#),
            ),
            (
                "dir com escape de terminal",
                linha_v4().replace(
                    r#""dir":"/home/u/dotfiles""#,
                    r#""dir":"/home/u/\u001b[31mx""#,
                ),
            ),
            (
                "action install",
                linha_v4().replace(r#""action":"adopt""#, r#""action":"install""#),
            ),
            (
                "action remove",
                linha_v4().replace(r#""action":"adopt""#, r#""action":"remove","undoes":1"#),
            ),
            (
                "undoes na adoção",
                linha_v4().replace(r#""action":"adopt""#, r#""action":"adopt","undoes":1"#),
            ),
            (
                "installed diferente do pedido",
                linha_v4().replacen(r#""installed":[".bashrc"]"#, r#""installed":[]"#, 1),
            ),
            (
                "still_missing preenchido",
                linha_v4().replacen(r#""still_missing":[]"#, r#""still_missing":[".bashrc"]"#, 1),
            ),
            (
                "dois caminhos no pedido",
                linha_v4()
                    .replacen(
                        r#""requested":[".bashrc"]"#,
                        r#""requested":[".bashrc",".profile"]"#,
                        1,
                    )
                    .replacen(
                        r#""installed":[".bashrc"]"#,
                        r#""installed":[".bashrc",".profile"]"#,
                        1,
                    ),
            ),
        ];
        for (nome, linha) in casos {
            assert!(parse_line(&linha).is_err(), "{nome}: {linha}");
        }
    }

    #[test]
    fn v4_valida_o_caminho_com_a_regra_dos_dotfiles() {
        // Mesma regra do relatório (ADR 0021): relativo, sem `..`, fora da denylist.
        for ruim in [
            ".ssh/config",
            "../.bashrc",
            "/etc/passwd",
            "",
            ".gnupg",
            "\u{1b}[31m.bashrc",
            ".bash_history",
        ] {
            let mut e = adocao(".bashrc");
            e.requested = v(&[ruim]);
            e.installed = v(&[ruim]);
            assert_eq!(
                parse_line(&to_line(&e)).unwrap_err(),
                HistoryError::InvalidPackageName(ruim.to_string()),
                "{ruim:?}"
            );
        }
    }

    #[test]
    fn v4_aceita_maiuscula_que_o_pacman_recusaria() {
        assert!(parse_line(&to_line(&adocao(".config/Code/User/settings.json"))).is_ok());
    }

    // --- data legível (UTC, sem crate de datas) ---

    #[test]
    fn format_utc_na_epoca() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
    }

    // ---- link de dotfile: v4 `link` + `dotfiles` + `dir` (ADR 0024) ----

    fn ligacao(path: &str) -> HistoryEntry {
        entry_from_link(
            path,
            DIR,
            &v(&["nodusctl", "dotfiles", "link", "--", path]),
            1_700_000_500,
        )
    }

    #[test]
    fn entry_from_link_registra_o_caminho_ligado_e_a_pasta() {
        let e = ligacao(".bashrc");
        assert_eq!(e.version, 4);
        assert_eq!(e.action, Action::Link);
        assert_eq!(e.manager, Manager::Dotfiles);
        assert_eq!(e.requested, v(&[".bashrc"]));
        assert_eq!(e.installed, v(&[".bashrc"]));
        assert!(e.still_missing.is_empty());
        assert_eq!(e.result, ExecResult::Ok);
        assert_eq!(e.undoes, None);
        assert_eq!(e.dir.as_deref(), Some(DIR));
        assert_eq!(e.at_unix, 1_700_000_500);
    }

    #[test]
    fn ligacao_serializa_action_link_e_faz_ida_e_volta() {
        let e = ligacao(".config/git/config");
        let linha = to_line(&e);
        assert!(linha.contains(r#""action":"link""#), "{linha}");
        assert_eq!(parse_line(&linha).unwrap(), e);
    }

    #[test]
    fn v4_link_recusa_forma_incoerente() {
        let base = || serde_json::to_value(ligacao(".bashrc")).unwrap();
        let mut dois = base();
        dois["requested"] = serde_json::json!([".bashrc", ".profile"]);
        dois["installed"] = serde_json::json!([".bashrc", ".profile"]);
        let mut sem_instalado = base();
        sem_instalado["installed"] = serde_json::json!([]);
        let mut faltando = base();
        faltando["still_missing"] = serde_json::json!([".bashrc"]);
        let mut falha = base();
        falha["result"] = serde_json::json!({"status": "failed", "code": 1});
        let mut com_undoes = base();
        com_undoes["undoes"] = serde_json::json!(1);
        let mut sem_dir = base();
        sem_dir.as_object_mut().unwrap().remove("dir");
        let mut pacman = base();
        pacman["manager"] = serde_json::json!("pacman");
        let mut dir_relativo = base();
        dir_relativo["dir"] = serde_json::json!("dotfiles");
        let mut segredo = base();
        segredo["requested"] = serde_json::json!([".ssh/config"]);
        segredo["installed"] = serde_json::json!([".ssh/config"]);
        for linha in [
            dois,
            sem_instalado,
            faltando,
            falha,
            com_undoes,
            sem_dir,
            pacman,
            dir_relativo,
            segredo,
        ] {
            assert!(parse_line(&linha.to_string()).is_err(), "{linha}");
        }
    }

    #[test]
    fn link_nas_versoes_antigas_e_recusado() {
        for versao in [1, 2, 3] {
            let mut linha = serde_json::to_value(ligacao(".bashrc")).unwrap();
            linha["version"] = serde_json::json!(versao);
            assert!(parse_line(&linha.to_string()).is_err(), "v{versao}");
        }
    }

    // ---- undo de dotfile: v4 `remove` + `undoes` + `dotfiles` (ADR 0023) ----

    fn reversao(undoes: u32, path: &str) -> HistoryEntry {
        entry_for_dotfile_undo(
            undoes,
            path,
            DIR,
            &v(&["nodusctl", "history", "undo", &undoes.to_string()]),
            1_700_000_400,
        )
    }

    #[test]
    fn entry_for_dotfile_undo_registra_a_reversao_ligada_a_adocao() {
        let e = reversao(2, ".bashrc");
        assert_eq!(e.version, 4);
        assert_eq!(e.action, Action::Remove);
        assert_eq!(e.manager, Manager::Dotfiles);
        assert_eq!(e.undoes, Some(2));
        assert_eq!(e.requested, v(&[".bashrc"]));
        assert!(e.installed.is_empty());
        assert_eq!(e.still_missing, v(&[".bashrc"]));
        assert_eq!(e.result, ExecResult::Ok);
        assert_eq!(e.dir.as_deref(), Some(DIR));
        assert_eq!(e.at_unix, 1_700_000_400);
    }

    #[test]
    fn reversao_de_dotfile_faz_ida_e_volta_pela_linha() {
        let e = reversao(1, ".config/git/config");
        assert_eq!(parse_line(&to_line(&e)).unwrap(), e);
    }

    #[test]
    fn v4_remove_exige_undoes_a_partir_de_1_manager_dotfiles_e_dir() {
        let mut sem_undoes = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        sem_undoes.as_object_mut().unwrap().remove("undoes");
        let mut zero = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        zero["undoes"] = serde_json::json!(0);
        let mut sem_dir = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        sem_dir.as_object_mut().unwrap().remove("dir");
        let mut pacman = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        pacman["manager"] = serde_json::json!("pacman");
        for linha in [sem_undoes, zero, sem_dir, pacman] {
            assert!(parse_line(&linha.to_string()).is_err(), "{linha}");
        }
    }

    #[test]
    fn v4_remove_recusa_forma_incoerente() {
        // Reversão de dotfile: um caminho, nada instalado, o caminho em still_missing, resultado ok.
        let mut dois = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        dois["requested"] = serde_json::json!([".bashrc", ".profile"]);
        dois["still_missing"] = serde_json::json!([".bashrc", ".profile"]);
        let mut instalado = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        instalado["installed"] = serde_json::json!([".bashrc"]);
        let mut falha = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        falha["result"] = serde_json::json!({"status": "failed", "code": 1});
        let mut outro = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        outro["still_missing"] = serde_json::json!([".profile"]);
        for linha in [dois, instalado, falha, outro] {
            assert!(parse_line(&linha.to_string()).is_err(), "{linha}");
        }
    }

    #[test]
    fn v4_remove_valida_o_caminho_e_o_dir() {
        let mut ssh = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        ssh["requested"] = serde_json::json!([".ssh/config"]);
        ssh["still_missing"] = serde_json::json!([".ssh/config"]);
        let mut dir_relativo = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        dir_relativo["dir"] = serde_json::json!("dotfiles");
        for linha in [ssh, dir_relativo] {
            assert!(parse_line(&linha.to_string()).is_err(), "{linha}");
        }
    }

    #[test]
    fn v3_remove_com_dotfiles_continua_recusado() {
        let mut linha = serde_json::to_value(reversao(1, ".bashrc")).unwrap();
        linha["version"] = serde_json::json!(3);
        linha.as_object_mut().unwrap().remove("dir");
        assert!(parse_line(&linha.to_string()).is_err());
    }

    // ---- reversão de Flatpak: v3 `remove` com `manager = flatpak` e `undoes` (ADR da reversão de Flatpak) ----

    fn remocao_flatpak(undoes: u32) -> HistoryEntry {
        HistoryEntry {
            version: 3,
            at_unix: 1_700_000_100,
            command: v(&["flatpak", "uninstall", "--user", "--", "org.gnome.Calculator"]),
            requested: v(&["org.gnome.Calculator"]),
            installed: vec![],
            still_missing: v(&["org.gnome.Calculator"]),
            result: ExecResult::Ok,
            action: Action::Remove,
            undoes: Some(undoes),
            manager: Manager::Flatpak,
            dir: None,
        }
    }

    #[test]
    fn linha_de_remocao_flatpak_tem_action_undoes_e_manager_no_fim() {
        assert_eq!(
            to_line(&remocao_flatpak(2)),
            r#"{"version":3,"at_unix":1700000100,"command":["flatpak","uninstall","--user","--","org.gnome.Calculator"],"requested":["org.gnome.Calculator"],"installed":[],"still_missing":["org.gnome.Calculator"],"result":{"status":"ok"},"action":"remove","undoes":2,"manager":"flatpak"}"#
        );
    }

    #[test]
    fn ida_e_volta_de_remocao_flatpak() {
        let e = remocao_flatpak(7);
        assert_eq!(parse_line(&to_line(&e)).unwrap(), e);
    }

    #[test]
    fn v3_remove_flatpak_exige_undoes_a_partir_de_1() {
        let mut sem = serde_json::to_value(remocao_flatpak(1)).unwrap();
        sem.as_object_mut().unwrap().remove("undoes");
        assert!(parse_line(&sem.to_string()).is_err(), "remove sem undoes");
        let mut zero = serde_json::to_value(remocao_flatpak(1)).unwrap();
        zero["undoes"] = serde_json::json!(0);
        assert!(parse_line(&zero.to_string()).is_err(), "undoes = 0");
    }

    #[test]
    fn v3_remove_flatpak_valida_o_id_pela_regra_do_flatpak() {
        for id in ["-x.y", "org..B", "sl", "org.x.A b", "org.x.A\u{1b}[31m", ""] {
            let mut e = remocao_flatpak(1);
            e.requested = v(&[id]);
            assert!(parse_line(&to_line(&e)).is_err(), "id {id:?}");
        }
        // Maiúsculas e hífen são válidos num ID Flatpak.
        let mut e = remocao_flatpak(1);
        e.requested = v(&["org.Gnome.My-App_2"]);
        e.still_missing = v(&["org.Gnome.My-App_2"]);
        assert!(parse_line(&to_line(&e)).is_ok());
    }

    #[test]
    fn v3_remove_flatpak_nao_leva_dir_e_dotfiles_continua_recusado() {
        let mut com_dir = serde_json::to_value(remocao_flatpak(1)).unwrap();
        com_dir["dir"] = serde_json::json!("/home/u/dotfiles");
        assert!(parse_line(&com_dir.to_string()).is_err(), "dir na v3");
        let mut dotfiles = serde_json::to_value(remocao_flatpak(1)).unwrap();
        dotfiles["manager"] = serde_json::json!("dotfiles");
        assert!(parse_line(&dotfiles.to_string()).is_err(), "manager dotfiles na v3");
    }

    #[test]
    fn v3_install_com_undoes_continua_recusado_para_flatpak() {
        let mut e = serde_json::to_value(remocao_flatpak(1)).unwrap();
        e["action"] = serde_json::json!("install");
        assert!(parse_line(&e.to_string()).is_err(), "install não leva undoes");
    }

    #[test]
    fn format_utc_datas_conhecidas() {
        assert_eq!(format_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(format_utc(1_794_626_444), "2026-11-14T03:20:44Z");
    }

    #[test]
    fn format_utc_dia_bissexto_e_virada_de_seculo() {
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_utc(4_102_444_799), "2099-12-31T23:59:59Z");
        assert_eq!(format_utc(4_102_444_800), "2100-01-01T00:00:00Z");
    }
}
