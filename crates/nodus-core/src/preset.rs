//! Preset declarativo de pacotes (fatia 1: só leitura). O preset é entrada não confiável (T1, T7).

use crate::inventory::{FlatpakApp, InventoryError, Package, parse_pacman_q, read_flatpak_apps};
use crate::runner::{CommandRunner, RunnerError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

/// Único formato aceito hoje. Outro número é recusado, nunca "adivinhado".
const FORMAT_VERSION: i64 = 1;

/// Estado desejado. Só existe passando por `parse_preset` ou por `preset_from_machine`, que validam
/// todos os nomes: na fatia 2 eles viram argumentos do `pacman`, e um `-Rns` seria lido como opção.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    pub pacman: Vec<String>,
    pub flatpak: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PresetError {
    #[error("preset inválido: {0}")]
    Toml(String),
    #[error("formato de preset {0} não suportado (este nodus só lê o formato {FORMAT_VERSION})")]
    UnsupportedVersion(i64),
    #[error("nome inválido no item {number} de `{list}`: {name:?}")]
    InvalidName {
        list: &'static str,
        number: usize,
        name: String,
    },
    #[error("nome repetido em `{list}`: {name:?}")]
    Duplicate { list: &'static str, name: String },
    #[error(
        "rótulo de máquina inválido {0:?}: use 1 a 63 caracteres de [a-z0-9._-], começando por letra ou dígito"
    )]
    InvalidMachineLabel(String),
    #[error(
        "`{list}` da máquina {machine:?} cita {name:?}, que não está na lista principal do preset"
    )]
    SkipNotInPreset {
        machine: String,
        list: &'static str,
        name: String,
    },
    #[error(
        "`{list}` da máquina {machine:?} cita {name:?}, que já está na lista principal do preset"
    )]
    AddAlreadyInPreset {
        machine: String,
        list: &'static str,
        name: String,
    },
    #[error("não consegui acrescentar ao preset sem alterar o resto: {0}")]
    Edit(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MachineError {
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Preset(#[from] PresetError),
}

/// Preset lido da máquina, mais o que o `init` precisa avisar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachinePreset {
    pub preset: Preset,
    pub flatpak_installed: bool,
}

/// O que faltaria instalar para a máquina ficar como o preset. Nada aqui executa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    /// O que falta e o pacman consegue instalar (só isto entra no comando do `apply`).
    pub pacman_missing: Vec<String>,
    pub pacman_present: usize,
    /// O que falta e o pacman não encontra nos repositórios (AUR, locais): o nodus não instala. Um
    /// item assim aborta a transação inteira do `pacman -S`, por isso fica fora de `pacman_missing`.
    pub pacman_unavailable: Vec<String>,
    pub flatpak_missing: Vec<String>,
    pub flatpak_present: usize,
    pub flatpak_installed: bool,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.pacman_missing.is_empty() && self.flatpak_missing.is_empty()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPreset {
    version: i64,
    #[serde(default)]
    pacman: Vec<String>,
    #[serde(default)]
    flatpak: Vec<String>,
    #[serde(default)]
    machines: BTreeMap<String, RawMachine>,
}

/// `[machines.<rótulo>]`: o que o preset compartilhado não vale para aquela máquina (`skip_*`) e o
/// que só ela quer a mais (`add_*`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMachine {
    #[serde(default)]
    skip_pacman: Vec<String>,
    #[serde(default)]
    skip_flatpak: Vec<String>,
    #[serde(default)]
    add_pacman: Vec<String>,
    #[serde(default)]
    add_flatpak: Vec<String>,
}

/// Itens por gerenciador: o que uma máquina pula ou acrescenta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Items {
    pub pacman: Vec<String>,
    pub flatpak: Vec<String>,
}

impl Items {
    pub fn is_empty(&self) -> bool {
        self.pacman.is_empty() && self.flatpak.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachineChanges {
    skip: Items,
    add: Items,
}

/// O preset como está no arquivo: as listas compartilhadas mais as exceções por máquina.
/// `for_machine` entrega o preset efetivo de uma máquina, e é ele que `plan`, `suggest` e `status`
/// comparam: nenhum deles precisa saber que existem exceções.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetFile {
    pub preset: Preset,
    machines: BTreeMap<String, MachineChanges>,
}

impl PresetFile {
    pub fn has_machines(&self) -> bool {
        !self.machines.is_empty()
    }

    /// O que a máquina `label` pula (vazio se não há tabela para ela; o rótulo é comparado inteiro).
    pub fn skipped(&self, label: &str) -> Items {
        self.machines
            .get(label)
            .map(|changes| changes.skip.clone())
            .unwrap_or_default()
    }

    /// O que a máquina `label` acrescenta (vazio se não há tabela para ela).
    pub fn added(&self, label: &str) -> Items {
        self.machines
            .get(label)
            .map(|changes| changes.add.clone())
            .unwrap_or_default()
    }

    /// O preset da máquina `label`: a lista principal sem o que ela pula, seguida do que ela
    /// acrescenta (o parse garante que nenhum acrescentado já está na lista principal).
    pub fn for_machine(&self, label: &str) -> Preset {
        let (skipped, added) = (self.skipped(label), self.added(label));
        let effective = |names: &[String], skip: &[String], add: &[String]| -> Vec<String> {
            names
                .iter()
                .filter(|name| !skip.contains(name))
                .chain(add)
                .cloned()
                .collect()
        };
        Preset {
            pacman: effective(&self.preset.pacman, &skipped.pacman, &added.pacman),
            flatpak: effective(&self.preset.flatpak, &skipped.flatpak, &added.flatpak),
        }
    }
}

/// Preset completo, sem as exceções por máquina (quem edita o arquivo, como o `adopt`, vê tudo).
pub fn parse_preset(input: &str) -> Result<Preset, PresetError> {
    Ok(parse_preset_file(input)?.preset)
}

pub fn parse_preset_file(input: &str) -> Result<PresetFile, PresetError> {
    let raw: RawPreset = toml::from_str(input).map_err(|e| PresetError::Toml(e.to_string()))?;
    if raw.version != FORMAT_VERSION {
        return Err(PresetError::UnsupportedVersion(raw.version));
    }
    validate("pacman", &raw.pacman, is_valid_pacman_name)?;
    validate("flatpak", &raw.flatpak, is_valid_flatpak_id)?;
    let mut machines = BTreeMap::new();
    for (label, machine) in raw.machines {
        crate::snapshot::validate_name(&label)
            .map_err(|_| PresetError::InvalidMachineLabel(label.clone()))?;
        validate("skip_pacman", &machine.skip_pacman, is_valid_pacman_name)?;
        validate("skip_flatpak", &machine.skip_flatpak, is_valid_flatpak_id)?;
        validate("add_pacman", &machine.add_pacman, is_valid_pacman_name)?;
        validate("add_flatpak", &machine.add_flatpak, is_valid_flatpak_id)?;
        // Um pulo que não bate com nenhum item é quase sempre typo: erro, não "não faz nada".
        for (list, skip, shared) in [
            ("skip_pacman", &machine.skip_pacman, &raw.pacman),
            ("skip_flatpak", &machine.skip_flatpak, &raw.flatpak),
        ] {
            if let Some(name) = skip.iter().find(|name| !shared.contains(name)) {
                return Err(PresetError::SkipNotInPreset {
                    machine: label,
                    list,
                    name: name.clone(),
                });
            }
        }
        // Acrescentar o que já está na lista principal é redundância ou engano (inclusive pular e
        // acrescentar o mesmo item): erro, para o efetivo ter uma só leitura possível.
        for (list, add, shared) in [
            ("add_pacman", &machine.add_pacman, &raw.pacman),
            ("add_flatpak", &machine.add_flatpak, &raw.flatpak),
        ] {
            if let Some(name) = add.iter().find(|name| shared.contains(name)) {
                return Err(PresetError::AddAlreadyInPreset {
                    machine: label,
                    list,
                    name: name.clone(),
                });
            }
        }
        machines.insert(
            label,
            MachineChanges {
                skip: Items {
                    pacman: machine.skip_pacman,
                    flatpak: machine.skip_flatpak,
                },
                add: Items {
                    pacman: machine.add_pacman,
                    flatpak: machine.add_flatpak,
                },
            },
        );
    }
    Ok(PresetFile {
        preset: Preset {
            pacman: raw.pacman,
            flatpak: raw.flatpak,
        },
        machines,
    })
}

fn validate(
    list: &'static str,
    names: &[String],
    is_valid: fn(&str) -> bool,
) -> Result<(), PresetError> {
    let mut seen = HashSet::new();
    for (index, name) in names.iter().enumerate() {
        if !is_valid(name) {
            return Err(PresetError::InvalidName {
                list,
                number: index + 1,
                name: name.clone(),
            });
        }
        if !seen.insert(name.as_str()) {
            return Err(PresetError::Duplicate {
                list,
                name: name.clone(),
            });
        }
    }
    Ok(())
}

/// Regra de nome do pacman (`PKGBUILD(5)`): `[a-z0-9@._+-]`, sem começar por `-` ou `.`.
pub fn is_valid_pacman_name(name: &str) -> bool {
    let allowed = |c: char| matches!(c, 'a'..='z' | '0'..='9' | '@' | '.' | '_' | '+' | '-');
    !name.is_empty() && !name.starts_with(['-', '.']) && name.chars().all(allowed)
}

/// ID de app Flatpak: segmentos `[A-Za-z0-9_-]` separados por `.`, pelo menos 2, nenhum vazio
/// nem começando por `-`. O Flatpak pode exigir mais (3 segmentos); aqui basta barrar injeção.
pub fn is_valid_flatpak_id(id: &str) -> bool {
    let allowed = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    let segment_ok = |s: &str| !s.is_empty() && !s.starts_with('-') && s.chars().all(allowed);
    id.split('.').count() >= 2 && id.split('.').all(segment_ok)
}

/// As listas do preset com a posição (em bytes) de cada valor no texto, para editar sem reescrever.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpannedPreset {
    #[allow(dead_code)] // só confere o formato; quem valida a versão é `parse_preset`
    version: i64,
    pacman: Option<toml::Spanned<Vec<toml::Spanned<String>>>>,
    flatpak: Option<toml::Spanned<Vec<toml::Spanned<String>>>>,
    /// As exceções por máquina não são editadas aqui; só precisam ser aceitas.
    #[allow(dead_code)]
    machines: Option<toml::Value>,
}

