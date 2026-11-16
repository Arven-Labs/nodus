//! P-19: com o stdout fechado (`nodusctl ... | head`, `| jq` ausente) o programa sai em silêncio.
//!
//! O pipe é criado com a ponta de leitura já fechada e só depois o filho é iniciado: assim o `EPIPE`
//! no primeiro `write` é garantido (fechar a leitura com o filho já rodando seria uma corrida).

use std::process::{Command, Stdio};

/// `workspace <pasta>` só lê e sempre imprime uma linha; não depende de pacman nem de Git.
fn workspace() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nodusctl"));
    command.args(["workspace", env!("CARGO_TARGET_TMPDIR")]);
    command
}

#[test]
fn stdout_fechado_sai_em_silencio_com_141() {
    let (leitor, escritor) = std::io::pipe().unwrap();
    drop(leitor);
    let saida = workspace()
        .stdout(escritor)
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert_eq!(saida.status.code(), Some(141), "stderr: {erro}");
    assert_eq!(erro, "", "não pode haver pânico nem mensagem");
}

#[test]
fn stdout_aberto_continua_imprimindo_e_sai_com_0() {
    let saida = workspace().output().unwrap();
    assert!(saida.status.success());
    let texto = String::from_utf8_lossy(&saida.stdout);
    assert!(texto.starts_with("Workspace: "), "{texto}");
}
