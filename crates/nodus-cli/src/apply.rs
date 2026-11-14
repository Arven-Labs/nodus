//! `nodus preset apply`: a primeira operação privilegiada (SECURITY.md §4). Mostra o comando exato,
//! pede confirmação (padrão N) e só então executa; o `[Y/n]` do pacman vem depois, com as dependências.

#[cfg(test)]
mod tests {
    use super::*;
    use nodus_core::preset::Plan;
    use std::io::Cursor;

    fn plano(pacman: &[&str], flatpak: &[&str]) -> Plan {
        Plan {
            pacman_missing: pacman.iter().map(|s| s.to_string()).collect(),
            pacman_present: 3,
            flatpak_missing: flatpak.iter().map(|s| s.to_string()).collect(),
            flatpak_present: 0,
            flatpak_installed: true,
        }
    }

    /// Roda `apply` com a entrada dada e um executor falso que só registra o que receberia.
    fn rodar(
        plan: &Plan,
        interactive: bool,
        entrada: &str,
        falha: bool,
    ) -> (anyhow::Result<Outcome>, String, Vec<Vec<String>>) {
        let mut input = Cursor::new(entrada.as_bytes().to_vec());
        let mut output = Vec::new();
        let mut executados = Vec::new();
        let resultado = apply(plan, interactive, &mut input, &mut output, |cmd| {
            executados.push(cmd.to_vec());
            if falha {
                anyhow::bail!("`sudo` terminou com erro (código Some(1))")
            }
            Ok(())
        });
        (resultado, String::from_utf8(output).unwrap(), executados)
    }

    // --- confirm ---

    #[test]
    fn so_s_ou_sim_confirmam() {
        for sim in ["s\n", "S\n", "sim\n", "SIM\n", "  s  \n", "s"] {
            let mut out = Vec::new();
            assert!(
                confirm(&mut Cursor::new(sim), &mut out).unwrap(),
                "{sim:?} deveria confirmar"
            );
            assert_eq!(String::from_utf8(out).unwrap(), "Executar? [s/N] ");
        }
    }

    #[test]
    fn qualquer_outra_resposta_e_nao() {
        let recusas = [
            "\n",
            "",
            "n\n",
            "nao\n",
            "y\n",
            "yes\n",
            "sim por favor\n",
            "ss\n",
        ];
        for nao in recusas {
            assert!(
                !confirm(&mut Cursor::new(nao), &mut Vec::new()).unwrap(),
                "{nao:?} não deveria confirmar"
            );
        }
    }

    // --- apply ---

    #[test]
    fn plano_vazio_nao_pergunta_nem_executa() {
        let (r, out, exec) = rodar(&plano(&[], &[]), true, "s\n", false);
        assert_eq!(r.unwrap(), Outcome::NothingToDo);
        assert!(exec.is_empty());
        assert!(!out.contains("Executar?"));
    }

    #[test]
    fn confirmado_executa_o_comando_exato_uma_vez() {
        let (r, out, exec) = rodar(&plano(&["sl", "zsh"], &[]), true, "s\n", false);
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(
            exec,
            [["sudo", "pacman", "-S", "--needed", "--", "sl", "zsh"]]
        );
        assert_eq!(
            out,
            "Comando: sudo pacman -S --needed -- sl zsh\nExecutar? [s/N] "
        );
    }

    #[test]
    fn recusado_nao_executa() {
        let (r, out, exec) = rodar(&plano(&["sl"], &[]), true, "\n", false);
        assert_eq!(r.unwrap(), Outcome::Declined);
        assert!(exec.is_empty());
        assert!(out.ends_with("Nada foi executado.\n"));
    }

    #[test]
    fn fim_da_entrada_e_recusa() {
        let (r, _, exec) = rodar(&plano(&["sl"], &[]), true, "", false);
        assert_eq!(r.unwrap(), Outcome::Declined);
        assert!(exec.is_empty());
    }

    #[test]
    fn sem_terminal_interativo_e_erro_e_nao_executa() {
        let (r, out, exec) = rodar(&plano(&["sl"], &[]), false, "s\n", false);
        let erro = r.unwrap_err().to_string();
        assert!(erro.contains("terminal interativo"), "{erro}");
        assert!(exec.is_empty());
        assert!(!out.contains("Executar?"));
    }

    #[test]
    fn falha_do_sudo_ou_do_pacman_vira_erro() {
        let (r, _, exec) = rodar(&plano(&["sl"], &[]), true, "s\n", true);
        assert!(r.is_err());
        assert_eq!(exec.len(), 1);
    }

    #[test]
    fn so_flatpak_faltando_avisa_e_nao_executa() {
        let (r, out, exec) = rodar(&plano(&[], &["org.a.B", "org.c.D"]), true, "s\n", false);
        assert_eq!(r.unwrap(), Outcome::FlatpakOnly);
        assert!(exec.is_empty());
        assert_eq!(
            out,
            "aviso: Flatpak ainda não é instalado pelo nodus: 2 app(s) do preset ficam de fora\n"
        );
    }

    #[test]
    fn pacman_e_flatpak_faltando_avisa_e_executa_so_o_pacman() {
        let (r, out, exec) = rodar(&plano(&["sl"], &["org.a.B"]), true, "s\n", false);
        assert_eq!(r.unwrap(), Outcome::Executed);
        assert_eq!(exec.len(), 1);
        assert!(out.starts_with("aviso: Flatpak ainda não é instalado pelo nodus: 1 app(s)"));
    }
}
