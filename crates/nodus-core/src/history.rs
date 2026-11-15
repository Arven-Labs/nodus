//! Histórico do que o nodus executou (uma entrada por `preset apply`). Puro, sem I/O: quem lê e
//! grava o arquivo é a CLI. O histórico é entrada externa na leitura (SECURITY.md T5): a linha
//! é estrita e recusada em vez de interpretada.

use crate::preset::{Plan, is_valid_flatpak_id, is_valid_pacman_name};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Versão do formato que o nodus grava. Mudar o formato exige subir este número. A leitura aceita
/// também a versão 1 (só instalações, sem `action` nem `undoes`) e a 2 (sem `manager`, sempre
/// pacman).
pub const ENTRY_VERSION: u32 = 3;

/// O que a entrada registra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// `preset apply`: instalação.
    Install,
    /// `history undo`: reversão de uma instalação anterior.
    Remove,
}

/// Qual gerenciador a entrada registra. Define a regra de validação dos nomes (pacote do pacman ou
/// ID de app Flatpak). Entradas v1 e v2 não têm o campo e são sempre `Pacman`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Manager {
    Pacman,
    Flatpak,
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
    /// A partir da v3. A leitura de v1 e v2 preenche `Pacman`; a escrita só grava v3.
    pub manager: Manager,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HistoryError {
    #[error("linha de histórico inválida: {0}")]
    Invalid(String),
    #[error("versão {0} do histórico não é suportada (esta versão lê da 1 à {ENTRY_VERSION})")]
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
    }
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
    let (action, undoes, manager) = match (raw.version, raw.action, raw.undoes, raw.manager) {
        (1, None, None, None) => (Action::Install, None, Manager::Pacman),
        (1, ..) => {
            return Err(invalid(
                "a versão 1 não tem `action`, `undoes` nem `manager`",
            ));
        }
        (2, Some(Action::Install), None, None) => (Action::Install, None, Manager::Pacman),
        (2, Some(Action::Remove), Some(n), None) if n >= 1 => {
            (Action::Remove, Some(n), Manager::Pacman)
        }
        (2, ..) => {
            return Err(invalid(
                "`install` não leva `undoes`; `remove` exige `undoes` a partir de 1; a versão 2 não tem `manager`",
            ));
        }
        (3, Some(Action::Install), None, Some(manager)) => (Action::Install, None, manager),
        (3, Some(Action::Remove), Some(n), Some(Manager::Pacman)) if n >= 1 => {
            (Action::Remove, Some(n), Manager::Pacman)
        }
        (3, ..) => {
            return Err(invalid(
                "a versão 3 exige `manager`; `install` não leva `undoes`; `remove` exige `undoes` a partir de 1 e só existe no pacman",
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
    };
    // Os nomes vão para a tela: nada de escape de terminal nem de opção do gerenciador.
    let is_valid_name: fn(&str) -> bool = match entry.manager {
        Manager::Pacman => is_valid_pacman_name,
        Manager::Flatpak => is_valid_flatpak_id,
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
        for ruim in [0, 4, 99] {
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

    // --- data legível (UTC, sem crate de datas) ---

    #[test]
    fn format_utc_na_epoca() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
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
