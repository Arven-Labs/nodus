//! Preset declarativo de pacotes (fatia 1: só leitura). O preset é entrada não confiável (T1, T7).

use crate::inventory::{FlatpakApp, InventoryError, Package, parse_pacman_q, read_flatpak_apps};
use crate::runner::{CommandRunner, RunnerError};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
    pub pacman_missing: Vec<String>,
    pub pacman_present: usize,
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
}

pub fn parse_preset(input: &str) -> Result<Preset, PresetError> {
    let raw: RawPreset = toml::from_str(input).map_err(|e| PresetError::Toml(e.to_string()))?;
    if raw.version != FORMAT_VERSION {
        return Err(PresetError::UnsupportedVersion(raw.version));
    }
    validate("pacman", &raw.pacman, is_valid_pacman_name)?;
    validate("flatpak", &raw.flatpak, is_valid_flatpak_id)?;
    Ok(Preset {
        pacman: raw.pacman,
        flatpak: raw.flatpak,
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
pub(crate) fn is_valid_pacman_name(name: &str) -> bool {
    let allowed = |c: char| matches!(c, 'a'..='z' | '0'..='9' | '@' | '.' | '_' | '+' | '-');
    !name.is_empty() && !name.starts_with(['-', '.']) && name.chars().all(allowed)
}

/// ID de app Flatpak: segmentos `[A-Za-z0-9_-]` separados por `.`, pelo menos 2, nenhum vazio
/// nem começando por `-`. O Flatpak pode exigir mais (3 segmentos); aqui basta barrar injeção.
pub(crate) fn is_valid_flatpak_id(id: &str) -> bool {
    let allowed = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    let segment_ok = |s: &str| !s.is_empty() && !s.starts_with('-') && s.chars().all(allowed);
    id.split('.').count() >= 2 && id.split('.').all(segment_ok)
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
    Ok(plan(preset, &installed, apps.as_deref()))
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
}
