# 0027: Ações na GUI: nenhuma escrita roda dentro dela; o botão abre um terminal com o comando da CLI

**Data:** 2026-11-19

## Contexto
O ADR 0026 deixou a GUI só de leitura e adiou para um ADR próprio "confirmação fora do terminal e elevação de privilégio" (AGENTS §4). As abas Preset e Histórico já mostram o que falta instalar e o que dá para desfazer, mas para agir o dono volta ao terminal e redigita o comando.

Lido em 2026-11-19 (`main.rs`, `apply.rs`, `dotfiles_*.rs`, `SECURITY.md`): todo executor que escreve **recusa rodar sem terminal** (`is_terminal()`), e o `sudo` e o `[Y/n]` do pacman herdam esse terminal. A GUI não tem terminal. Os comandos que escrevem são: `preset apply` e `history undo` (privilegiados: `sudo pacman`), e `dotfiles adopt`, `dotfiles link`, `workspace set`/`unset`, `preset adopt`, `ignored`/`unignore` e o undo de dotfile (escrita de usuário, sem root). O `SECURITY.md` §4 exige plano e confirmação **por operação** nas duas classes, e proíbe "sim para tudo" na privilegiada.

A dor concreta: o ciclo "ver o plano na GUI, ir ao terminal, redigitar" é o atrito que a GUI existe para tirar, mas o jeito de tirá-lo não pode abrir uma superfície de elevação de privilégio.

## Decisão
Escolhas do dono em 2026-11-19.
1. **A GUI não executa nenhum comando que escreve no próprio processo.** O botão de ação **abre um emulador de terminal já rodando** `nodusctl <comando> <argumentos>`. O plano, o `[s/N]`, o `sudo` e o `[Y/n]` do pacman continuam sendo da CLI, no terminal. A GUI não vê senha, não roda root, não confirma por conta própria e nunca passa `--yes`, `-y` nem `--noconfirm`.
2. **Uma regra só para as duas classes (tudo ou nada).** As escritas sem root (`dotfiles adopt/link/undo`, `workspace set/unset`, `preset adopt/ignore`) seguem o mesmo mecanismo das privilegiadas. A pergunta de auditoria é uma: "a GUI executa alguma escrita?", e a resposta é não.
3. **Lista fechada, separada da de leitura.** As ações ficam num `enum` próprio no Rust (`src-tauri/src/nodusctl.rs` ou módulo novo), distinto de `Allowed` (que continua só leitura). Cada ação tem os argumentos fixos. Argumento que vem da tela só entra se o Rust o converter para um tipo estrito (por exemplo, o `<n>` do `history undo` como inteiro sem sinal), e isso é definido no PR da ação, não aqui. O arquivo do preset continua vindo de `NODUS_PRESET` (absoluto, lido no Rust). A primeira ação (`preset apply`) não tem argumento vindo do frontend.
4. **Execução sem shell.** O terminal e o comando são lançados com lista de argumentos (T7), nunca por string interpolada. Se for preciso um invólucro para a janela esperar uma tecla ao fim, o texto do script é **constante** e os argumentos entram como parâmetros posicionais, sem interpolação.
5. **Fallback:** se nenhum terminal for encontrado, a GUI mostra o comando exato com um botão Copiar, e o dono o cola no terminal.
6. **Depois da ação**, a GUI não tenta saber o resultado: a janela do terminal mostra o da CLI, e uma ação "Atualizar" relê a aba (leitura, barata). A GUI não tem como detectar o fim do comando.

## Alternativas
- **Só mostrar o comando, sem botão que abre terminal:** a mais simples e segura, sem processo novo; mas não tira o atrito do "um clique". Fica como o fallback do item 5, e como saída se descobrirmos que abrir terminal é frágil.
- **Confirmar na GUI com polkit (`pkexec`) e modo não interativo na CLI:** descartada. Exige uma API de escrita nova na CLI (por exemplo, um token do plano), regra e agente do polkit e um vetor T4 novo; a confirmação sairia do lugar onde o comando exato fica à vista. Custo de reverter **difícil** (contrato público da CLI mais política do polkit). Reabre se abrir terminal se mostrar ruim de usar.
- **Por classe (sem root confirma num diálogo da GUI; só privilegiadas usam o terminal):** descartada por ora. Exige o modo não interativo da CLI para essas escritas e duas regras em vez de uma; cada comando novo passaria a exigir a decisão "em qual classe cai", e um erro de classificação viraria escrita sem confirmação visível. Reabre quando houver dor concreta em ações simples como `workspace set`.
- **Ligar nos crates do `nodus-core` para executar na GUI:** já descartada no ADR 0026.

