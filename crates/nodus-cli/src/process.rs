//! Execução real de processos externos (implementa `CommandRunner` do core).

#[cfg(test)]
mod tests {
    use super::*;
    use nodus_core::runner::{CommandRunner, RunnerError};

    #[test]
    fn devolve_a_saida_padrao() {
        let saida = RealRunner.run("echo", &["oi"]).unwrap();
        assert_eq!(saida, "oi\n");
    }

    #[test]
    fn argumento_com_metacaracteres_de_shell_e_literal() {
        let saida = RealRunner.run("echo", &["a; rm -rf / $(id)"]).unwrap();
        assert_eq!(saida, "a; rm -rf / $(id)\n");
    }

    #[test]
    fn codigo_de_saida_diferente_de_zero_e_falha() {
        let erro = RealRunner.run("false", &[]).unwrap_err();
        assert_eq!(
            erro,
            RunnerError::Failed {
                program: "false".into(),
                code: Some(1)
            }
        );
    }

    #[test]
    fn programa_inexistente_e_erro_de_inicio() {
        let erro = RealRunner
            .run("nodus-programa-que-nao-existe", &[])
            .unwrap_err();
        assert!(matches!(erro, RunnerError::Spawn { .. }));
    }
}
