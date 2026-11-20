# 0026: GUI desktop (Fase 1.5): Tauri 2 + React chamando `nodusctl --json`, primeira versão só de leitura

**Data:** 2026-11-17

## Contexto
O MVP foi fechado em 2026-11-17 (`v0.1.0`). O dono só vai usar o nodus quando houver interface, então a Fase 1.5 (desktop) é a próxima frente. O ADR 0004 já decidiu que o desktop "consome o contrato que a CLI já expõe (comandos e saída JSON)" e que esse contrato "precisa ser estável e versionado cedo"; o ADR 0005 deixou Tauri/React adiado, sem decisão. A SPEC §8 prevê Tauri + React + TypeScript.

Lido em 2026-11-17 (sem rodar nada):
- Os executores de `apply`, `undo`, `adopt` e `link` ficam em `crates/nodus-cli/src/` (binário) e confirmam por terminal (`is_terminal()`, `main.rs`); `nodus-core` só tem domínio puro (planos, parsers, tipos).
- Há `--json` em `inventory`, `updates`, `repos`, `workspace`, `dotfiles`, `preset plan` e `preset suggest`, `history`. A raiz do JSON **não é uniforme**: `repos`, `dotfiles` e `history` emitem um **array** na raiz; `inventory`, `updates` e `preset plan` emitem um objeto. Nenhum tem campo de versão.
- Precedente no workspace: `warden/desktop` e `anchor/desktop` usam Tauri 2 + React/Vite, com o `src-tauri` dependendo direto dos crates do projeto, e têm workflows de CI separados para frontend, Rust do `src-tauri` e e2e (o do warden roda a tela contra um backend falso no lugar do Tauri, com dados inventados).

A dor concreta: sem interface o dono não usa o produto, e sem uso não há feedback para a Fase 1.5 em diante.

## Decisão
Escolhas do dono em 2026-11-17.
1. **Integração por processo.** A GUI executa `nodusctl <comando> --json` e lê a saída. Não liga nos crates (como o warden faz).
2. **Primeira versão só de leitura:** inventário, atualizações, repositórios e dotfiles (relatório). A GUI não roda `sudo`, não escreve no HOME e não chama comando que escreve (`apply`, `undo`, `adopt`, `link`, `preset adopt`, `workspace set`). Ações viram um ADR próprio (confirmação fora do terminal e elevação de privilégio, AGENTS §4).
3. **Stack:** Tauri 2 + React + TypeScript, num diretório `desktop/` do monorepo (como `warden/desktop`).
4. **Contrato (recomendação, a confirmar):** a GUI e a CLI saem do mesmo repositório e da mesma tag; então a GUI **exige que `nodusctl --version` tenha o mesmo `major.minor` que ela** e recusa o resto com uma mensagem clara. Sem campo de versão novo por enquanto.
5. **A saída do `nodusctl` é dado externo para a GUI:** parsing estrito dos campos conhecidos, nada de `eval` nem HTML injetado, falha visível para JSON inesperado.
6. **Primeira fatia:** uma tela de inventário (`inventory --json`), para provar o caminho ponta a ponta com o menor contrato. Depois, `updates`, `repos` e `dotfiles`.
7. **Testes:** frontend e e2e com um backend falso no lugar do `nodusctl` (padrão do warden), no CI. A GUI não faz parte do binário `nodusctl`: instalar a CLI continua sem Node nem WebKit.

## Alternativas
- **Crate direto (como o warden):** descartada. Exigiria antes mover os executores do `nodus-cli` para um crate compartilhado (refatoração grande, outra decisão). Reabre se spawnar o processo ou o contrato se mostrar ruim (custo médio: trocar o ponto de chamada).
- **Já executar ações na primeira versão:** descartada. Sem terminal não há `sudo` interativo nem o `[s/N]` do nodus; desenhar isso junto com a primeira tela mistura duas decisões difíceis.
- **UI em Rust puro (egui ou similar):** descartada. Foge da SPEC §8 e do padrão do workspace, e a dependência nova passaria pela avaliação do ADR 0013 do workspace.
- **Campo `schema` em cada JSON:** descartada **por ora**. Como `repos`, `dotfiles` e `history` têm array na raiz, adicionar um campo exige embrulhar tudo em `{"schema": 1, "data": ...}` e quebra a forma atual de todos os comandos, inclusive os testes. Reabre quando a GUI e a CLI puderem andar em versões diferentes (por exemplo, GUI falando com o agente de outra máquina, Fase 2).
- **Comando `nodusctl contract`** com a versão do contrato: descartada por ora, pelo mesmo motivo (nova superfície pública sem dor concreta; o `--version` já existe).