/// Acrescenta nomes ao texto de um preset **sem mexer em mais nada**: comentários, ordem, linhas em
/// branco e formatação ficam byte a byte (só o trecho inserido muda). Nome já presente é ignorado e
/// repetido entre os novos entra uma vez; sem nada a acrescentar devolve o texto igual. Uma lista
/// que não existe é criada no fim. O resultado é reparseado e precisa ter os nomes antigos seguidos
/// dos novos, senão é erro e nada é devolvido (o chamador não grava um preset que não confere).
pub fn add_to_preset(
    text: &str,
    pacman: &[String],
    flatpak: &[String],
) -> Result<String, PresetError> {
    let before = parse_preset(text)?;
    let new_pacman = only_new(pacman, &before.pacman);
    let new_flatpak = only_new(flatpak, &before.flatpak);
    validate("pacman", &new_pacman, is_valid_pacman_name)?;
    validate("flatpak", &new_flatpak, is_valid_flatpak_id)?;
    if new_pacman.is_empty() && new_flatpak.is_empty() {
        return Ok(text.to_string());
    }

    let raw: SpannedPreset = toml::from_str(text).map_err(|e| PresetError::Toml(e.to_string()))?;
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut edits: Vec<(usize, String)> = Vec::new();
    let mut appended = String::new();
    for (key, array, new) in [
        ("pacman", raw.pacman, &new_pacman),
        ("flatpak", raw.flatpak, &new_flatpak),
    ] {
        if new.is_empty() {
            continue;
        }
        match array {
            Some(array) => {
                let last_end = array.get_ref().last().map(|item| item.span().end);
                edits.extend(insert_into_array(text, array.span(), last_end, new, eol)?);
            }
            None => {
                appended.push_str(&format!("{key} = [{eol}"));
                for name in new {
                    appended.push_str(&format!("    \"{name}\",{eol}"));
                }
                appended.push_str(&format!("]{eol}"));
            }
        }
    }
    // Do fim para o começo: inserir antes não desloca as posições que ainda faltam (a ordenação é
    // estável, então duas inserções na mesma posição ficam na ordem em que foram pedidas).
    edits.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
    let mut out = text.to_string();
    for (position, inserted) in edits {
        out.insert_str(position, &inserted);
    }
    if !appended.is_empty() {
        if !out.ends_with('\n') {
            out.push_str(eol);
        }
        out.push_str(&appended);
    }

    let after = parse_preset(&out).map_err(|e| PresetError::Edit(e.to_string()))?;
    let expected = |old: &[String], new: &[String]| -> Vec<String> {
        old.iter().chain(new).cloned().collect()
    };
    if after.pacman != expected(&before.pacman, &new_pacman)
        || after.flatpak != expected(&before.flatpak, &new_flatpak)
    {
        return Err(PresetError::Edit(
            "o texto resultante não tem os mesmos nomes que o esperado".into(),
        ));
    }
    Ok(out)
}

/// Os nomes de `wanted` que ainda não estão em `present`, sem repetir, na ordem pedida.
fn only_new(wanted: &[String], present: &[String]) -> Vec<String> {
    let mut seen: HashSet<&str> = present.iter().map(String::as_str).collect();
    wanted
        .iter()
        .filter(|name| seen.insert(name.as_str()))
        .cloned()
        .collect()
}

/// As inserções (posição, texto) que põem `new` numa lista cujo `[` e `]` estão em `span`.
/// `last_end` é onde termina o último elemento, se houver. Os nomes já foram validados (sem aspas
/// nem barra invertida), então vão entre aspas sem escapar.
fn insert_into_array(
    text: &str,
    span: std::ops::Range<usize>,
    last_end: Option<usize>,
    new: &[String],
    eol: &str,
) -> Result<Vec<(usize, String)>, PresetError> {
    let open = span.start;
    let close = span.end.saturating_sub(1);
    if text.get(open..=open) != Some("[") || text.get(close..=close) != Some("]") {
        return Err(PresetError::Edit("não achei os colchetes da lista".into()));
    }
    let line_start = text[..close].rfind('\n').map_or(0, |index| index + 1);
    let closes_on_its_own_line = text[line_start..close].trim().is_empty();
    let mut edits = Vec::new();
    if closes_on_its_own_line {
        // Uma linha por nome, logo antes da linha do `]`. O último elemento precisa de vírgula.
        if let Some(end) = last_end
            && next_significant(&text[end..close]) != Some(',')
        {
            edits.push((end, ",".to_string()));
        }
        let lines: String = new
            .iter()
            .map(|name| format!("    \"{name}\",{eol}"))
            .collect();
        edits.push((line_start, lines));
    } else if let Some(end) = last_end {
        // Na mesma linha: `, "nome"` depois do último elemento (uma vírgula final que já exista
        // continua depois do que entrou, o que o TOML aceita).
        let names: String = new.iter().map(|name| format!(", \"{name}\"")).collect();
        edits.push((end, names));
    } else {
        let names: Vec<String> = new.iter().map(|name| format!("\"{name}\"")).collect();
        edits.push((open + 1, names.join(", ")));
    }
    Ok(edits)
}

/// O primeiro caractere que não é espaço nem comentário (`#` até o fim da linha).
fn next_significant(text: &str) -> Option<char> {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '#' {
            for skipped in chars.by_ref() {
                if skipped == '\n' {
                    break;
                }
            }
        } else if !c.is_whitespace() {
            return Some(c);
        }
    }
    None
}

/// Texto TOML de um preset, um nome por linha (fácil de editar e de ver no diff). Os nomes já foram
/// validados e não têm aspas nem barra invertida, então podem ir entre aspas sem escapar.
pub fn to_toml(preset: &Preset) -> String {
    let mut out = format!(
        "# Preset do nodus (formato {FORMAT_VERSION}). Gerado por `nodusctl preset init`; edite à vontade.\nversion = {FORMAT_VERSION}\n"
    );
    for (key, names) in [("pacman", &preset.pacman), ("flatpak", &preset.flatpak)] {
        out.push_str(key);
        out.push_str(" = [\n");
        for name in names {
            out.push_str(&format!("    \"{name}\",\n"));
        }
        out.push_str("]\n");
    }
    out
}

/// Preset com o que foi instalado de propósito (`pacman -Qe`) e os apps Flatpak.
pub fn preset_from_machine(runner: &dyn CommandRunner) -> Result<MachinePreset, MachineError> {
    let explicit = read_pacman(runner, "-Qe")?;
    let apps = read_flatpak_apps(runner)?;
    let pacman: Vec<String> = explicit.into_iter().map(|p| p.name).collect();
    let flatpak: Vec<String> = apps
        .iter()
        .flatten()
        .map(|a| a.application.clone())
        .collect();
    // O que veio da máquina também é entrada externa: passa pela mesma validação do arquivo.
    validate("pacman", &pacman, is_valid_pacman_name)?;
    validate("flatpak", &flatpak, is_valid_flatpak_id)?;
    Ok(MachinePreset {
        preset: Preset { pacman, flatpak },
        flatpak_installed: apps.is_some(),
    })
}

/// Compara o desejado com o instalado. `flatpak = None`: Flatpak não instalado, então todo app do
/// preset falta. Pacote instalado fora do preset não entra: o plano nunca propõe remover.
pub fn plan(preset: &Preset, installed: &[Package], flatpak: Option<&[FlatpakApp]>) -> Plan {
    let installed: HashSet<&str> = installed.iter().map(|p| p.name.as_str()).collect();
    let apps: HashSet<&str> = flatpak
        .unwrap_or_default()
        .iter()
        .map(|a| a.application.as_str())
        .collect();
    let (pacman_missing, pacman_present) = split_missing(&preset.pacman, &installed);
    let (flatpak_missing, flatpak_present) = split_missing(&preset.flatpak, &apps);
    Plan {
        pacman_missing,
        pacman_present,
        pacman_unavailable: vec![],
        flatpak_missing,
        flatpak_present,
        flatpak_installed: flatpak.is_some(),
    }
}

fn split_missing(wanted: &[String], have: &HashSet<&str>) -> (Vec<String>, usize) {
    let missing: Vec<String> = wanted
        .iter()
        .filter(|name| !have.contains(name.as_str()))
        .cloned()
        .collect();
    let present = wanted.len() - missing.len();
    (missing, present)
}

/// Comando exato para instalar o que falta do pacman, ou `None` se não falta nada. `--needed` pula o
/// que já está instalado; `--` impede que um nome vire opção (além da validação). Nunca `--noconfirm`
/// (o `[Y/n]` do pacman mostra as dependências) nem `-y` (`-Sy` sem `-u` é atualização parcial).
pub fn pacman_install_command(plan: &Plan) -> Option<Vec<String>> {
    if plan.pacman_missing.is_empty() {
        return None;
    }
    let mut command: Vec<String> = ["sudo", "pacman", "-S", "--needed", "--"]
        .map(String::from)
        .to_vec();
    command.extend(plan.pacman_missing.iter().cloned());
    Some(command)
}

/// O que está instalado de propósito nesta máquina e fora do preset (critério 5 do MVP, fatia só
/// relatório). Nada aqui executa nem escreve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestions {
    pub pacman_extra: Vec<String>,
    /// Explícitos que o pacman marca como de fora dos repositórios (`pacman -Qem`, AUR ou locais):
    /// colados no preset, o `apply` falharia, porque `pacman -S` só enxerga os repositórios.
    pub foreign_extra: Vec<String>,
    pub flatpak_extra: Vec<String>,
    pub flatpak_installed: bool,
}

impl Suggestions {
    pub fn is_empty(&self) -> bool {
        self.pacman_extra.is_empty()
            && self.foreign_extra.is_empty()
            && self.flatpak_extra.is_empty()
    }
}

