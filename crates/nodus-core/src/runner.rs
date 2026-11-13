//! Costura para processos externos: o core pede, quem chama decide como executar.

use thiserror::Error;

/// Executa um programa com lista de argumentos (nunca string de shell, T7) e devolve a saída padrão.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RunnerError {
    /// Programa inexistente no PATH: estado normal para programas opcionais, como o `flatpak`.
    #[error("`{program}` não encontrado")]
    NotFound { program: String },
    #[error("não consegui iniciar `{program}`: {reason}")]
    Spawn { program: String, reason: String },
    #[error("`{program}` terminou com erro (código {code:?})")]
    Failed { program: String, code: Option<i32> },
}
