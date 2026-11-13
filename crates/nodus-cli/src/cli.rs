use clap::Parser;

/// Plataforma aberta para padronizar e administrar ambientes de computadores.
#[derive(Parser, Debug)]
#[command(name = "nodus", version = nodus_core::VERSION)]
pub struct Cli {}

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