/// Diferença entre o que a máquina tem de propósito e o preset, na ordem da máquina. Só devolve nomes
/// que vieram de `machine` (já validados em `preset_from_machine`): `foreign` só classifica.
pub fn suggest(preset: &Preset, machine: &MachinePreset, foreign: &[String]) -> Suggestions {
    let in_pacman: HashSet<&str> = preset.pacman.iter().map(String::as_str).collect();
    let in_flatpak: HashSet<&str> = preset.flatpak.iter().map(String::as_str).collect();
    let foreign: HashSet<&str> = foreign.iter().map(String::as_str).collect();
    let (foreign_extra, pacman_extra): (Vec<String>, Vec<String>) = machine
        .preset
        .pacman
        .iter()
        .filter(|name| !in_pacman.contains(name.as_str()))
        .cloned()
        .partition(|name| foreign.contains(name.as_str()));
    let flatpak_extra = machine
        .preset
        .flatpak
        .iter()
        .filter(|id| !in_flatpak.contains(id.as_str()))
        .cloned()
        .collect();
    Suggestions {
        pacman_extra,
        foreign_extra,
        flatpak_extra,
        flatpak_installed: machine.flatpak_installed,
    }
}

/// Tira das sugestões o que o dono mandou ignorar (critério 5 do MVP). `ignore_pacman` vale para as
/// duas listas do pacman (de dentro e de fora dos repositórios) e `ignore_flatpak` para a do Flatpak;
/// um nome ignorado que não está nas sugestões não faz nada. Devolve o que ficou visível e os nomes
/// escondidos, na ordem pacman, fora dos repositórios, Flatpak. Pura: o config é da CLI.
pub fn hide_ignored(
    suggestions: &Suggestions,
    ignore_pacman: &[String],
    ignore_flatpak: &[String],
) -> (Suggestions, Vec<String>) {
    fn split(names: &[String], ignored: &[String], hidden: &mut Vec<String>) -> Vec<String> {
        let mut kept = Vec::new();
        for name in names {
            if ignored.contains(name) {
                hidden.push(name.clone());
            } else {
                kept.push(name.clone());
            }
        }
        kept
    }
    let mut hidden = Vec::new();
    let visible = Suggestions {
        pacman_extra: split(&suggestions.pacman_extra, ignore_pacman, &mut hidden),
        foreign_extra: split(&suggestions.foreign_extra, ignore_pacman, &mut hidden),
        flatpak_extra: split(&suggestions.flatpak_extra, ignore_flatpak, &mut hidden),
        flatpak_installed: suggestions.flatpak_installed,
    };
    (visible, hidden)
}

/// Sugestões contra a máquina: lê `pacman -Qe`, `pacman -Qem` e `flatpak list`. Dependências ficam de
/// fora porque `-Qe` só lista o que foi instalado de propósito (o mesmo critério do `preset init`).
pub fn collect_suggestions(
    runner: &dyn CommandRunner,
    preset: &Preset,
) -> Result<Suggestions, MachineError> {
    let machine = preset_from_machine(runner)?;
    let foreign: Vec<String> = read_pacman(runner, "-Qem")?
        .into_iter()
        .map(|package| package.name)
        .collect();
    Ok(suggest(preset, &machine, &foreign))
}

/// Remote fixo da instalação Flatpak (ADR 0015). Outros remotes ficam fora do MVP.
const FLATHUB: &str = "flathub";

/// Comando exato para instalar os apps Flatpak que faltam, ou `None` se não falta nenhum. `--user`
/// (sem privilégio), `--` antes do remote e dos IDs (nenhum ID vira opção, T7; aceito pelo Flatpak,
/// medido em 2026-11-15). Nunca `-y` nem `--noninteractive`: o prompt do Flatpak mostra runtimes e
/// dependências, e pode perguntar mais de uma vez.
pub fn flatpak_install_command(plan: &Plan) -> Option<Vec<String>> {
    if plan.flatpak_missing.is_empty() {
        return None;
    }
    let mut command: Vec<String> = ["flatpak", "install", "--user", "--", FLATHUB]
        .map(String::from)
        .to_vec();
    command.extend(plan.flatpak_missing.iter().cloned());
    Some(command)
}

/// O remote `flathub` existe na instalação do usuário? Nome exato, uma linha por remote. Sem
/// `flatpak` no sistema não há remote: `false`, não erro.
pub fn flathub_configured(runner: &dyn CommandRunner) -> Result<bool, InventoryError> {
    match runner.run("flatpak", &["remotes", "--user", "--columns=name"]) {
        Ok(output) => Ok(output.lines().any(|line| line.trim() == FLATHUB)),
        Err(RunnerError::NotFound { .. }) => Ok(false),
        Err(erro) => Err(erro.into()),
    }
}

/// Plano contra a máquina. Usa `pacman -Q` (todos), não `-Qe`: pacote que veio como dependência
/// também está presente, e contá-lo como faltante quebraria a idempotência.
pub fn collect_plan(runner: &dyn CommandRunner, preset: &Preset) -> Result<Plan, MachineError> {
    let installed = read_pacman(runner, "-Q")?;
    let apps = read_flatpak_apps(runner)?;
    let mut result = plan(preset, &installed, apps.as_deref());
    let missing = std::mem::take(&mut result.pacman_missing);
    let (installable, unavailable) = split_unavailable(runner, missing)?;
    result.pacman_missing = installable;
    result.pacman_unavailable = unavailable;
    Ok(result)
}

/// Separa o que o pacman consegue instalar do que não encontra, simulando com `pacman -S --print
/// --needed` (sem root, não instala nada; é a mesma resolução do `apply` real, então vale também para
/// grupos e *provides*, que uma lista de pacotes erraria). Caminho comum: uma única chamada com o lote
/// inteiro. Se ela falhar, uma chamada por item, porque o nome do culpado só sai no stderr e o
/// `CommandRunner` devolve só o stdout. Qualquer falha de um item sozinho vira "indisponível" (não
/// distingo "não encontrado" de outro erro com o mesmo exit 1); erro de início (sem `pacman`) propaga.
fn split_unavailable(
    runner: &dyn CommandRunner,
    missing: Vec<String>,
) -> Result<(Vec<String>, Vec<String>), InventoryError> {
    if missing.is_empty() {
        return Ok((missing, vec![]));
    }
    match simulate_install(runner, &missing) {
        Ok(()) => return Ok((missing, vec![])),
        Err(RunnerError::Failed { .. }) => {}
        Err(other) => return Err(other.into()),
    }
    let (mut installable, mut unavailable) = (vec![], vec![]);
    for name in missing {
        match simulate_install(runner, std::slice::from_ref(&name)) {
            Ok(()) => installable.push(name),
            Err(RunnerError::Failed { .. }) => unavailable.push(name),
            Err(other) => return Err(other.into()),
        }
    }
    Ok((installable, unavailable))
}

fn simulate_install(runner: &dyn CommandRunner, names: &[String]) -> Result<(), RunnerError> {
    let mut args = vec!["-S", "--print", "--needed", "--"];
    args.extend(names.iter().map(String::as_str));
    runner.run("pacman", &args).map(|_| ())
}

