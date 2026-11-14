use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Plataforma aberta para padronizar e administrar ambientes de computadores.
#[derive(Parser, Debug)]
#[command(name = "nodus", version = nodus_core::VERSION, arg_required_else_help = true)]
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
    /// Lista os repositórios Git filhos diretos de uma pasta e o estado de cada um (somente leitura).
    Repos {
        /// Pasta cujos filhos diretos são examinados.
        dir: PathBuf,
        /// Faz `fetch` do `origin` antes de ler o estado (usa a rede).
        #[arg(long)]
        fetch: bool,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser, error::ErrorKind};

    #[test]
    fn version_imprime_nome_e_versao() {
        let erro = Cli::try_parse_from(["nodus", "--version"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::DisplayVersion);
        assert_eq!(erro.to_string().trim(), "nodus 0.0.1");
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
    fn repos_exige_a_pasta() {
        let erro = Cli::try_parse_from(["nodus", "repos"]).unwrap_err();
        assert_eq!(erro.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn repos_so_com_a_pasta_e_offline_em_texto() {
        let cli = Cli::try_parse_from(["nodus", "repos", "/ws"]).unwrap();
        match cli.command {
            Command::Repos { dir, fetch, json } => {
                assert_eq!(dir, std::path::PathBuf::from("/ws"));
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