## Consequências
- **Custo de reverter: fácil.** É um botão e um módulo; nada persistente, nenhum dado nem contrato novo. Relaxar a regra depois (diálogo na GUI, polkit) é a decisão difícil e pede ADR novo.
- Habilita: botões de ação sem tocar na CLI nem no modelo de ameaças do MVP; a confirmação continua onde o comando exato está à vista (`SECURITY.md` §4).
- Custa: depende de haver um terminal; o fluxo sai da janela da GUI; a GUI não sabe quando acabou. Não testável de ponta a ponta no CI (não há terminal gráfico): o teste usa um terminal falso que grava o `argv`, no padrão do `falso` do `nodusctl.rs`, e prova que só a lista fechada vira processo.
- Superfície nova: lançar um processo é execução (T4, T7). A mitigação é a lista fechada, o `argv` sem shell, o caminho vindo do Rust e a recusa de qualquer argumento que o frontend tente embutir; o teste de mutação (allowlist por prefixo) é obrigatório, como nas abas de leitura.

## Medido em 2026-11-19 (nesta máquina, sem lançar nada)
- Terminal instalado: só `konsole` 26.08.3, com `-e <cmd>` ("catches all" os argumentos seguintes) e `--hold`/`--noclose`. `xdg-terminal-exec` não está instalado; `$TERMINAL` está vazio.

## Emenda (2026-11-19): medido antes da primeira ação (PR #99)
`konsole` 26.08.3, `konsole -e sh -c <script> sh a "b c" "x'y"`: os argumentos chegam intactos, um por parâmetro; `$0` é o `sh` do próprio comando; há tty; `PATH`, `HOME`, `DISPLAY` e `WAYLAND_DISPLAY` são herdados. O processo do `konsole` **não** retorna cedo: um comando de 2 s manteve o `konsole` vivo por 2,2 s. Consequência: a GUI não espera por ele (`spawn` mais uma thread com `wait()`, sem zumbi). O invólucro é uma constante de uma linha (`"$@"; …; read _`) que mostra o código de saída e espera o Enter, então não depende de `--hold`, que é do `konsole`. Implementado só para `preset apply` e só para o `konsole`; cada terminal novo entra depois de medido. Medição feita no terminal do Claude Code, não a partir do app Tauri.

## Emenda (2026-11-19): regra do argumento vindo da tela (PR #100, `history undo`)
O único argumento que a tela pode mandar é **numérico estrito**: `u32`, o mesmo tipo da CLI (`history undo <n>`, `n >= 1`). Um inteiro não tem sintaxe para escapar de nada (aspas, `;`, `../`), e o Tauri já recusa texto, negativo e fracionário ao desserializar, antes do nosso código. O Rust ainda recusa `n` ausente ou 0 na `history_undo` e qualquer `n` na `preset_apply` (`invalid_argument`: argumento que a ação não definiu é recusado, não ignorado). O número é estável porque o histórico só acrescenta; se o arquivo for editado à mão, o plano que a CLI mostra antes de confirmar é a última defesa. **Texto e caminho vindos da tela continuam fora**: `dotfiles adopt/link` e `workspace set` pedem desenho próprio (lista de permitidos, symlink, `~`) antes de entrar.

## Não verificado
- Como escolher o terminal em outras máquinas (`xdg-terminal-exec`, `$TERMINAL` vem de ambiente externo, lista curta): decidir no PR de implementação, depois de medir. As flags de `-e` diferem entre emuladores.
- Se `konsole -e nodusctl preset apply /caminho com espaço/p.toml` passa o espaço como um argumento só; se uma janela aberta a partir de um app Tauri herda `HOME`, `PATH` e `SUDO_*` do jeito esperado; e como manter a janela aberta ao fim (`--hold` é do `konsole`; o invólucro com `sh` constante é a alternativa).
- Wayland versus X11 ao lançar o terminal pelo app Tauri.
- Este ADR foi escrito por IA e revisado só pelo dono.