/// `pacman -Q` e `-Qe` saem no mesmo formato `nome versão` (medido em 2026-11-13).
fn read_pacman(runner: &dyn CommandRunner, query: &str) -> Result<Vec<Package>, InventoryError> {
    Ok(parse_pacman_q(&runner.run("pacman", &[query])?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{FlatpakApp, Package};
    use crate::runner::{CommandRunner, RunnerError};

    fn pacote(nome: &str) -> Package {
        Package {
            name: nome.into(),
            version: "1.0-1".into(),
        }
    }

    fn app(id: &str) -> FlatpakApp {
        FlatpakApp {
            application: id.into(),
            version: None,
            branch: "stable".into(),
            origin: "flathub".into(),
            installation: "user".into(),
        }
    }

    fn preset(pacman: &[&str], flatpak: &[&str]) -> Preset {
        parse_preset(&format!(
            "version = 1\npacman = {pacman:?}\nflatpak = {flatpak:?}\n"
        ))
        .unwrap()
    }

    // --- parse_preset ---

    #[test]
    fn le_um_preset_valido() {
        let p = parse_preset(
            "# meu preset\nversion = 1\npacman = [\"base\", \"git\"]\nflatpak = [\"org.nongnu.lingot\"]\n",
        )
        .unwrap();
        assert_eq!(p.pacman, ["base", "git"]);
        assert_eq!(p.flatpak, ["org.nongnu.lingot"]);
    }

    #[test]
    fn listas_ausentes_viram_vazias() {
        let p = parse_preset("version = 1\n").unwrap();
        assert!(p.pacman.is_empty());
        assert!(p.flatpak.is_empty());
    }

    #[test]
    fn version_ausente_e_erro() {
        assert!(matches!(
            parse_preset("pacman = [\"git\"]\n"),
            Err(PresetError::Toml(_))
        ));
    }

    #[test]
    fn version_diferente_de_1_e_erro() {
        assert_eq!(
            parse_preset("version = 2\n"),
            Err(PresetError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn campo_desconhecido_e_erro() {
        assert!(matches!(
            parse_preset("version = 1\npacmna = [\"git\"]\n"),
            Err(PresetError::Toml(_))
        ));
    }

    #[test]
    fn toml_quebrado_e_erro() {
        assert!(matches!(
            parse_preset("version = 1\npacman = [\"git\"\n"),
            Err(PresetError::Toml(_))
        ));
    }

    #[test]
    fn nomes_de_pacman_aceitos() {
        let nomes = [
            "7zip",
            "lib32-glibc",
            "gtk+3",
            "python3.14",
            "foo@bar",
            "a_b",
        ];
        assert_eq!(preset(&nomes, &[]).pacman, nomes);
    }

    #[test]
    fn nome_de_pacman_que_vira_opcao_ou_injecao_e_recusado() {
        let recusados = [
            // Só minúsculas e hífen: o único motivo de recusa é o `-` no começo (viraria opção).
            "--noconfirm",
            "-rdd",
            "-Rns",
            "--overwrite=*",
            ".oculto",
            "a;b",
            "a b",
            "Git",
            "",
            "a\nb",
            "a/b",
        ];
        for nome in recusados {
            let toml = format!("version = 1\npacman = [\"ok\", {nome:?}]\n");
            assert_eq!(
                parse_preset(&toml),
                Err(PresetError::InvalidName {
                    list: "pacman",
                    number: 2,
                    name: nome.to_string(),
                }),
                "{nome:?} deveria ser recusado"
            );
        }
    }

    #[test]
    fn ids_de_flatpak_aceitos() {
        let ids = ["org.nongnu.lingot", "com.github.foo_bar.App-1", "io.x"];
        assert_eq!(preset(&[], &ids).flatpak, ids);
    }

    #[test]
    fn id_de_flatpak_invalido_e_recusado() {
        let recusados = [
            "lingot", "org..x", ".org.x", "org.x.", "-org.x", "org.-x", "org.x;y", "org.x y",
            "org/x.y", "",
        ];
        for id in recusados {
            let toml = format!("version = 1\nflatpak = [{id:?}]\n");
            assert_eq!(
                parse_preset(&toml),
                Err(PresetError::InvalidName {
                    list: "flatpak",
                    number: 1,
                    name: id.to_string(),
                }),
                "{id:?} deveria ser recusado"
            );
        }
    }

    #[test]
    fn nome_repetido_e_erro() {
        assert_eq!(
            parse_preset("version = 1\npacman = [\"git\", \"vim\", \"git\"]\n"),
            Err(PresetError::Duplicate {
                list: "pacman",
                name: "git".into(),
            })
        );
        assert_eq!(
            parse_preset("version = 1\nflatpak = [\"org.a.B\", \"org.a.B\"]\n"),
            Err(PresetError::Duplicate {
                list: "flatpak",
                name: "org.a.B".into(),
            })
        );
    }

    // --- to_toml ---

    #[test]
    fn to_toml_tem_um_nome_por_linha_e_volta_igual() {
        let p = preset(&["base", "git"], &["org.nongnu.lingot"]);
        let texto = to_toml(&p);
        assert_eq!(
            texto,
            "# Preset do nodus (formato 1). Gerado por `nodusctl preset init`; edite à vontade.\nversion = 1\npacman = [\n    \"base\",\n    \"git\",\n]\nflatpak = [\n    \"org.nongnu.lingot\",\n]\n"
        );
        assert_eq!(parse_preset(&texto).unwrap(), p);
    }

    #[test]
    fn to_toml_com_listas_vazias_volta_igual() {
        let p = preset(&[], &[]);
        assert_eq!(parse_preset(&to_toml(&p)).unwrap(), p);
    }

    // --- pacman_install_command ---

    #[test]
    fn comando_de_instalacao_e_exato() {
        let p = preset(&["zsh", "git", "neovim"], &[]);
        let plano = plan(&p, &[pacote("git")], Some(&[]));
        assert_eq!(
            pacman_install_command(&plano).unwrap(),
            ["sudo", "pacman", "-S", "--needed", "--", "zsh", "neovim"]
        );
    }

    #[test]
    fn comando_nunca_pula_a_confirmacao_do_pacman_nem_sincroniza() {
        let plano = plan(&preset(&["zsh"], &[]), &[], Some(&[]));
        let comando = pacman_install_command(&plano).unwrap();
        for proibido in ["--noconfirm", "-Sy", "-Syu", "-y", "--overwrite"] {
            assert!(!comando.iter().any(|a| a == proibido), "{proibido}");
        }
    }

    #[test]
    fn sem_pacote_faltando_nao_ha_comando() {
        let p = preset(&["git"], &["org.a.B"]);
        let plano = plan(&p, &[pacote("git")], None);
        assert_eq!(pacman_install_command(&plano), None);
    }

    // --- flatpak_install_command ---

    #[test]
    fn comando_flatpak_e_exato() {
        let p = preset(
            &[],
            &["org.gnome.Calculator", "org.nongnu.lingot", "org.a.B"],
        );
        let plano = plan(&p, &[], Some(&[app("org.nongnu.lingot")]));
        assert_eq!(
            flatpak_install_command(&plano).unwrap(),
            [
                "flatpak",
                "install",
                "--user",
                "--",
                "flathub",
                "org.gnome.Calculator",
                "org.a.B"
            ]
        );
    }

    #[test]
    fn comando_flatpak_nunca_pula_o_prompt_nem_instala_no_sistema() {
        let plano = plan(&preset(&[], &["org.a.B"]), &[], Some(&[]));
        let comando = flatpak_install_command(&plano).unwrap();
        for proibido in ["-y", "--assumeyes", "--noninteractive", "--system"] {
            assert!(!comando.iter().any(|a| a == proibido), "{proibido}");
        }
        // `--` antes do remote e dos IDs: nenhum ID pode virar opção (T7).
        let separador = comando.iter().position(|a| a == "--").unwrap();
        assert_eq!(comando[separador + 1], "flathub");
    }

    #[test]
    fn sem_app_flatpak_faltando_nao_ha_comando() {
        let plano = plan(
            &preset(&["git"], &["org.a.B"]),
            &[],
            Some(&[app("org.a.B")]),
        );
        assert_eq!(flatpak_install_command(&plano), None);
    }

    // --- flathub_configured ---

    fn remotes(saida: Result<&str, RunnerError>) -> Fake {
        let mut r = Fake::novo();
        r.flatpak_remotes = saida.map(String::from);
        r
    }

    #[test]
    fn flathub_configurado_no_usuario() {
        let r = remotes(Ok("flathub\n"));
        assert_eq!(flathub_configured(&r), Ok(true));
        assert!(r.chamou(&["flatpak", "remotes", "--user", "--columns=name"]));
    }

    #[test]
    fn flathub_entre_outros_remotes() {
        assert_eq!(
            flathub_configured(&remotes(Ok("fedora\nflathub\n"))),
            Ok(true)
        );
    }

    #[test]
    fn sem_remote_nenhum_flathub_nao_esta_configurado() {
        assert_eq!(flathub_configured(&remotes(Ok(""))), Ok(false));
    }

    #[test]
    fn outro_remote_ou_nome_parecido_nao_e_flathub() {
        for saida in ["fedora\n", "flathub-beta\n", "my-flathub\n", "Flathub\n"] {
            assert_eq!(
                flathub_configured(&remotes(Ok(saida))),
                Ok(false),
                "{saida:?}"
            );
        }
    }

    #[test]
    fn flatpak_ausente_nao_tem_flathub() {
        let r = remotes(Err(RunnerError::NotFound {
            program: "flatpak".into(),
        }));
        assert_eq!(flathub_configured(&r), Ok(false));
    }

    #[test]
    fn falha_do_flatpak_ao_listar_remotes_vira_erro() {
        let r = remotes(Err(RunnerError::Failed {
            program: "flatpak".into(),
            code: Some(1),
        }));
        assert!(flathub_configured(&r).is_err());
    }

    // --- suggest: o que está instalado de propósito e fora do preset ---

    fn maquina(pacman: &[&str], flatpak: &[&str], flatpak_installed: bool) -> MachinePreset {
        MachinePreset {
            preset: preset(pacman, flatpak),
            flatpak_installed,
        }
    }

    fn v(itens: &[&str]) -> Vec<String> {
        itens.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sugere_so_o_que_esta_fora_do_preset_na_ordem_da_maquina() {
        let s = suggest(
            &preset(&["git"], &[]),
            &maquina(&["zsh", "git", "vim"], &[], true),
            &[],
        );
        assert_eq!(s.pacman_extra, ["zsh", "vim"]);
        assert!(s.foreign_extra.is_empty());
    }

    #[test]
    fn item_do_preset_que_a_maquina_nao_tem_nao_e_sugestao() {
        // Faltar na máquina é assunto do `plan`, não do `suggest`.
        let s = suggest(
            &preset(&["git", "emacs"], &["org.a.B"]),
            &maquina(&["git"], &[], true),
            &[],
        );
        assert!(s.is_empty());
    }

    #[test]
    fn pacote_de_fora_dos_repositorios_vai_para_a_secao_propria() {
        let s = suggest(
            &preset(&[], &[]),
            &maquina(&["git", "brave-bin"], &[], true),
            &v(&["brave-bin"]),
        );
        assert_eq!(s.pacman_extra, ["git"]);
        assert_eq!(s.foreign_extra, ["brave-bin"]);
    }

    #[test]
    fn pacote_de_fora_que_ja_esta_no_preset_nao_e_sugerido() {
        let s = suggest(
            &preset(&["brave-bin"], &[]),
            &maquina(&["brave-bin"], &[], true),
            &v(&["brave-bin"]),
        );
        assert!(s.is_empty());
    }

    #[test]
    fn so_sugere_nome_que_veio_da_maquina() {
        // O nome estrangeiro só vale se também está entre os explícitos já validados.
        let s = suggest(
            &preset(&[], &[]),
            &maquina(&["git"], &[], true),
            &v(&["-Rns"]),
        );
        assert_eq!(s.pacman_extra, ["git"]);
        assert!(s.foreign_extra.is_empty());
    }

    #[test]
    fn sugere_apps_flatpak_fora_do_preset() {
        let s = suggest(
            &preset(&[], &["org.a.B"]),
            &maquina(&[], &["org.a.B", "org.gnome.Calculator"], true),
            &[],
        );
        assert_eq!(s.flatpak_extra, ["org.gnome.Calculator"]);
        assert!(s.flatpak_installed);
    }

    #[test]
    fn sem_flatpak_instalado_nao_ha_sugestao_flatpak_e_o_estado_aparece() {
        let s = suggest(&preset(&[], &[]), &maquina(&[], &[], false), &[]);
        assert!(s.flatpak_extra.is_empty());
        assert!(!s.flatpak_installed);
    }

    #[test]
    fn collect_nao_sugere_dependencia_so_o_que_foi_instalado_de_proposito() {
        // `glibc` está em `pacman -Q` mas não em `-Qe`: veio como dependência.
        let r = Fake::novo();
        let s = collect_suggestions(&r, &preset(&[], &[])).unwrap();
        assert_eq!(s.pacman_extra, ["git", "vim"]);
        assert!(!s.pacman_extra.iter().any(|n| n == "glibc"));
        assert!(r.chamou(&["pacman", "-Qem"]));
    }

    #[test]
    fn collect_separa_o_que_o_pacman_marca_como_estrangeiro() {
        let mut r = Fake::novo();
        r.pacman_qem = Ok("vim 9.1-1\n".into());
        let s = collect_suggestions(&r, &preset(&[], &[])).unwrap();
        assert_eq!(s.pacman_extra, ["git"]);
        assert_eq!(s.foreign_extra, ["vim"]);
    }

    #[test]
    fn init_seguido_de_suggest_da_vazio() {
        let mut r = Fake::novo();
        r.pacman_qem = Ok("vim 9.1-1\n".into());
        let do_init = preset_from_machine(&r).unwrap().preset;
        assert!(collect_suggestions(&r, &do_init).unwrap().is_empty());
    }

    #[test]
    fn collect_sem_flatpak_marca_nao_instalado() {
        let mut r = Fake::novo();
        r.flatpak = Err(RunnerError::NotFound {
            program: "flatpak".into(),
        });
        let s = collect_suggestions(&r, &preset(&["git", "vim"], &[])).unwrap();
        assert!(!s.flatpak_installed);
        assert!(s.is_empty());
    }

    #[test]
    fn falha_do_pacman_ao_listar_estrangeiros_vira_erro() {
        let mut r = Fake::novo();
        r.pacman_qem = Err(RunnerError::Failed {
            program: "pacman".into(),
            code: Some(1),
        });
        assert!(collect_suggestions(&r, &preset(&[], &[])).is_err());
    }

    #[test]
    fn sugestoes_serializam_com_os_nomes_do_json() {
        let s = suggest(
            &preset(&[], &[]),
            &maquina(&["git", "brave-bin"], &["org.a.B"], true),
            &v(&["brave-bin"]),
        );
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["pacman_extra"], serde_json::json!(["git"]));
        assert_eq!(json["foreign_extra"], serde_json::json!(["brave-bin"]));
        assert_eq!(json["flatpak_extra"], serde_json::json!(["org.a.B"]));
        assert_eq!(json["flatpak_installed"], serde_json::json!(true));
    }

    // --- plan ---

    #[test]
    fn plano_vazio_quando_tudo_ja_esta_instalado() {
        let p = preset(&["git", "vim"], &["org.a.B"]);
        let instalados = [pacote("git"), pacote("vim"), pacote("glibc")];
        let plano = plan(&p, &instalados, Some(&[app("org.a.B")]));
        assert_eq!(
            plano,
            Plan {
                pacman_missing: vec![],
                pacman_present: 2,
                pacman_unavailable: vec![],
                flatpak_missing: vec![],
                flatpak_present: 1,
                flatpak_installed: true,
            }
        );
        assert!(plano.is_empty());
    }

    #[test]
    fn plano_lista_so_o_que_falta_na_ordem_do_preset() {
        let p = preset(&["zsh", "git", "neovim"], &["org.a.B", "org.c.D"]);
        let plano = plan(&p, &[pacote("git")], Some(&[app("org.c.D")]));
        assert_eq!(plano.pacman_missing, ["zsh", "neovim"]);
        assert_eq!(plano.pacman_present, 1);
        assert_eq!(plano.flatpak_missing, ["org.a.B"]);
        assert_eq!(plano.flatpak_present, 1);
        assert!(!plano.is_empty());
    }

    #[test]
    fn pacote_instalado_fora_do_preset_nao_entra_no_plano() {
        let p = preset(&["git"], &[]);
        let instalados = [pacote("git"), pacote("steam")];
        let plano = plan(&p, &instalados, Some(&[app("org.x.Y")]));
        assert!(plano.is_empty());
    }

    #[test]
    fn sem_flatpak_instalado_todos_os_apps_do_preset_faltam() {
        let p = preset(&[], &["org.a.B"]);
        let plano = plan(&p, &[], None);
        assert!(!plano.flatpak_installed);
        assert_eq!(plano.flatpak_missing, ["org.a.B"]);
        assert_eq!(plano.flatpak_present, 0);
    }

    #[test]
    fn plano_serializa_em_snake_case() {
        let plano = plan(&preset(&["git"], &[]), &[], Some(&[]));
        let json = serde_json::to_value(&plano).unwrap();
        assert_eq!(json["pacman_missing"], serde_json::json!(["git"]));
        assert_eq!(json["flatpak_installed"], serde_json::json!(true));
    }

    // --- leitura da máquina ---

    /// Responde por programa e primeiro argumento; registra cada chamada.
    struct Fake {
        pacman_qe: Result<String, RunnerError>,
        pacman_q: Result<String, RunnerError>,
        pacman_qem: Result<String, RunnerError>,
        /// Nomes que `pacman -S --print` não encontra: a chamada que os inclui falha com exit 1.
        indisponiveis: Vec<String>,
        /// Erro da simulação `pacman -S --print` que não é "não encontrado" (ex.: pacman sumiu).
        pacman_sync_erro: Option<RunnerError>,
        flatpak: Result<String, RunnerError>,
        flatpak_remotes: Result<String, RunnerError>,
        chamadas: std::cell::RefCell<Vec<Vec<String>>>,
    }

    impl Fake {
        fn novo() -> Self {
            Fake {
                pacman_qe: Ok("git 2.56.0-1\nvim 9.1-1\n".into()),
                pacman_q: Ok("git 2.56.0-1\nglibc 2.42-1\nvim 9.1-1\n".into()),
                pacman_qem: Ok("".into()),
                indisponiveis: vec![],
                pacman_sync_erro: None,
                flatpak: Ok("org.nongnu.lingot\t1.1.1\tstable\tflathub\tuser\n".into()),
                flatpak_remotes: Ok("flathub\n".into()),
                chamadas: Default::default(),
            }
        }

        /// Houve uma chamada exatamente com estes programa e argumentos?
        fn chamou(&self, comando: &[&str]) -> bool {
            self.chamadas.borrow().iter().any(|c| c[..] == *comando)
        }
    }

    impl CommandRunner for Fake {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError> {
            let mut chamada = vec![program.to_string()];
            chamada.extend(args.iter().map(|a| a.to_string()));
            self.chamadas.borrow_mut().push(chamada);
            match (program, args.first().copied()) {
                ("pacman", Some("-Qe")) => self.pacman_qe.clone(),
                ("pacman", Some("-Q")) => self.pacman_q.clone(),
                ("pacman", Some("-Qem")) => self.pacman_qem.clone(),
                ("pacman", Some("-S")) => match &self.pacman_sync_erro {
                    Some(erro) => Err(erro.clone()),
                    None if args
                        .iter()
                        .any(|a| self.indisponiveis.iter().any(|n| n == a)) =>
                    {
                        Err(RunnerError::Failed {
                            program: "pacman".into(),
                            code: Some(1),
                        })
                    }
                    None => Ok(String::new()),
                },
                ("flatpak", Some("list")) => self.flatpak.clone(),
                ("flatpak", Some("remotes")) => self.flatpak_remotes.clone(),
                _ => panic!("chamada inesperada: {program} {args:?}"),
            }
        }
    }

    #[test]
    fn preset_da_maquina_usa_so_os_pacotes_explicitos() {
        let r = Fake::novo();
        let m = preset_from_machine(&r).unwrap();
        assert_eq!(m.preset.pacman, ["git", "vim"]);
        assert_eq!(m.preset.flatpak, ["org.nongnu.lingot"]);
        assert!(m.flatpak_installed);
        assert!(r.chamou(&["pacman", "-Qe"]));
        assert!(!r.chamou(&["pacman", "-Q"]));
    }

    #[test]
    fn preset_da_maquina_sem_flatpak_fica_sem_apps_e_avisa() {
        let mut r = Fake::novo();
        r.flatpak = Err(RunnerError::NotFound {
            program: "flatpak".into(),
        });
        let m = preset_from_machine(&r).unwrap();
        assert!(m.preset.flatpak.is_empty());
        assert!(!m.flatpak_installed);
    }

    #[test]
    fn preset_da_maquina_com_flatpak_sem_apps_nao_e_o_mesmo_que_sem_flatpak() {
        let mut r = Fake::novo();
        r.flatpak = Ok(String::new());
        let m = preset_from_machine(&r).unwrap();
        assert!(m.preset.flatpak.is_empty());
        assert!(m.flatpak_installed);
    }

    #[test]
    fn preset_da_maquina_recusa_nome_invalido_vindo_do_pacman() {
        let mut r = Fake::novo();
        r.pacman_qe = Ok("git 2.56.0-1\n-Rns 1.0-1\n".into());
        assert!(matches!(
            preset_from_machine(&r),
            Err(MachineError::Preset(PresetError::InvalidName { .. }))
        ));
    }

    #[test]
    fn init_seguido_de_plan_na_mesma_maquina_da_plano_vazio() {
        let r = Fake::novo();
        let p = parse_preset(&to_toml(&preset_from_machine(&r).unwrap().preset)).unwrap();
        assert!(collect_plan(&r, &p).unwrap().is_empty());
    }

    #[test]
    fn plan_da_maquina_considera_todos_os_pacotes_instalados() {
        // `glibc` veio como dependência (não está no -Qe), mas está instalado: não pode entrar no plano.
        let r = Fake::novo();
        let plano = collect_plan(&r, &preset(&["glibc", "zsh"], &[])).unwrap();
        assert_eq!(plano.pacman_missing, ["zsh"]);
        assert!(r.chamou(&["pacman", "-Q"]));
    }

    #[test]
    fn plan_da_maquina_sem_flatpak_marca_nao_instalado() {
        let mut r = Fake::novo();
        r.flatpak = Err(RunnerError::NotFound {
            program: "flatpak".into(),
        });
        let plano = collect_plan(&r, &preset(&[], &["org.a.B"])).unwrap();
        assert!(!plano.flatpak_installed);
    }

    #[test]
    fn falha_do_pacman_vira_erro() {
        let mut r = Fake::novo();
        r.pacman_q = Err(RunnerError::Failed {
            program: "pacman".into(),
            code: Some(1),
        });
        assert!(collect_plan(&r, &preset(&["git"], &[])).is_err());
    }

    // --- itens que o pacman não encontra (P-18) ---

    /// As chamadas à simulação `pacman -S ...`, na ordem.
    fn simulacoes(r: &Fake) -> Vec<Vec<String>> {
        r.chamadas
            .borrow()
            .iter()
            .filter(|c| c[0] == "pacman" && c.get(1).map(String::as_str) == Some("-S"))
            .cloned()
            .collect()
    }

    #[test]
    fn nada_faltando_nao_consulta_a_simulacao_do_pacman() {
        let r = Fake::novo();
        let plano = collect_plan(&r, &preset(&["git", "vim"], &[])).unwrap();
        assert!(simulacoes(&r).is_empty());
        assert!(plano.pacman_unavailable.is_empty());
    }

    #[test]
    fn tudo_instalavel_custa_uma_unica_simulacao_com_o_lote_inteiro() {
        let r = Fake::novo();
        let plano = collect_plan(&r, &preset(&["git", "zsh", "neovim"], &[])).unwrap();
        assert_eq!(plano.pacman_missing, ["zsh", "neovim"]);
        assert!(plano.pacman_unavailable.is_empty());
        assert_eq!(
            simulacoes(&r),
            [["pacman", "-S", "--print", "--needed", "--", "zsh", "neovim"]]
        );
    }

    #[test]
    fn item_que_o_pacman_nao_encontra_sai_do_que_instalar_e_vai_para_indisponiveis() {
        let mut r = Fake::novo();
        r.indisponiveis = vec!["brave-bin".into()];
        let plano = collect_plan(&r, &preset(&["zsh", "brave-bin", "neovim"], &[])).unwrap();
        assert_eq!(plano.pacman_missing, ["zsh", "neovim"]);
        assert_eq!(plano.pacman_unavailable, ["brave-bin"]);
        // Lote (falha) e depois um por item, na ordem do preset.
        assert_eq!(
            simulacoes(&r),
            [
                vec![
                    "pacman",
                    "-S",
                    "--print",
                    "--needed",
                    "--",
                    "zsh",
                    "brave-bin",
                    "neovim"
                ],
                vec!["pacman", "-S", "--print", "--needed", "--", "zsh"],
                vec!["pacman", "-S", "--print", "--needed", "--", "brave-bin"],
                vec!["pacman", "-S", "--print", "--needed", "--", "neovim"],
            ]
        );
    }

    #[test]
    fn so_indisponiveis_nao_deixa_nada_para_instalar() {
        let mut r = Fake::novo();
        r.indisponiveis = vec!["brave-bin".into(), "google-chrome".into()];
        let plano = collect_plan(&r, &preset(&["brave-bin", "google-chrome"], &[])).unwrap();
        assert!(plano.pacman_missing.is_empty());
        assert_eq!(plano.pacman_unavailable, ["brave-bin", "google-chrome"]);
        assert!(plano.is_empty());
        assert_eq!(pacman_install_command(&plano), None);
    }

    #[test]
    fn indisponivel_nao_conta_como_presente() {
        let mut r = Fake::novo();
        r.indisponiveis = vec!["brave-bin".into()];
        // `git` e `vim` estão instalados; `brave-bin` falta e é indisponível.
        let plano = collect_plan(&r, &preset(&["git", "vim", "brave-bin"], &[])).unwrap();
        assert_eq!(plano.pacman_present, 2);
    }

    #[test]
    fn erro_da_simulacao_que_nao_e_item_inexistente_propaga() {
        // Sem `pacman` (ou sem conseguir iniciá-lo) não dá para classificar: nada de "indisponível".
        for erro in [
            RunnerError::NotFound {
                program: "pacman".into(),
            },
            RunnerError::Spawn {
                program: "pacman".into(),
                reason: "sem permissão".into(),
            },
        ] {
            let mut r = Fake::novo();
            r.pacman_sync_erro = Some(erro);
            assert!(collect_plan(&r, &preset(&["zsh"], &[])).is_err());
        }
    }

    #[test]
    fn plano_serializa_os_indisponiveis() {
        let mut r = Fake::novo();
        r.indisponiveis = vec!["brave-bin".into()];
        let plano = collect_plan(&r, &preset(&["zsh", "brave-bin"], &[])).unwrap();
        let json = serde_json::to_value(&plano).unwrap();
        assert_eq!(json["pacman_missing"], serde_json::json!(["zsh"]));
        assert_eq!(json["pacman_unavailable"], serde_json::json!(["brave-bin"]));
    }

    // --- add_to_preset: acrescenta nomes ao texto do preset sem mexer no resto ---

    fn add(texto: &str, pacman: &[&str], flatpak: &[&str]) -> Result<String, PresetError> {
        let a: Vec<String> = pacman.iter().map(|s| s.to_string()).collect();
        let b: Vec<String> = flatpak.iter().map(|s| s.to_string()).collect();
        add_to_preset(texto, &a, &b)
    }

    fn adicionado(texto: &str, pacman: &[&str], flatpak: &[&str]) -> String {
        add(texto, pacman, flatpak).unwrap()
    }

    #[test]
    fn multilinha_com_virgula_final_acrescenta_linhas_antes_do_colchete() {
        let texto = "version = 1\npacman = [\n    \"git\",\n]\nflatpak = []\n";
        assert_eq!(
            adicionado(texto, &["vim", "curl"], &[]),
            "version = 1\npacman = [\n    \"git\",\n    \"vim\",\n    \"curl\",\n]\nflatpak = []\n"
        );
    }

    #[test]
    fn multilinha_sem_virgula_final_poe_a_virgula_no_ultimo() {
        let texto = "version = 1\npacman = [\n    \"git\"\n]\n";
        assert_eq!(
            adicionado(texto, &["vim"], &[]),
            "version = 1\npacman = [\n    \"git\",\n    \"vim\",\n]\n"
        );
    }

    #[test]
    fn comentario_depois_do_ultimo_elemento_fica_onde_estava() {
        let com_virgula = "version = 1\npacman = [\n    \"git\", # base\n]\n";
        assert_eq!(
            adicionado(com_virgula, &["vim"], &[]),
            "version = 1\npacman = [\n    \"git\", # base\n    \"vim\",\n]\n"
        );
        let sem_virgula = "version = 1\npacman = [\n    \"git\" # base\n]\n";
        assert_eq!(
            adicionado(sem_virgula, &["vim"], &[]),
            "version = 1\npacman = [\n    \"git\", # base\n    \"vim\",\n]\n"
        );
    }

    #[test]
    fn comentario_solto_antes_do_colchete_fica_antes_do_novo() {
        let texto = "version = 1\npacman = [\n    \"git\",\n    # editores abaixo\n]\n";
        assert_eq!(
            adicionado(texto, &["vim"], &[]),
            "version = 1\npacman = [\n    \"git\",\n    # editores abaixo\n    \"vim\",\n]\n"
        );
    }

    #[test]
    fn lista_inline_ganha_os_nomes_no_fim_da_mesma_linha() {
        let texto = "version = 1\npacman = [\"git\", \"zsh\"]\n";
        assert_eq!(
            adicionado(texto, &["vim", "curl"], &[]),
            "version = 1\npacman = [\"git\", \"zsh\", \"vim\", \"curl\"]\n"
        );
    }

    #[test]
    fn lista_vazia_inline_e_multilinha() {
        assert_eq!(
            adicionado("version = 1\npacman = []\n", &["vim", "curl"], &[]),
            "version = 1\npacman = [\"vim\", \"curl\"]\n"
        );
        assert_eq!(
            adicionado("version = 1\npacman = [\n]\n", &["vim", "curl"], &[]),
            "version = 1\npacman = [\n    \"vim\",\n    \"curl\",\n]\n"
        );
        assert_eq!(
            adicionado(
                "version = 1\npacman = [\n    # nada ainda\n]\n",
                &["vim"],
                &[]
            ),
            "version = 1\npacman = [\n    # nada ainda\n    \"vim\",\n]\n"
        );
    }

    #[test]
    fn colchete_na_mesma_linha_do_ultimo_elemento_vira_insercao_inline() {
        let texto = "version = 1\npacman = [\n    \"git\"]\n";
        assert_eq!(
            adicionado(texto, &["vim"], &[]),
            "version = 1\npacman = [\n    \"git\", \"vim\"]\n"
        );
    }

    #[test]
    fn chave_ausente_e_criada_no_fim_do_arquivo() {
        let texto = "version = 1\npacman = [\n    \"git\",\n]\n";
        assert_eq!(
            adicionado(texto, &[], &["org.gnome.Calculator"]),
            "version = 1\npacman = [\n    \"git\",\n]\nflatpak = [\n    \"org.gnome.Calculator\",\n]\n"
        );
        // Sem quebra de linha no fim do arquivo: ela é posta antes da chave nova.
        assert_eq!(
            adicionado("version = 1\npacman = [\"git\"]", &[], &["org.a.B"]),
            "version = 1\npacman = [\"git\"]\nflatpak = [\n    \"org.a.B\",\n]\n"
        );
        // As duas ausentes.
        assert_eq!(
            adicionado("version = 1\n", &["vim"], &["org.a.B"]),
            "version = 1\npacman = [\n    \"vim\",\n]\nflatpak = [\n    \"org.a.B\",\n]\n"
        );
    }

    #[test]
    fn pacman_e_flatpak_juntos_mesmo_com_o_flatpak_declarado_primeiro() {
        let texto = "version = 1\nflatpak = [\n    \"org.a.B\",\n]\npacman = [\n    \"git\",\n]\n";
        assert_eq!(
            adicionado(texto, &["vim"], &["org.c.D"]),
            "version = 1\nflatpak = [\n    \"org.a.B\",\n    \"org.c.D\",\n]\npacman = [\n    \"git\",\n    \"vim\",\n]\n"
        );
    }

    #[test]
    fn nome_ja_presente_e_repetido_nos_novos_entram_uma_vez_so() {
        let texto = "version = 1\npacman = [\n    \"git\",\n]\n";
        assert_eq!(
            adicionado(texto, &["git", "vim", "vim"], &[]),
            "version = 1\npacman = [\n    \"git\",\n    \"vim\",\n]\n"
        );
    }

    #[test]
    fn nada_a_acrescentar_devolve_o_texto_identico() {
        let esquisito = "# topo\nversion   =   1\npacman=[ \"git\" ,\"zsh\"   ]  # fim\n";
        assert_eq!(adicionado(esquisito, &[], &[]), esquisito);
        assert_eq!(adicionado(esquisito, &["git", "zsh"], &[]), esquisito);
    }

    #[test]
    fn so_o_trecho_inserido_muda_o_resto_fica_byte_a_byte() {
        let texto = "# Meu preset\n\nversion = 1   # formato\n\n# pacotes\npacman = [\n    \"git\",   # controle\n\n    \"zsh\"\n]\n\n# apps\nflatpak = [\n    \"org.a.B\",\n]\n# fim\n";
        assert_eq!(
            adicionado(texto, &["vim"], &["org.c.D"]),
            "# Meu preset\n\nversion = 1   # formato\n\n# pacotes\npacman = [\n    \"git\",   # controle\n\n    \"zsh\",\n    \"vim\",\n]\n\n# apps\nflatpak = [\n    \"org.a.B\",\n    \"org.c.D\",\n]\n# fim\n"
        );
    }

    #[test]
    fn fim_de_linha_crlf_e_mantido() {
        let texto = "version = 1\r\npacman = [\r\n    \"git\",\r\n]\r\n";
        assert_eq!(
            adicionado(texto, &["vim"], &["org.a.B"]),
            "version = 1\r\npacman = [\r\n    \"git\",\r\n    \"vim\",\r\n]\r\nflatpak = [\r\n    \"org.a.B\",\r\n]\r\n"
        );
    }

    #[test]
    fn o_resultado_sempre_reparseia_com_os_antigos_e_depois_os_novos() {
        let formas = [
            "version = 1\n",
            "version = 1\npacman = []\nflatpak = []\n",
            "version = 1\npacman = [\"git\",]\n",
            "version = 1\npacman = [ \"git\" , \"zsh\" ]\nflatpak = [ ]\n",
            "version = 1\npacman = [\n\t\"git\"\n]\n",
            "version = 1\npacman = [\n    \"git\", \"zsh\", # dois\n]\n",
            "version = 1\npacman = [\n    \"git\"] # cola\nflatpak = [\"org.a.B\"]\n",
            "version = 1\npacman = [\n\n    \"git\"\n\n]\n",
            "version = 1\npacman = [ # abre\n    \"git\"\n]\n",
            "version = 1\npacman = [\n    'git',\n]\n",
        ];
        for texto in formas {
            let antes = parse_preset(texto).unwrap();
            let depois = parse_preset(&adicionado(texto, &["vim", "curl"], &["org.x.Y"]))
                .unwrap_or_else(|erro| panic!("{texto:?}: {erro}"));
            let esperado_pacman: Vec<String> = antes
                .pacman
                .iter()
                .cloned()
                .chain(["vim".to_string(), "curl".to_string()])
                .collect();
            let esperado_flatpak: Vec<String> = antes
                .flatpak
                .iter()
                .cloned()
                .chain(["org.x.Y".to_string()])
                .collect();
            assert_eq!(depois.pacman, esperado_pacman, "{texto:?}");
            assert_eq!(depois.flatpak, esperado_flatpak, "{texto:?}");
        }
    }

    #[test]
    fn preset_de_entrada_invalido_e_erro_e_nao_edita() {
        assert!(matches!(
            add("version = = 1\n", &["vim"], &[]),
            Err(PresetError::Toml(_))
        ));
        assert_eq!(
            add("version = 2\n", &["vim"], &[]),
            Err(PresetError::UnsupportedVersion(2))
        );
        assert!(matches!(
            add("version = 1\npacman = [\"git\", \"git\"]\n", &["vim"], &[]),
            Err(PresetError::Duplicate { .. })
        ));
        assert!(add("version = 1\nextra = 1\n", &["vim"], &[]).is_err());
    }

    #[test]
    fn nome_novo_invalido_e_erro_e_nao_vira_texto() {
        for (pacman, flatpak) in [
            (vec!["-Rns"], vec![]),
            (vec!["a\"b"], vec![]),
            (vec!["vim\n"], vec![]),
            (vec![], vec!["semponto"]),
            (vec![], vec!["org.a.B\"\nx = \"y"]),
        ] {
            assert!(
                matches!(
                    add("version = 1\n", &pacman, &flatpak),
                    Err(PresetError::InvalidName { .. })
                ),
                "{pacman:?} {flatpak:?}"
            );
        }
    }

    // --- hide_ignored: tira das sugestões o que o dono mandou ignorar ---

    fn sugestoes(pacman: &[&str], foreign: &[&str], flatpak: &[&str]) -> Suggestions {
        let lista = |nomes: &[&str]| nomes.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        Suggestions {
            pacman_extra: lista(pacman),
            foreign_extra: lista(foreign),
            flatpak_extra: lista(flatpak),
            flatpak_installed: true,
        }
    }

    fn nomes(lista: &[&str]) -> Vec<String> {
        lista.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn esconde_pacman_flatpak_e_fora_dos_repositorios_por_nome() {
        let s = sugestoes(&["a", "b", "c"], &["aur1", "aur2"], &["org.x.A", "org.x.B"]);
        let (visivel, escondidos) = hide_ignored(&s, &nomes(&["b", "aur2"]), &nomes(&["org.x.A"]));
        assert_eq!(visivel.pacman_extra, nomes(&["a", "c"]));
        assert_eq!(visivel.foreign_extra, nomes(&["aur1"]));
        assert_eq!(visivel.flatpak_extra, nomes(&["org.x.B"]));
        // Na ordem em que aparecem: pacman, fora dos repositórios, Flatpak.
        assert_eq!(escondidos, nomes(&["b", "aur2", "org.x.A"]));
    }

    #[test]
    fn sem_ignorados_nada_muda_e_nada_e_escondido() {
        let s = sugestoes(&["a"], &["aur1"], &["org.x.A"]);
        let (visivel, escondidos) = hide_ignored(&s, &[], &[]);
        assert_eq!(visivel, s);
        assert!(escondidos.is_empty());
    }

    #[test]
    fn ignorado_que_nao_esta_nas_sugestoes_e_ignorado_em_silencio() {
        let s = sugestoes(&["a"], &[], &[]);
        let (visivel, escondidos) =
            hide_ignored(&s, &nomes(&["nao-sugerido"]), &nomes(&["org.x.Z"]));
        assert_eq!(visivel, s);
        assert!(escondidos.is_empty());
    }

    #[test]
    fn a_lista_do_pacman_nao_esconde_flatpak_e_vice_versa() {
        let s = sugestoes(&["org.x.A"], &[], &["org.x.A"]);
        let (visivel, escondidos) = hide_ignored(&s, &nomes(&["org.x.A"]), &[]);
        assert!(visivel.pacman_extra.is_empty());
        assert_eq!(visivel.flatpak_extra, nomes(&["org.x.A"]));
        assert_eq!(escondidos, nomes(&["org.x.A"]));
    }

    #[test]
    fn esconder_tudo_deixa_as_sugestoes_vazias_e_preserva_o_flag_do_flatpak() {
        let mut s = sugestoes(&["a"], &[], &["org.x.A"]);
        s.flatpak_installed = false;
        let (visivel, escondidos) = hide_ignored(&s, &nomes(&["a"]), &nomes(&["org.x.A"]));
        assert!(visivel.is_empty());
        assert!(!visivel.flatpak_installed);
        assert_eq!(escondidos.len(), 2);
    }

    // Exceções por máquina (Fase 2, fatia 3): `[machines.<rótulo>]` com `skip_pacman` e
    // `skip_flatpak`. O preset é entrada não confiável: tudo é validado no parse.

    const COM_MAQUINAS: &str = "version = 1\n\
        pacman = [\"base\", \"git\", \"steam\"]\n\
        flatpak = [\"org.gnome.Calculator\", \"org.x.Y\"]\n\
        \n\
        [machines.notebook]\n\
        skip_pacman = [\"steam\"]\n\
        skip_flatpak = [\"org.gnome.Calculator\"]\n";

    fn com_pulo(corpo: &str) -> String {
        format!(
            "version = 1\npacman = [\"base\", \"git\", \"steam\"]\nflatpak = [\"org.x.Y\"]\n\n{corpo}"
        )
    }

    #[test]
    fn maquina_com_tabela_recebe_o_preset_sem_os_itens_pulados() {
        let arquivo = parse_preset_file(COM_MAQUINAS).unwrap();
        let efetivo = arquivo.for_machine("notebook");
        assert_eq!(efetivo.pacman, nomes(&["base", "git"]));
        assert_eq!(efetivo.flatpak, nomes(&["org.x.Y"]));
    }

    #[test]
    fn maquina_sem_tabela_recebe_o_preset_inteiro() {
        let arquivo = parse_preset_file(COM_MAQUINAS).unwrap();
        let efetivo = arquivo.for_machine("desktop");
        assert_eq!(efetivo.pacman, nomes(&["base", "git", "steam"]));
        assert_eq!(efetivo.flatpak, nomes(&["org.gnome.Calculator", "org.x.Y"]));
    }

    #[test]
    fn o_rotulo_e_comparado_inteiro_e_sem_confundir_maiusculas() {
        let arquivo = parse_preset_file(COM_MAQUINAS).unwrap();
        for outro in ["note", "notebook2", "Notebook", "notebook.", ""] {
            assert_eq!(
                arquivo.for_machine(outro).pacman,
                nomes(&["base", "git", "steam"]),
                "{outro:?} não é o rótulo `notebook`"
            );
        }
    }

    #[test]
    fn sem_tabela_nenhuma_o_resultado_e_o_de_antes() {
        let texto = "version = 1\npacman = [\"base\"]\nflatpak = [\"org.x.Y\"]\n";
        let arquivo = parse_preset_file(texto).unwrap();
        assert_eq!(arquivo.preset, parse_preset(texto).unwrap());
        assert_eq!(arquivo.for_machine("qualquer"), arquivo.preset);
        assert!(!arquivo.has_machines());
    }

    #[test]
    fn parse_preset_devolve_o_preset_inteiro_mesmo_com_tabelas() {
        let completo = parse_preset(COM_MAQUINAS).unwrap();
        assert_eq!(completo.pacman, nomes(&["base", "git", "steam"]));
        assert!(parse_preset_file(COM_MAQUINAS).unwrap().has_machines());
    }

    #[test]
    fn skipped_lista_o_que_foi_pulado_naquela_maquina() {
        let arquivo = parse_preset_file(COM_MAQUINAS).unwrap();
        let pulados = arquivo.skipped("notebook");
        assert_eq!(pulados.pacman, nomes(&["steam"]));
        assert_eq!(pulados.flatpak, nomes(&["org.gnome.Calculator"]));
        assert!(arquivo.skipped("desktop").is_empty());
    }

    #[test]
    fn tabela_so_com_uma_das_listas_vale() {
        let texto = com_pulo("[machines.pc]\nskip_pacman = [\"steam\"]\n");
        let efetivo = parse_preset_file(&texto).unwrap().for_machine("pc");
        assert_eq!(efetivo.pacman, nomes(&["base", "git"]));
        assert_eq!(efetivo.flatpak, nomes(&["org.x.Y"]));
    }

    #[test]
    fn duas_maquinas_pulam_coisas_diferentes() {
        let texto = com_pulo(
            "[machines.a]\nskip_pacman = [\"steam\"]\n[machines.b]\nskip_pacman = [\"git\"]\n",
        );
        let arquivo = parse_preset_file(&texto).unwrap();
        assert_eq!(arquivo.for_machine("a").pacman, nomes(&["base", "git"]));
        assert_eq!(arquivo.for_machine("b").pacman, nomes(&["base", "steam"]));
    }

    #[test]
    fn pular_item_que_nao_esta_na_lista_principal_e_erro() {
        let texto = com_pulo("[machines.pc]\nskip_pacman = [\"stam\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::SkipNotInPreset {
                machine: "pc".into(),
                list: "skip_pacman",
                name: "stam".into(),
            })
        );
        let texto = com_pulo("[machines.pc]\nskip_flatpak = [\"org.z.Z\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::SkipNotInPreset {
                machine: "pc".into(),
                list: "skip_flatpak",
                name: "org.z.Z".into(),
            })
        );
    }

    #[test]
    fn nome_invalido_ou_repetido_em_skip_e_recusado() {
        let texto = com_pulo("[machines.pc]\nskip_pacman = [\"git\", \"--overwrite=*\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::InvalidName {
                list: "skip_pacman",
                number: 2,
                name: "--overwrite=*".into(),
            })
        );
        let texto = com_pulo("[machines.pc]\nskip_pacman = [\"git\", \"git\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::Duplicate {
                list: "skip_pacman",
                name: "git".into(),
            })
        );
    }

    #[test]
    fn rotulo_invalido_e_recusado() {
        let compridos = "a".repeat(64);
        let recusados = [
            "Notebook",
            "a b",
            "-x",
            ".x",
            "a/b",
            "a\"b",
            compridos.as_str(),
        ];
        for rotulo in recusados {
            let texto = com_pulo(&format!("[machines.{rotulo:?}]\nskip_pacman = [\"git\"]\n"));
            assert_eq!(
                parse_preset_file(&texto),
                Err(PresetError::InvalidMachineLabel(rotulo.to_string())),
                "{rotulo:?} deveria ser recusado"
            );
        }
        let ok = "a".repeat(63);
        let texto = com_pulo(&format!("[machines.{ok}]\nskip_pacman = [\"git\"]\n"));
        assert!(parse_preset_file(&texto).is_ok());
    }

    #[test]
    fn campo_desconhecido_na_tabela_da_maquina_e_erro() {
        let texto = com_pulo("[machines.pc]\nskip_pacmna = [\"git\"]\n");
        assert!(matches!(
            parse_preset_file(&texto),
            Err(PresetError::Toml(_))
        ));
        let texto = com_pulo("[machines.pc]\nadd_pacman = [\"vim\"]\n");
        assert!(matches!(
            parse_preset_file(&texto),
            Err(PresetError::Toml(_))
        ));
    }

    #[test]
    fn tabela_machines_com_tipo_errado_e_erro() {
        let texto = "version = 1\npacman = [\"git\"]\nmachines = [\"pc\"]\n";
        assert!(matches!(
            parse_preset_file(texto),
            Err(PresetError::Toml(_))
        ));
    }

    #[test]
    fn a_versao_continua_sendo_1_com_tabelas() {
        let texto = COM_MAQUINAS.replacen("version = 1", "version = 2", 1);
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn adotar_numa_lista_existente_mantem_as_tabelas_intactas() {
        let novo = add_to_preset(COM_MAQUINAS, &nomes(&["vim"]), &[]).unwrap();
        let arquivo = parse_preset_file(&novo).unwrap();
        assert_eq!(
            arquivo.preset.pacman,
            nomes(&["base", "git", "steam", "vim"])
        );
        assert_eq!(
            arquivo.for_machine("notebook").pacman,
            nomes(&["base", "git", "vim"])
        );
        assert!(novo.contains("[machines.notebook]\nskip_pacman = [\"steam\"]\n"));
    }

    #[test]
    fn adotar_criando_uma_lista_que_falta_com_tabela_no_arquivo_recusa_sem_gravar() {
        // O texto novo iria para o fim do arquivo, isto é, para dentro da última tabela.
        let texto = "version = 1\npacman = [\"git\"]\n\n[machines.pc]\nskip_pacman = [\"git\"]\n";
        assert!(matches!(
            add_to_preset(texto, &[], &nomes(&["org.x.Y"])),
            Err(PresetError::Edit(_))
        ));
    }

    // Acréscimos por máquina (Fase 2, fatia 4): `add_pacman` e `add_flatpak`. O preset efetivo é
    // lista principal sem os pulados, seguida dos acrescentados.

    const COM_ADD: &str = "version = 1\n\
        pacman = [\"base\", \"git\"]\n\
        flatpak = [\"org.x.Y\"]\n\
        \n\
        [machines.desktop]\n\
        add_pacman = [\"steam\", \"htop\"]\n\
        add_flatpak = [\"org.z.Z\"]\n";

    #[test]
    fn maquina_com_add_recebe_a_lista_principal_mais_os_acrescentados() {
        let efetivo = parse_preset_file(COM_ADD).unwrap().for_machine("desktop");
        assert_eq!(efetivo.pacman, nomes(&["base", "git", "steam", "htop"]));
        assert_eq!(efetivo.flatpak, nomes(&["org.x.Y", "org.z.Z"]));
    }

    #[test]
    fn outra_maquina_nao_recebe_os_acrescentados() {
        let arquivo = parse_preset_file(COM_ADD).unwrap();
        let efetivo = arquivo.for_machine("notebook");
        assert_eq!(efetivo.pacman, nomes(&["base", "git"]));
        assert_eq!(efetivo.flatpak, nomes(&["org.x.Y"]));
        assert!(arquivo.added("notebook").is_empty());
    }

    #[test]
    fn added_lista_o_que_foi_acrescentado_naquela_maquina() {
        let arquivo = parse_preset_file(COM_ADD).unwrap();
        let acrescentados = arquivo.added("desktop");
        assert_eq!(acrescentados.pacman, nomes(&["steam", "htop"]));
        assert_eq!(acrescentados.flatpak, nomes(&["org.z.Z"]));
        assert!(arquivo.skipped("desktop").is_empty());
    }

    #[test]
    fn so_add_ja_conta_como_ter_tabelas() {
        assert!(parse_preset_file(COM_ADD).unwrap().has_machines());
    }

    #[test]
    fn skip_e_add_na_mesma_maquina_pulam_e_acrescentam() {
        let texto = "version = 1\npacman = [\"base\", \"git\"]\nflatpak = []\n\n\
            [machines.pc]\nskip_pacman = [\"git\"]\nadd_pacman = [\"steam\"]\n";
        let arquivo = parse_preset_file(texto).unwrap();
        assert_eq!(arquivo.for_machine("pc").pacman, nomes(&["base", "steam"]));
        assert_eq!(arquivo.skipped("pc").pacman, nomes(&["git"]));
        assert_eq!(arquivo.added("pc").pacman, nomes(&["steam"]));
    }

    #[test]
    fn parse_preset_continua_devolvendo_so_a_lista_principal() {
        let completo = parse_preset(COM_ADD).unwrap();
        assert_eq!(completo.pacman, nomes(&["base", "git"]));
        assert_eq!(completo.flatpak, nomes(&["org.x.Y"]));
    }

    #[test]
    fn acrescentar_o_que_ja_esta_na_lista_principal_e_erro() {
        let texto = com_pulo("[machines.pc]\nadd_pacman = [\"git\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::AddAlreadyInPreset {
                machine: "pc".into(),
                list: "add_pacman",
                name: "git".into(),
            })
        );
        let texto = com_pulo("[machines.pc]\nadd_flatpak = [\"org.x.Y\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::AddAlreadyInPreset {
                machine: "pc".into(),
                list: "add_flatpak",
                name: "org.x.Y".into(),
            })
        );
    }

    #[test]
    fn o_mesmo_item_em_skip_e_add_da_mesma_maquina_e_erro() {
        let texto = com_pulo("[machines.pc]\nskip_pacman = [\"git\"]\nadd_pacman = [\"git\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::AddAlreadyInPreset {
                machine: "pc".into(),
                list: "add_pacman",
                name: "git".into(),
            })
        );
    }

    #[test]
    fn nome_invalido_ou_repetido_em_add_e_recusado() {
        let texto = com_pulo("[machines.pc]\nadd_pacman = [\"vim\", \"--overwrite=*\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::InvalidName {
                list: "add_pacman",
                number: 2,
                name: "--overwrite=*".into(),
            })
        );
        let texto = com_pulo("[machines.pc]\nadd_pacman = [\"vim\", \"vim\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::Duplicate {
                list: "add_pacman",
                name: "vim".into(),
            })
        );
        let texto = com_pulo("[machines.pc]\nadd_flatpak = [\"lingot\"]\n");
        assert_eq!(
            parse_preset_file(&texto),
            Err(PresetError::InvalidName {
                list: "add_flatpak",
                number: 1,
                name: "lingot".into(),
            })
        );
    }

    #[test]
    fn campo_com_nome_parecido_com_add_continua_sendo_erro() {
        let texto = com_pulo("[machines.pc]\nadd_pacmna = [\"vim\"]\n");
        assert!(matches!(
            parse_preset_file(&texto),
            Err(PresetError::Toml(_))
        ));
    }

    #[test]
    fn adotar_numa_lista_existente_mantem_as_tabelas_com_add_intactas() {
        let novo = add_to_preset(COM_ADD, &nomes(&["vim"]), &[]).unwrap();
        let arquivo = parse_preset_file(&novo).unwrap();
        assert_eq!(arquivo.preset.pacman, nomes(&["base", "git", "vim"]));
        assert_eq!(
            arquivo.for_machine("desktop").pacman,
            nomes(&["base", "git", "vim", "steam", "htop"])
        );
        assert!(novo.contains("[machines.desktop]\nadd_pacman = [\"steam\", \"htop\"]\n"));
    }

    #[test]
    fn adotar_o_que_outra_maquina_acrescenta_recusa_sem_gravar() {
        // O item iria para a lista principal e `add_pacman` da outra máquina viraria redundante:
        // o texto novo não passa no parse, então nada é devolvido.
        assert!(matches!(
            add_to_preset(COM_ADD, &nomes(&["steam"]), &[]),
            Err(PresetError::Edit(_))
        ));
    }
}
