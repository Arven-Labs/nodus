//! `nodusctl preset adopt` (critério 5 do MVP): pergunta, item a item, se o que está instalado de
//! propósito e fora do preset entra nele. Aqui só a conversa; quem edita o arquivo é `preset_file`.

use anyhow::{Result, bail};
use nodus_core::preset::Suggestions;
use std::io::{self, BufRead, Write};

/// O que o dono respondeu, na ordem em que foi perguntado: o que entra no preset e o que ele mandou
/// ignorar para sempre (`i`).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Adoption {
    pub pacman: Vec<String>,
    pub flatpak: Vec<String>,
    pub ignored_pacman: Vec<String>,
    pub ignored_flatpak: Vec<String>,
}

impl Adoption {
    /// Nada foi adotado (o preset não precisa ser escrito). Pode haver ignorados.
    pub fn is_empty(&self) -> bool {
        self.pacman.is_empty() && self.flatpak.is_empty()
    }

    /// Algo foi mandado ignorar (o config precisa ser escrito).
    pub fn has_ignored(&self) -> bool {
        !self.ignored_pacman.is_empty() || !self.ignored_flatpak.is_empty()
    }
}

enum Answer {
    Adopt,
    Ignore,
    Skip,
    Quit,
}

/// `s` ou `sim` adotam; `i` ou `ignorar` mandam ignorar para sempre; `q` ou `sair` e o fim da
/// entrada param de perguntar; qualquer outra coisa (inclusive vazio) pula só este item, e ele volta
/// a ser perguntado na próxima rodada. Não existe "sim para todos", como no `apply`.
fn read_answer(
    input: &mut impl BufRead,
    output: &mut impl Write,
    name: &str,
    kind: &str,
) -> io::Result<Answer> {
    write!(output, "Adotar {name} ({kind})? [s/N/i/q] ")?;
    output.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        writeln!(output)?;
        return Ok(Answer::Quit);
    }
    Ok(match line.trim().to_lowercase().as_str() {
        "s" | "sim" => Answer::Adopt,
        "i" | "ignorar" => Answer::Ignore,
        "q" | "sair" => Answer::Quit,
        _ => Answer::Skip,
    })
}

/// O resultado de perguntar uma lista: adotados, ignorados e se o dono mandou parar.
struct Asked {
    adopted: Vec<String>,
    ignored: Vec<String>,
    quit: bool,
}

/// Pergunta cada nome de `names`.
fn ask_list(
    input: &mut impl BufRead,
    output: &mut impl Write,
    kind: &str,
    names: &[String],
) -> io::Result<Asked> {
    let mut asked = Asked {
        adopted: Vec::new(),
        ignored: Vec::new(),
        quit: false,
    };
    for name in names {
        match read_answer(input, output, name, kind)? {
            Answer::Adopt => asked.adopted.push(name.clone()),
            Answer::Ignore => asked.ignored.push(name.clone()),
            Answer::Skip => {}
            Answer::Quit => {
                asked.quit = true;
                break;
            }
        }
    }
    Ok(asked)
}