## Consequências
- Habilita: uma interface usável sem mexer no núcleo nem na segurança do MVP; qualquer correção de saída da CLI vira correção da GUI sem reescrever a lógica.
- Custa: cada comando é um processo novo (latência de dezenas de ms, aceitável para telas de leitura; não medido); o contrato de saída passa a ser API pública e mudar o JSON quebra a GUI (o teste com backend falso e o `major.minor` travado são a rede de proteção); a GUI só aceita a CLI da mesma versão.
- **Tensão com o ADR 0009 do workspace (build e teste só no CI):** `tauri dev` compila localmente e o frontend traz `node_modules` (100 a 300 MB no `warden/desktop` e no `anchor/desktop`). Proposta: build e testes no CI (workflows separados, como no warden); desenvolvimento local só com exceção pontual do dono, que precisa ser decidida **antes** de existir código.
- Fica para depois, com ADR próprio: ações privilegiadas (pkexec/polkit), GUI falando com agente remoto, campo de versão no JSON, empacotamento (AppImage, PKGBUILD) e assinatura do binário da GUI (o ADR 0010 proíbe binário distribuído sem ADR novo).

## Emenda (2026-11-17): exceção ao ADR 0009 para o desenvolvimento local da GUI
**Decidido pelo dono (opção 1):** o `tauri dev` e o build local da GUI são permitidos **pontualmente**, rodados pelo dono (o hook `guard-heavy` barra o agente; o dono roda com `!` ou no terminal). O CI continua sendo o juiz: testes, clippy, build de produção e e2e da GUI rodam lá, em workflows separados. A exceção cobre só o `desktop/` (Node e `src-tauri`); o resto do nodus segue o ADR 0009. **Disco:** antes do primeiro `npm install`, rodar `scripts/disk-report.sh`; o `node_modules` do warden e do anchor tem de 100 a 300 MB. Se um projeto passar de 2 GB, a limpeza volta a ser decisão do dono. A exceção vale para o desenvolvimento da Fase 1.5 e deve ser revista quando a GUI tiver CI próprio estável. **Medido em 2026-11-17 (primeiro `tauri dev`, 2 min 43 s):** `desktop/src-tauri/target` com **3,1 GB** (build de debug com WebKitGTK) e `desktop/node_modules` com 106 MB; o `nodus` passou de 292 MB para 3,5 GB e o `scripts/disk-report.sh` deu `ALERTA` (limite de 2 GB). A estimativa anterior (100 a 600 MB, vinda do warden) estava errada para o `target`. Decisão de limpeza (`cargo clean` em `desktop/src-tauri`) segue do dono.

## Emenda 2 (2026-11-20): a exceção passa a valer só no PC do dono
O ADR 0017 do workspace (`docs/adr/0017-teste-so-no-github-build-so-no-pc-do-dono.md`: teste só no GitHub, build só no PC do dono, nada disso na VM de desenvolvimento) substitui, para este projeto, a parte "o dono roda com `!`" da emenda acima: o `!` do Claude Code roda na própria VM, então não vale. O que continua: o `tauri dev` e o build local da GUI são do dono, **no PC dele, à mão, fora da sessão do agente**. Na VM, nem o agente nem o `!` rodam `npm run build`, `npm test`, `cargo build` ou `tauri dev` (o hook `guard-heavy` barra o agente). Testes, clippy, build de produção e e2e seguem no CI. O resto da emenda (escopo só do `desktop/`, disco, revisão quando a GUI tiver CI estável) não muda.

## Não verificado
- Tamanho real de um shell Tauri, tempo de build no CI e dependências no Arch (WebKitGTK): não medidos.
- Versões atuais do Tauri 2 e do WebKitGTK: conferir na documentação oficial antes de implementar; meu conhecimento pode estar desatualizado.
- A latência de `nodusctl --json` por chamada, e se `inventory` (hardware, pacotes) é rápido o bastante para uma tela.
- Se o `src-tauri` do warden e do anchor realmente seguem o padrão de testes com backend falso para a parte Rust: li só os nomes dos workflows e o comentário do `desktop-e2e.yml` do warden.
- Este ADR foi escrito por IA e revisado só pelo dono.
