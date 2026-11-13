//! Execução real de processos externos (implementa `CommandRunner` do core).

use nodus_core::runner::{CommandRunner, RunnerError};
use std::io::ErrorKind;
use std::process::Command;

pub struct RealRunner;

impl CommandRunner for RealRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, RunnerError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| match e.kind() {
                ErrorKind::NotFound => RunnerError::NotFound {
                    program: program.to_string(),
                },
                _ => RunnerError::Spawn {
                    program: program.to_string(),
                    reason: e.to_string(),
                },
            })?;
        if !output.status.success() {
            return Err(RunnerError::Failed {
                program: program.to_string(),
                code: output.status.code(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

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
    fn programa_inexistente_e_nao_encontrado() {
        let erro = RealRunner
            .run("nodus-programa-que-nao-existe", &[])
            .unwrap_err();
        assert_eq!(
            erro,
            RunnerError::NotFound {
                program: "nodus-programa-que-nao-existe".into()
            }
        );
    }

    #[test]
    fn programa_sem_permissao_de_execucao_e_erro_de_inicio_e_nao_nao_encontrado() {
        let dir = std::env::temp_dir().join(format!("nodus-teste-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let arquivo = dir.join("sem-permissao");
        std::fs::write(&arquivo, "#!/bin/sh\n").unwrap();
        let erro = RealRunner.run(arquivo.to_str().unwrap(), &[]).unwrap_err();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(erro, RunnerError::Spawn { .. }));
    }
}