/// Faz as perguntas, pacman primeiro e depois Flatpak. `interactive`: a entrada é um terminal (sem
/// ele não há como perguntar, e nada é escrito). Os de fora dos repositórios (AUR e locais) só são
/// listados: o `apply` não os instala, então adotá-los deixaria o preset impossível de aplicar
/// (ADR 0016).
pub fn ask(
    suggestions: &Suggestions,
    interactive: bool,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Adoption> {
    if !interactive {
        bail!(
            "o `preset adopt` pergunta item a item e precisa de um terminal interativo: nada foi escrito"
        );
    }
    if !suggestions.foreign_extra.is_empty() {
        writeln!(
            output,
            "Fora dos repositórios (AUR ou locais; o `apply` não os instala, então não pergunto): {}",
            suggestions.foreign_extra.join(", ")
        )?;
    }
    let mut adoption = Adoption::default();
    let pacman = ask_list(input, output, "pacman", &suggestions.pacman_extra)?;
    adoption.pacman = pacman.adopted;
    adoption.ignored_pacman = pacman.ignored;
    if !pacman.quit {
        let flatpak = ask_list(input, output, "flatpak", &suggestions.flatpak_extra)?;
        adoption.flatpak = flatpak.adopted;
        adoption.ignored_flatpak = flatpak.ignored;
    }
    Ok(adoption)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodus_core::preset::Suggestions;

    fn sugestoes(pacman: &[&str], foreign: &[&str], flatpak: &[&str]) -> Suggestions {
        Suggestions {
            pacman_extra: pacman.iter().map(|s| s.to_string()).collect(),
            foreign_extra: foreign.iter().map(|s| s.to_string()).collect(),
            flatpak_extra: flatpak.iter().map(|s| s.to_string()).collect(),
            flatpak_installed: true,
        }
    }

    /// Roda `ask` com terminal interativo e a entrada dada; devolve o resultado e o que foi escrito.
    fn rodar(sug: &Suggestions, entrada: &str) -> (Result<Adoption>, String) {
        let mut input = entrada.as_bytes();
        let mut output = Vec::new();
        let r = ask(sug, true, &mut input, &mut output);
        (r, String::from_utf8(output).unwrap())
    }

    fn nomes(lista: &[&str]) -> Vec<String> {
        lista.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn s_sim_e_maiusculas_adotam() {
        let sug = sugestoes(&["a", "b", "c", "d"], &[], &[]);
        let (r, _) = rodar(&sug, "s\nsim\nS\nSIM\n");
        assert_eq!(r.unwrap().pacman, nomes(&["a", "b", "c", "d"]));
    }

    #[test]
    fn vazio_n_e_qualquer_outra_coisa_pulam() {
        let sug = sugestoes(&["a", "b", "c", "d"], &[], &[]);
        let (r, out) = rodar(&sug, "\nn\nnão\nquero\n");
        let adocao = r.unwrap();
        assert!(adocao.is_empty());
        // Pulou os quatro: perguntou os quatro.
        assert_eq!(out.matches("Adotar ").count(), 4, "{out}");
    }

    #[test]
    fn q_para_de_perguntar_e_guarda_o_que_ja_foi_adotado() {
        let sug = sugestoes(&["a", "b", "c", "d"], &[], &[]);
        let (r, out) = rodar(&sug, "s\nq\ns\ns\n");
        assert_eq!(r.unwrap().pacman, nomes(&["a"]));
        assert_eq!(out.matches("Adotar ").count(), 2, "{out}");
        assert!(!out.contains("Adotar c"), "{out}");
    }

    #[test]
    fn sair_e_q_maiusculo_tambem_param() {
        for parada in ["sair", "Q", "SAIR"] {
            let sug = sugestoes(&["a", "b"], &[], &[]);
            let (r, out) = rodar(&sug, &format!("{parada}\ns\n"));
            assert!(r.unwrap().is_empty(), "{parada}");
            assert_eq!(out.matches("Adotar ").count(), 1, "{parada}: {out}");
        }
    }

    #[test]
    fn fim_da_entrada_para_e_guarda_o_que_ja_foi_adotado() {
        let sug = sugestoes(&["a", "b", "c"], &[], &[]);
        let (r, out) = rodar(&sug, "s\n");
        assert_eq!(r.unwrap().pacman, nomes(&["a"]));
        assert_eq!(out.matches("Adotar ").count(), 2, "{out}");
    }

    #[test]
    fn pergunta_o_pacman_antes_do_flatpak_com_o_tipo_na_pergunta() {
        let sug = sugestoes(&["vim"], &[], &["org.a.B"]);
        let (r, out) = rodar(&sug, "s\ns\n");
        let adocao = r.unwrap();
        assert_eq!(adocao.pacman, nomes(&["vim"]));
        assert_eq!(adocao.flatpak, nomes(&["org.a.B"]));
        let vim = out.find("Adotar vim (pacman)? [s/N/i/q]").expect(&out);
        let app = out.find("Adotar org.a.B (flatpak)? [s/N/i/q]").expect(&out);
        assert!(vim < app, "{out}");
    }

    #[test]
    fn q_no_pacman_nao_pergunta_o_flatpak() {
        let sug = sugestoes(&["vim"], &[], &["org.a.B"]);
        let (r, out) = rodar(&sug, "q\ns\n");
        assert!(r.unwrap().is_empty());
        assert!(!out.contains("org.a.B"), "{out}");
    }

    #[test]
    fn os_de_fora_dos_repositorios_sao_listados_e_nunca_perguntados() {
        let sug = sugestoes(&["vim"], &["brave-bin", "yay"], &[]);
        let (r, out) = rodar(&sug, "s\n");
        let adocao = r.unwrap();
        assert_eq!(adocao.pacman, nomes(&["vim"]));
        assert!(adocao.flatpak.is_empty());
        assert!(out.contains("brave-bin") && out.contains("yay"), "{out}");
        assert!(!out.contains("Adotar brave-bin"), "{out}");
        assert!(!out.contains("Adotar yay"), "{out}");
    }

    #[test]
    fn so_os_de_fora_dos_repositorios_nao_pergunta_nada() {
        let sug = sugestoes(&[], &["brave-bin"], &[]);
        let (r, out) = rodar(&sug, "s\n");
        assert!(r.unwrap().is_empty());
        assert!(!out.contains("Adotar "), "{out}");
    }

    #[test]
    fn sem_nada_fora_do_preset_nao_pergunta() {
        let (r, out) = rodar(&sugestoes(&[], &[], &[]), "s\n");
        assert!(r.unwrap().is_empty());
        assert!(!out.contains("Adotar "), "{out}");
    }

    #[test]
    fn i_e_ignorar_vao_para_a_lista_de_ignorados_e_nao_para_o_preset() {
        let sug = sugestoes(&["a", "b", "c"], &[], &["org.x.A"]);
        let (r, _) = rodar(&sug, "i\nignorar\nI\nIGNORAR\n");
        let adocao = r.unwrap();
        assert!(adocao.is_empty(), "nada foi adotado");
        assert!(adocao.has_ignored());
        assert_eq!(adocao.ignored_pacman, nomes(&["a", "b", "c"]));
        assert_eq!(adocao.ignored_flatpak, nomes(&["org.x.A"]));
    }

    #[test]
    fn adotar_pular_e_ignorar_na_mesma_rodada() {
        let sug = sugestoes(&["a", "b", "c"], &[], &[]);
        let (r, _) = rodar(&sug, "s\nn\ni\n");
        let adocao = r.unwrap();
        assert_eq!(adocao.pacman, nomes(&["a"]));
        assert_eq!(adocao.ignored_pacman, nomes(&["c"]));
        assert!(adocao.ignored_flatpak.is_empty());
    }

    #[test]
    fn q_guarda_o_que_ja_foi_ignorado() {
        let sug = sugestoes(&["a", "b", "c"], &[], &["org.x.A"]);
        let (r, out) = rodar(&sug, "i\nq\ni\n");
        let adocao = r.unwrap();
        assert_eq!(adocao.ignored_pacman, nomes(&["a"]));
        assert!(adocao.ignored_flatpak.is_empty());
        assert_eq!(out.matches("Adotar ").count(), 2, "{out}");
    }

    #[test]
    fn fim_da_entrada_guarda_o_que_ja_foi_ignorado() {
        let sug = sugestoes(&["a", "b"], &[], &[]);
        let (r, _) = rodar(&sug, "i\n");
        assert_eq!(r.unwrap().ignored_pacman, nomes(&["a"]));
    }

    #[test]
    fn sem_ignorar_nada_has_ignored_e_falso() {
        let sug = sugestoes(&["a"], &[], &[]);
        let (r, _) = rodar(&sug, "s\n");
        assert!(!r.unwrap().has_ignored());
    }

    #[test]
    fn sem_terminal_interativo_e_erro_e_nao_pergunta() {
        let sug = sugestoes(&["vim"], &[], &["org.a.B"]);
        let mut input = "s\ns\n".as_bytes();
        let mut output = Vec::new();
        let erro = ask(&sug, false, &mut input, &mut output)
            .unwrap_err()
            .to_string();
        assert!(erro.contains("terminal interativo"), "{erro}");
        assert!(output.is_empty());
    }
}
