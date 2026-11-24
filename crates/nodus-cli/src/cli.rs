use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Plataforma aberta para padronizar e administrar ambientes de computadores.
#[derive(Parser, Debug)]
#[command(name = "nodusctl", version = nodus_core::VERSION, arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Mostra o sistema e os pacotes instalados (somente leitura).
    Inventory {
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
    },
    /// Lista atualizações pendentes de pacman e Flatpak (somente leitura, usa a rede).
    Updates {
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
    },
    /// Grava um snapshot desta máquina (SO, capacidades, pacotes instalados de propósito e apps
    /// Flatpak) em `<dotfiles_dir>/.nodus/snapshots/<rótulo>.json`, para versionar no seu
    /// repositório e comparar máquinas. Sem segredos, hora ou versões. O nodus não roda git: o
    /// commit é seu. Regravar substitui o arquivo do mesmo rótulo.
    Snapshot {
        /// Rótulo da máquina (`[a-z0-9._-]`, até 63 caracteres). Sem ele, usa o hostname.
        #[arg(long)]
        name: Option<String>,
    },
    /// Compara dois snapshots gravados pelo `snapshot` (em `<dotfiles_dir>/.nodus/snapshots/`) e
    /// mostra o que só existe em cada máquina, por lista. Só lê arquivos (somente leitura). Uma
    /// máquina sem Flatpak não é acusada de "faltar apps": a lista fica "não comparável". Divergir
    /// não é erro (código 0); arquivo ausente, versão desconhecida ou symlink são. Com `--all`, em
    /// vez de dois rótulos, compara todos os pares de snapshots legíveis (em ordem de rótulo); um
    /// snapshot ilegível vira aviso, e com menos de dois legíveis não há o que comparar (não é erro).
    Compare {
        /// Rótulo da primeira máquina (o nome do arquivo, sem `.json`).
        #[arg(
            required_unless_present = "all",
            requires = "b",
            conflicts_with = "all"
        )]
        a: Option<String>,
        /// Rótulo da segunda máquina.
        #[arg(required_unless_present = "all", conflicts_with = "all")]
        b: Option<String>,
        /// Compara todos os pares de snapshots em vez de dois rótulos.
        #[arg(long)]
        all: bool,
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
    },
    /// Mostra o que o nodus executou (cada `preset apply` e cada reversão), do mais antigo ao mais
    /// novo, com o número de cada entrada (somente leitura). `undo <n>` desfaz uma instalação.
    History {
        /// Emite JSON em vez de texto (só vale para a listagem; o `undo` ignora).
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        action: Option<HistoryAction>,
    },
    /// Mostra o estado dos dotfiles da lista curada (somente leitura; não abre os arquivos). A lista
    /// padrão é ajustada com `dotfiles_extra` e `dotfiles_skip` no config.toml. `adopt` leva um
    /// arquivo para a pasta-fonte (`dotfiles_dir`) e deixa um symlink no lugar.
    #[command(args_conflicts_with_subcommands = true)]
    Dotfiles {
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        action: Option<DotfilesAction>,
    },
    /// Lista os repositórios Git filhos diretos de uma pasta e o estado de cada um (somente leitura).
    /// Sem pasta: a de workspace (NODUS_WORKSPACE, ou a detectada; ver `workspace`). `sync` avança
    /// os repositórios que só estão atrás do upstream (`merge --ff-only`, com plano e confirmação).
    #[command(args_conflicts_with_subcommands = true)]
    Repos {
        /// Pasta cujos filhos diretos são examinados.
        dir: Option<PathBuf>,
        /// Faz `fetch` do `origin` antes de ler o estado (usa a rede).
        #[arg(long)]
        fetch: bool,
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        action: Option<ReposAction>,
    },
    /// Mostra qual é a pasta de workspace (só lê). Sem argumento: a variável NODUS_WORKSPACE, a
    /// pasta gravada com `workspace set`, ou a detecção entre pastas conhecidas sob o HOME (a com
    /// mais repositórios Git).
    #[command(args_conflicts_with_subcommands = true)]
    Workspace {
        /// Pasta de workspace informada à mão (vence a variável, a gravada e a detecção).
        dir: Option<PathBuf>,
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        action: Option<WorkspaceAction>,
    },
    /// Preset declarativo de pacotes (somente leitura nesta versão: nada é instalado).
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ReposAction {
    /// Faz `fetch` do `origin` de cada repositório, mostra o plano (quais avançam e por que os
    /// outros ficam de fora) e, com a sua confirmação, roda `git merge --ff-only` nos que só estão
    /// atrás do upstream. Nunca faz pull, reset, rebase, troca de branch nem resolve conflito; um
    /// repositório com alteração local, divergido ou com filtro na config fica de fora (ADR 0034).
    /// Não há desfazer: o histórico guarda o commit de antes.
    Sync {
        /// Pasta cujos filhos diretos são sincronizados (padrão: a de workspace).
        dir: Option<PathBuf>,
        /// Só este repositório (nome da pasta); pode repetir. Sem isso, todos os filhos diretos.
        #[arg(long = "only", value_name = "NOME")]
        only: Vec<String>,
        /// Só mostra o plano em JSON e não executa nada.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum DotfilesAction {
    /// Copia um arquivo do HOME para a pasta-fonte (`dotfiles_dir` no config.toml), confere byte a
    /// byte e troca o original por um symlink. Mostra o plano e pede confirmação; nunca sobrescreve.
    Adopt {
        /// Caminho do arquivo relativo ao HOME (ex.: `.bashrc`, `.config/git/config`).
        path: String,
    },
    /// Cria no HOME o symlink para um arquivo que já está na pasta-fonte (máquina nova). Mostra o
    /// plano e pede confirmação; cria as pastas que faltam; nunca sobrescreve um destino existente.
    /// Com `--all`, liga em lote os itens da lista curada (a de `nodusctl dotfiles`) que existem na
    /// pasta-fonte e faltam no HOME, com um plano e uma confirmação; o que não dá é pulado.
    Link {
        /// Caminho do arquivo relativo ao HOME (ex.: `.bashrc`, `.config/git/config`).
        #[arg(required_unless_present = "all", conflicts_with = "all")]
        path: Option<String>,
        /// Liga todos os itens da lista curada que a pasta-fonte tem.
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum WorkspaceAction {
    /// Grava a pasta como workspace no config do usuário (`~/.config/nodus/config.toml`). Vale
    /// quando não há argumento nem NODUS_WORKSPACE.
    Set {
        /// Pasta de workspace (precisa existir; grava o caminho real).
        dir: PathBuf,
    },
    /// Esquece a pasta gravada (volta à detecção).
    Unset,
}

#[derive(Subcommand, Debug)]
pub enum HistoryAction {
    /// Remove (`sudo pacman -Rs`) os pacotes que a entrada `n` instalou e ainda estão instalados
    /// de propósito, depois de simular, mostrar o plano e pedir confirmação.
    Undo {
        /// Número da entrada, como mostrado por `nodusctl history` (a partir de 1).
        #[arg(value_parser = clap::value_parser!(u32).range(1..))]
        n: u32,
    },
}

#[derive(Subcommand, Debug)]
pub enum PresetAction {
    /// Imprime um preset com os pacotes instalados de propósito e os apps Flatpak desta máquina.
    Init,
    /// Compara um preset com esta máquina e mostra o que faltaria instalar (não executa nada).
    Plan {
        /// Arquivo TOML do preset.
        file: PathBuf,
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
        /// Rótulo desta máquina para as exceções `[machines.<rótulo>]` do preset. Sem ele, usa o
        /// hostname (só se o preset tem `[machines]`).
        #[arg(long)]
        machine: Option<String>,
    },
    /// Pergunta, item a item (s/N/i/q), se o que está instalado de propósito e fora do preset entra
    /// nele, e acrescenta só o que você adotar, sem mexer no resto do arquivo. `i` ignora o item
    /// para sempre (fica no config do usuário). Não instala nada.
    Adopt {
        /// Arquivo TOML do preset (é editado no lugar; se for um symlink, grava no alvo).
        file: PathBuf,
        /// Rótulo desta máquina para as exceções `[machines.<rótulo>]` do preset: o que ela
        /// acrescenta não é oferecido. Sem ele, usa o hostname (só se o preset tem `[machines]`).
        /// O que for adotado entra sempre nas listas compartilhadas.
        #[arg(long)]
        machine: Option<String>,
    },
    /// Lista o que está instalado de propósito nesta máquina e fora do preset, com o trecho para
    /// colar nele (não executa nem escreve nada). Os itens ignorados ficam de fora e são contados.
    Suggest {
        /// Arquivo TOML do preset.
        file: PathBuf,
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
        /// Mostra também os itens ignorados.
        #[arg(long)]
        all: bool,
        /// Rótulo desta máquina para as exceções `[machines.<rótulo>]` do preset. Sem ele, usa o
        /// hostname (só se o preset tem `[machines]`).
        #[arg(long)]
        machine: Option<String>,
    },
    /// Lista os itens que você mandou ignorar (no config do usuário).
    Ignored,
    /// Tira itens da lista de ignorados: eles voltam a aparecer no `adopt` e no `suggest`.
    Unignore {
        /// Nomes (pacman) ou IDs (Flatpak) a tirar da lista.
        #[arg(required = true)]
        names: Vec<String>,
    },
    /// Mostra, por máquina, o que do preset ela não tem e o que ela tem fora do preset, lendo os
    /// snapshots de `<dotfiles_dir>/.nodus/snapshots/` (somente leitura; não roda pacman nem
    /// flatpak). Sem rótulos, olha todos os snapshots (um ilegível vira aviso); com rótulos, só
    /// esses (um ausente ou ruim é erro). O snapshot guarda só o que foi instalado de propósito.
    Status {
        /// Arquivo TOML do preset.
        file: PathBuf,
        /// Rótulos das máquinas a mostrar (o nome do arquivo, sem `.json`). Sem eles, todas.
        labels: Vec<String>,
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
    },
    /// Instala o que falta: pacotes pacman (`sudo pacman -S --needed`) e depois apps Flatpak
    /// (`flatpak install --user`, remote `flathub`). Cada passo mostra o comando e pede confirmação.
    Apply {
        /// Arquivo TOML do preset.
        file: PathBuf,
        /// Não instala os apps Flatpak desta vez (máquina sem Flatpak, ou preset compartilhado com
        /// outra máquina). Aplica só o pacman e lista o que ficou de fora.
        #[arg(long)]
        skip_flatpak: bool,
        /// Rótulo desta máquina para as exceções `[machines.<rótulo>]` do preset. Sem ele, usa o
        /// hostname (só se o preset tem `[machines]`).
        #[arg(long)]
        machine: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser, error::ErrorKind};

    #[test]
    fn version_imprime_nome_e_versao() {
        let erro = Cli::try_parse_from(["nodus", "--version"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::DisplayVersion);
        assert_eq!(erro.to_string().trim(), "nodusctl 0.1.0");
    }

    #[test]
    fn o_comando_se_chama_nodusctl_para_nao_colidir_com_o_nodus_do_cratesio() {
        assert_eq!(Cli::command().get_name(), "nodusctl");
    }

    #[test]
    fn help_descreve_o_programa() {
        let erro = Cli::try_parse_from(["nodus", "--help"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::DisplayHelp);
        assert!(erro.to_string().contains("nodus"));
        assert!(Cli::command().get_about().is_some());
    }

    #[test]
    fn flag_desconhecida_e_erro_de_uso() {
        let erro = Cli::try_parse_from(["nodus", "--xyz"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn inventory_sem_flag_e_texto() {
        let cli = Cli::try_parse_from(["nodus", "inventory"]).unwrap();
        assert!(matches!(cli.command, Command::Inventory { json: false }));
    }

    #[test]
    fn inventory_com_json() {
        let cli = Cli::try_parse_from(["nodus", "inventory", "--json"]).unwrap();
        assert!(matches!(cli.command, Command::Inventory { json: true }));
    }

    #[test]
    fn updates_sem_flag_e_texto() {
        let cli = Cli::try_parse_from(["nodus", "updates"]).unwrap();
        assert!(matches!(cli.command, Command::Updates { json: false }));
    }

    #[test]
    fn updates_com_json() {
        let cli = Cli::try_parse_from(["nodus", "updates", "--json"]).unwrap();
        assert!(matches!(cli.command, Command::Updates { json: true }));
    }

    #[test]
    fn dotfiles_em_texto_e_em_json() {
        for (args, esperado) in [
            (vec!["nodus", "dotfiles"], false),
            (vec!["nodus", "dotfiles", "--json"], true),
        ] {
            match Cli::try_parse_from(&args).unwrap().command {
                Command::Dotfiles { json, action } => {
                    assert_eq!(json, esperado, "{args:?}");
                    assert!(action.is_none(), "{args:?}");
                }
                outro => panic!("comando errado: {outro:?}"),
            }
        }
    }

    #[test]
    fn dotfiles_adopt_recebe_o_caminho_relativo() {
        let cli =
            Cli::try_parse_from(["nodus", "dotfiles", "adopt", ".config/git/config"]).unwrap();
        match cli.command {
            Command::Dotfiles {
                action: Some(DotfilesAction::Adopt { path }),
                ..
            } => assert_eq!(path, ".config/git/config"),
            outro => panic!("comando errado: {outro:?}"),
        }
        // sem caminho, é erro de uso
        assert!(Cli::try_parse_from(["nodus", "dotfiles", "adopt"]).is_err());
    }

    #[test]
    fn snapshot_aceita_nome_opcional_e_recusa_o_resto() {
        let cli = Cli::try_parse_from(["nodus", "snapshot"]).unwrap();
        assert!(matches!(cli.command, Command::Snapshot { name: None }));
        let cli = Cli::try_parse_from(["nodus", "snapshot", "--name", "meu-pc"]).unwrap();
        match cli.command {
            Command::Snapshot { name } => assert_eq!(name.as_deref(), Some("meu-pc")),
            outro => panic!("comando errado: {outro:?}"),
        }
        // O rótulo é uma opção, não um argumento solto; e não há outra opção.
        assert!(Cli::try_parse_from(["nodus", "snapshot", "meu-pc"]).is_err());
        assert!(Cli::try_parse_from(["nodus", "snapshot", "--out", "/tmp"]).is_err());
    }

    #[test]
    fn compare_recebe_dois_rotulos_e_json_opcional() {
        let cli = Cli::try_parse_from(["nodus", "compare", "pc-a", "pc-b"]).unwrap();
        match cli.command {
            Command::Compare { a, b, all, json } => {
                assert_eq!((a.as_deref(), b.as_deref()), (Some("pc-a"), Some("pc-b")));
                assert!(!all);
                assert!(!json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
        let cli = Cli::try_parse_from(["nodus", "compare", "pc-a", "pc-b", "--json"]).unwrap();
        assert!(matches!(cli.command, Command::Compare { json: true, .. }));
        // São exatamente dois rótulos.
        assert!(Cli::try_parse_from(["nodus", "compare"]).is_err());
        assert!(Cli::try_parse_from(["nodus", "compare", "pc-a"]).is_err());
        assert!(Cli::try_parse_from(["nodus", "compare", "a", "b", "c"]).is_err());
    }

    #[test]
    fn compare_all_nao_leva_rotulos_e_aceita_json() {
        let cli = Cli::try_parse_from(["nodus", "compare", "--all"]).unwrap();
        match cli.command {
            Command::Compare { a, b, all, json } => {
                assert_eq!((a, b), (None, None));
                assert!(all);
                assert!(!json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
        let cli = Cli::try_parse_from(["nodus", "compare", "--all", "--json"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Compare {
                all: true,
                json: true,
                ..
            }
        ));
        // `--all` e rótulos não se misturam, em nenhuma ordem.
        for args in [
            ["nodus", "compare", "--all", "pc-a"].as_slice(),
            ["nodus", "compare", "--all", "pc-a", "pc-b"].as_slice(),
            ["nodus", "compare", "pc-a", "--all"].as_slice(),
            ["nodus", "compare", "pc-a", "pc-b", "--all"].as_slice(),
        ] {
            let erro = Cli::try_parse_from(args).unwrap_err();
            assert_eq!(erro.kind(), ErrorKind::ArgumentConflict, "{args:?}");
        }
    }

    #[test]
    fn compare_sem_rotulos_e_sem_all_e_erro_de_uso() {
        for args in [
            ["nodus", "compare"].as_slice(),
            ["nodus", "compare", "--json"].as_slice(),
            ["nodus", "compare", "pc-a", "--json"].as_slice(),
        ] {
            assert!(Cli::try_parse_from(args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn dotfiles_link_recebe_o_caminho_relativo() {
        let cli = Cli::try_parse_from(["nodus", "dotfiles", "link", ".config/git/config"]).unwrap();
        match cli.command {
            Command::Dotfiles {
                action: Some(DotfilesAction::Link { path, all }),
                ..
            } => {
                assert_eq!(path.as_deref(), Some(".config/git/config"));
                assert!(!all);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
        // sem caminho e sem --all, é erro de uso
        assert!(Cli::try_parse_from(["nodus", "dotfiles", "link"]).is_err());
    }

    #[test]
    fn dotfiles_link_all_nao_aceita_caminho_junto() {
        let cli = Cli::try_parse_from(["nodus", "dotfiles", "link", "--all"]).unwrap();
        match cli.command {
            Command::Dotfiles {
                action: Some(DotfilesAction::Link { path, all }),
                ..
            } => {
                assert_eq!(path, None);
                assert!(all);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
        assert!(Cli::try_parse_from(["nodus", "dotfiles", "link", "--all", ".bashrc"]).is_err());
    }

    #[test]
    fn workspace_sem_argumento_e_em_texto() {
        let cli = Cli::try_parse_from(["nodus", "workspace"]).unwrap();
        match cli.command {
            Command::Workspace { dir, json, action } => {
                assert_eq!(dir, None);
                assert!(!json);
                assert!(action.is_none());
            }
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn workspace_com_pasta_e_json() {
        let cli = Cli::try_parse_from(["nodus", "workspace", "/ws", "--json"]).unwrap();
        match cli.command {
            Command::Workspace { dir, json, action } => {
                assert_eq!(dir, Some(std::path::PathBuf::from("/ws")));
                assert!(json);
                assert!(action.is_none());
            }
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn workspace_set_leva_a_pasta() {
        let cli = Cli::try_parse_from(["nodus", "workspace", "set", "/ws"]).unwrap();
        match cli.command {
            Command::Workspace { dir, action, .. } => {
                assert_eq!(dir, None);
                match action {
                    Some(WorkspaceAction::Set { dir }) => {
                        assert_eq!(dir, std::path::PathBuf::from("/ws"));
                    }
                    outra => panic!("ação errada: {outra:?}"),
                }
            }
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn workspace_set_aceita_pasta_depois_do_duplo_hifen() {
        // A GUI (ADR 0028) sempre manda `--` antes do caminho: um nome que começa com `-` é a pasta, não uma opção.
        let cli = Cli::try_parse_from(["nodus", "workspace", "set", "--", "-x"]).unwrap();
        match cli.command {
            Command::Workspace {
                action: Some(WorkspaceAction::Set { dir }),
                ..
            } => {
                assert_eq!(dir, std::path::PathBuf::from("-x"));
            }
            outro => panic!("comando errado: {outro:?}"),
        }
        let erro = Cli::try_parse_from(["nodus", "workspace", "set", "-x"]).unwrap_err();
        assert_eq!(
            erro.kind(),
            ErrorKind::UnknownArgument,
            "sem o `--`, `-x` é uma opção"
        );
    }

    #[test]
    fn workspace_set_exige_a_pasta() {
        let erro = Cli::try_parse_from(["nodus", "workspace", "set"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn workspace_unset_nao_leva_argumento() {
        let cli = Cli::try_parse_from(["nodus", "workspace", "unset"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Workspace {
                action: Some(WorkspaceAction::Unset),
                ..
            }
        ));
        let erro = Cli::try_parse_from(["nodus", "workspace", "unset", "/ws"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn workspace_set_nao_aceita_json() {
        let erro = Cli::try_parse_from(["nodus", "workspace", "--json", "set", "/ws"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn repos_sem_pasta_e_aceito() {
        let cli = Cli::try_parse_from(["nodus", "repos"]).unwrap();
        match cli.command {
            Command::Repos {
                dir,
                fetch,
                json,
                action,
            } => {
                assert_eq!(dir, None);
                assert!(!fetch);
                assert!(!json);
                assert!(action.is_none());
            }
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn repos_so_com_a_pasta_e_offline_em_texto() {
        let cli = Cli::try_parse_from(["nodus", "repos", "/ws"]).unwrap();
        match cli.command {
            Command::Repos {
                dir, fetch, json, ..
            } => {
                assert_eq!(dir, Some(std::path::PathBuf::from("/ws")));
                assert!(!fetch);
                assert!(!json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn repos_com_fetch_e_json() {
        let cli = Cli::try_parse_from(["nodus", "repos", "/ws", "--fetch", "--json"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Repos {
                fetch: true,
                json: true,
                ..
            }
        ));
    }

    #[test]
    fn repos_sync_sem_argumentos_usa_o_workspace_e_todos_os_repositorios() {
        let cli = Cli::try_parse_from(["nodus", "repos", "sync"]).unwrap();
        match cli.command {
            Command::Repos {
                action: Some(ReposAction::Sync { dir, only, json }),
                ..
            } => {
                assert_eq!(dir, None);
                assert!(only.is_empty());
                assert!(!json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn repos_sync_com_pasta_only_repetido_e_json() {
        let cli = Cli::try_parse_from([
            "nodus", "repos", "sync", "/ws", "--only", "alfa", "--only", "beta", "--json",
        ])
        .unwrap();
        match cli.command {
            Command::Repos {
                action: Some(ReposAction::Sync { dir, only, json }),
                ..
            } => {
                assert_eq!(dir, Some(std::path::PathBuf::from("/ws")));
                assert_eq!(only, ["alfa", "beta"]);
                assert!(json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn repos_sync_nao_mistura_com_as_opcoes_da_listagem() {
        // `--fetch` e a pasta da listagem não valem antes do `sync` (o sync sempre faz fetch).
        let erro = Cli::try_parse_from(["nodus", "repos", "--fetch", "sync"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::ArgumentConflict);
        let erro = Cli::try_parse_from(["nodus", "repos", "/ws", "sync"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::ArgumentConflict);
        assert!(Cli::try_parse_from(["nodus", "repos", "sync", "--fetch"]).is_err());
    }

    #[test]
    fn preset_init() {
        let cli = Cli::try_parse_from(["nodus", "preset", "init"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Preset {
                action: PresetAction::Init
            }
        ));
    }

    #[test]
    fn preset_plan_com_arquivo_e_json() {
        let cli = Cli::try_parse_from(["nodus", "preset", "plan", "p.toml", "--json"]).unwrap();
        match cli.command {
            Command::Preset {
                action: PresetAction::Plan { file, json, .. },
            } => {
                assert_eq!(file, std::path::PathBuf::from("p.toml"));
                assert!(json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn machine_e_opcional_em_plan_apply_e_suggest() {
        let machine = |args: &[&str]| -> Option<String> {
            let mut argv = vec!["nodus", "preset"];
            argv.extend_from_slice(args);
            match Cli::try_parse_from(argv).unwrap().command {
                Command::Preset {
                    action:
                        PresetAction::Plan { machine, .. }
                        | PresetAction::Apply { machine, .. }
                        | PresetAction::Suggest { machine, .. }
                        | PresetAction::Adopt { machine, .. },
                } => machine,
                outro => panic!("comando errado: {outro:?}"),
            }
        };
        for acao in ["plan", "apply", "suggest", "adopt"] {
            assert_eq!(machine(&[acao, "p.toml"]), None, "{acao} sem a flag");
            assert_eq!(
                machine(&[acao, "p.toml", "--machine", "notebook"]),
                Some("notebook".to_string()),
                "{acao} com a flag"
            );
        }
    }

    #[test]
    fn machine_nao_existe_em_status() {
        let args = ["nodus", "preset", "status", "p.toml", "--machine", "x"];
        let erro = Cli::try_parse_from(args).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn history_sem_flag_e_texto() {
        let cli = Cli::try_parse_from(["nodus", "history"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::History {
                json: false,
                action: None
            }
        ));
    }

    #[test]
    fn history_com_json() {
        let cli = Cli::try_parse_from(["nodus", "history", "--json"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::History {
                json: true,
                action: None
            }
        ));
    }

    #[test]
    fn history_undo_com_numero() {
        let cli = Cli::try_parse_from(["nodus", "history", "undo", "3"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::History {
                json: false,
                action: Some(HistoryAction::Undo { n: 3 })
            }
        ));
    }

    #[test]
    fn history_undo_exige_numero_a_partir_de_1() {
        let sem = Cli::try_parse_from(["nodus", "history", "undo"]).unwrap_err();
        assert_eq!(sem.kind(), ErrorKind::MissingRequiredArgument);
        for ruim in ["0", "-1", "abc", "1.5", ""] {
            let erro = Cli::try_parse_from(["nodus", "history", "undo", ruim]).unwrap_err();
            assert!(
                matches!(
                    erro.kind(),
                    ErrorKind::ValueValidation | ErrorKind::UnknownArgument
                ),
                "{ruim:?}: {:?}",
                erro.kind()
            );
        }
    }

    #[test]
    fn history_undo_nao_tem_modo_sim_para_tudo() {
        for flag in ["--yes", "-y", "--noconfirm"] {
            let erro = Cli::try_parse_from(["nodus", "history", "undo", "1", flag]).unwrap_err();
            assert_eq!(erro.kind(), ErrorKind::UnknownArgument, "{flag}");
        }
    }

    #[test]
    fn preset_apply_com_arquivo() {
        let cli = Cli::try_parse_from(["nodus", "preset", "apply", "p.toml"]).unwrap();
        match cli.command {
            Command::Preset {
                action: PresetAction::Apply { file, .. },
            } => assert_eq!(file, std::path::PathBuf::from("p.toml")),
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn preset_apply_skip_flatpak_e_opt_in() {
        let sem = Cli::try_parse_from(["nodus", "preset", "apply", "p.toml"]).unwrap();
        let com =
            Cli::try_parse_from(["nodus", "preset", "apply", "p.toml", "--skip-flatpak"]).unwrap();
        let flag = |cli: Cli| match cli.command {
            Command::Preset {
                action: PresetAction::Apply { skip_flatpak, .. },
            } => skip_flatpak,
            outro => panic!("comando errado: {outro:?}"),
        };
        assert!(!flag(sem), "sem a flag o padrão continua recusando");
        assert!(flag(com));
    }

    #[test]
    fn preset_apply_nao_tem_modo_sim_para_tudo() {
        for flag in ["--yes", "-y", "--noconfirm"] {
            let erro =
                Cli::try_parse_from(["nodus", "preset", "apply", "p.toml", flag]).unwrap_err();
            assert_eq!(erro.kind(), ErrorKind::UnknownArgument, "{flag}");
        }
    }

    #[test]
    fn preset_apply_exige_o_arquivo() {
        let erro = Cli::try_parse_from(["nodus", "preset", "apply"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn preset_adopt_leva_o_arquivo() {
        let cli = Cli::try_parse_from(["nodus", "preset", "adopt", "p.toml"]).unwrap();
        match cli.command {
            Command::Preset {
                action: PresetAction::Adopt { file, .. },
            } => assert_eq!(file, std::path::PathBuf::from("p.toml")),
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn preset_adopt_exige_o_arquivo_e_nao_tem_json() {
        let erro = Cli::try_parse_from(["nodus", "preset", "adopt"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::MissingRequiredArgument);
        let erro =
            Cli::try_parse_from(["nodus", "preset", "adopt", "p.toml", "--json"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn preset_suggest_all_e_opcional() {
        let cli = Cli::try_parse_from(["nodus", "preset", "suggest", "p.toml"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Preset {
                action: PresetAction::Suggest {
                    all: false,
                    json: false,
                    ..
                }
            }
        ));
        let cli = Cli::try_parse_from(["nodus", "preset", "suggest", "p.toml", "--all"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Preset {
                action: PresetAction::Suggest { all: true, .. }
            }
        ));
    }

    #[test]
    fn preset_ignored_nao_leva_argumento() {
        let cli = Cli::try_parse_from(["nodus", "preset", "ignored"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Preset {
                action: PresetAction::Ignored
            }
        ));
        let erro = Cli::try_parse_from(["nodus", "preset", "ignored", "x"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn preset_unignore_leva_um_ou_mais_nomes() {
        let cli = Cli::try_parse_from(["nodus", "preset", "unignore", "a", "org.x.A"]).unwrap();
        match cli.command {
            Command::Preset {
                action: PresetAction::Unignore { names },
            } => assert_eq!(names, ["a", "org.x.A"]),
            outro => panic!("comando errado: {outro:?}"),
        }
        let erro = Cli::try_parse_from(["nodus", "preset", "unignore"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn preset_status_recebe_o_arquivo_rotulos_opcionais_e_json() {
        let cli = Cli::try_parse_from(["nodus", "preset", "status", "p.toml"]).unwrap();
        match cli.command {
            Command::Preset {
                action: PresetAction::Status { file, labels, json },
            } => {
                assert_eq!(file, std::path::PathBuf::from("p.toml"));
                assert!(labels.is_empty());
                assert!(!json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
        let cli = Cli::try_parse_from([
            "nodus", "preset", "status", "p.toml", "pc-a", "pc-b", "--json",
        ])
        .unwrap();
        match cli.command {
            Command::Preset {
                action: PresetAction::Status { labels, json, .. },
            } => {
                assert_eq!(labels, ["pc-a", "pc-b"]);
                assert!(json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
        let erro = Cli::try_parse_from(["nodus", "preset", "status"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn preset_plan_exige_o_arquivo() {
        let erro = Cli::try_parse_from(["nodus", "preset", "plan"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn preset_sem_acao_mostra_a_ajuda() {
        let erro = Cli::try_parse_from(["nodus", "preset"]).unwrap_err();
        assert_eq!(
            erro.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn sem_subcomando_mostra_a_ajuda() {
        let erro = Cli::try_parse_from(["nodus"]).unwrap_err();
        assert_eq!(
            erro.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn subcomando_desconhecido_e_erro_de_uso() {
        let erro = Cli::try_parse_from(["nodus", "apagar-tudo"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::InvalidSubcommand);
    }
}
