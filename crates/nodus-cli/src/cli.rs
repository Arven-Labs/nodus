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
    /// Mostra o que o nodus executou (cada `preset apply` e cada reversão), do mais antigo ao mais
    /// novo, com o número de cada entrada (somente leitura). `undo <n>` desfaz uma instalação.
    History {
        /// Emite JSON em vez de texto (só vale para a listagem; o `undo` ignora).
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        action: Option<HistoryAction>,
    },
    /// Lista os repositórios Git filhos diretos de uma pasta e o estado de cada um (somente leitura).
    /// Sem pasta: a de workspace (NODUS_WORKSPACE, ou a detectada; ver `workspace`).
    Repos {
        /// Pasta cujos filhos diretos são examinados.
        dir: Option<PathBuf>,
        /// Faz `fetch` do `origin` antes de ler o estado (usa a rede).
        #[arg(long)]
        fetch: bool,
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
    },
    /// Mostra qual é a pasta de workspace (só lê). Sem argumento: a variável NODUS_WORKSPACE, ou a
    /// detecção entre pastas conhecidas sob o HOME (a com mais repositórios Git).
    Workspace {
        /// Pasta de workspace informada à mão (vence a variável e a detecção).
        dir: Option<PathBuf>,
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
    },
    /// Preset declarativo de pacotes (somente leitura nesta versão: nada é instalado).
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },
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
    },
    /// Lista o que está instalado de propósito nesta máquina e fora do preset, com o trecho para
    /// colar nele (não executa nem escreve nada).
    Suggest {
        /// Arquivo TOML do preset.
        file: PathBuf,
        /// Emite JSON em vez de texto.
        #[arg(long)]
        json: bool,
    },
    /// Instala o que falta: pacotes pacman (`sudo pacman -S --needed`) e depois apps Flatpak
    /// (`flatpak install --user`, remote `flathub`). Cada passo mostra o comando e pede confirmação.
    Apply {
        /// Arquivo TOML do preset.
        file: PathBuf,
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
        assert_eq!(erro.to_string().trim(), "nodusctl 0.0.1");
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
            Command::Repos { dir, fetch, json } => {
                assert_eq!(dir, None);
                assert!(!fetch);
                assert!(!json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
    }

    #[test]
    fn repos_so_com_a_pasta_e_offline_em_texto() {
        let cli = Cli::try_parse_from(["nodus", "repos", "/ws"]).unwrap();
        match cli.command {
            Command::Repos { dir, fetch, json } => {
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
                action: PresetAction::Plan { file, json },
            } => {
                assert_eq!(file, std::path::PathBuf::from("p.toml"));
                assert!(json);
            }
            outro => panic!("comando errado: {outro:?}"),
        }
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
                action: PresetAction::Apply { file },
            } => assert_eq!(file, std::path::PathBuf::from("p.toml")),
            outro => panic!("comando errado: {outro:?}"),
        }
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
