# nodus: MVP (Fase 1)

CLI + agente local, só Arch Linux (ADR 0004). Base: SPEC §16.

**Status: fechado pelo dono em 2026-11-17** (versão `v0.1.0`; ver "Fechamento do MVP" abaixo).

## Critérios de pronto
1. Instalar a CLI em um Arch Linux (por código-fonte, ADR 0010).
2. `inventory`: hardware, sistema e aplicativos instalados. (Marcos 2 a 4: sistema, hardware (CPU, RAM, discos), pacotes pacman e apps Flatpak, texto e `--json`. Critério 2 completo no código; sistema, kernel, CPU, memória, pacotes e Flatpak conferidos por leitura contra `/etc/os-release`, `uname`, `/proc` e `pacman -Q` (SESSIONS.md); discos não aparecem como conferidos com `lsblk`.)
3. Criar um preset declarativo (apps e configurações). (Fatia 1: `nodusctl preset init` imprime um preset TOML com os pacotes explícitos, `pacman -Qe`, e os apps Flatpak; só pacotes, sem configurações. **Decidido pelo dono em 2026-11-17:** no MVP, "configurações" são os dotfiles do critério 11; o preset fica só de pacotes e apps.)
4. Reaplicar o preset sem duplicar nem quebrar configurações (idempotência). (Fatia 1: `nodusctl preset plan <arquivo>` mostra o que falta, comparando com `pacman -Q`; `init` seguido de `plan` dá plano vazio. Fatia 2: `nodusctl preset apply <arquivo>` instala o que falta do pacman com `sudo pacman -S --needed`; plano vazio não executa nada. Fatia 5 (ADR 0015): o `apply` instala também os apps Flatpak (`flatpak install --user -- flathub`), em passo próprio com confirmação própria. P-18: `plan` e `apply` simulam `pacman -S --print` e separam os itens que o pacman não encontra nos repositórios (AUR, locais): aparecem como "indisponíveis" e ficam fora do comando, para não abortar o passo inteiro.)
5. Detectar app recém-instalado e perguntar se entra no preset. (Fatia 1, só relatório: `nodusctl preset suggest <arquivo> [--json]` compara `pacman -Qe`, `pacman -Qem` e `flatpak list` com o preset e lista o que está instalado fora dele, com o trecho TOML para colar; pacotes de fora dos repositórios vão à parte, porque o `apply` não os instala. Sem estado: "recém-instalado" é "ainda não adotado". Fatia 2: `nodusctl preset adopt <arquivo>` pergunta item a item (`s`/`N`/`q`) e acrescenta o adotado ao preset sem mexer no resto do arquivo (ADR 0019, PR #59). Fatia 3: `i` no prompt ignora o item para sempre (lista no `config.toml`); `suggest` esconde e conta os ignorados (`--all` mostra); `preset ignored` lista e `preset unignore` desfaz (ADR 0020, PR #61). Critério completo.)
6. Identificar a pasta padrão de workspace. (`nodusctl workspace [pasta] [--json]`, ADR 0017: argumento, depois `NODUS_WORKSPACE`, depois a detecção entre candidatos conhecidos sob o `$HOME` (`Documents/workspace`, `workspace`, `Projects`, `dev`, `src`, ...), a com mais repositórios Git filhos; só relatório, sem estado. O `nodusctl repos` sem pasta usa o workspace resolvido (variável, depois detecção; PR #52), com `Workspace: <pasta> (<origem>)` em stderr. `nodusctl workspace set <pasta>` grava a escolha em `~/.config/nodus/config.toml` e `workspace unset` a esquece; a precedência passa a ser argumento, variável, gravada, detecção (ADR 0018, PR #57).)
7. Descobrir repositórios Git e exibir o estado (somente leitura, mais `fetch`). (Marco 6: `nodusctl repos <pasta> [--fetch] [--json]` lista os filhos diretos com branch, commit, upstream, ahead/behind e URL do `origin`, só por refs com o isolamento do ADR 0009; sem "sujo/limpo", sem `status`. `--fetch` só do `origin`.)
8. Verificar atualizações de pacotes (pacman) e apps (Flatpak).
9. Exibir um plano antes de qualquer mudança; nada executa sem confirmação. (`preset apply`: plano, comando exato, `Executar? [s/N]` e o `[Y/n]` do pacman; sem terminal interativo, recusa.)
10. Registrar o que mudou e permitir reversão quando suportada. (Fatia 3: o registro existe, `nodusctl history` lista cada `preset apply` executado. A reversão de pacman existe (`nodusctl history undo <n>`, ADR 0011, fatia 4) e foi verificada no Arch real; a de Flatpak existe desde o ADR 0029 (`flatpak uninstall --user`, sem simulação e sem prompt do Flatpak, `[s/N]` do nodus como única confirmação) e ainda não foi verificada no Arch real pela CLI.)
11. Relatar o estado dos dotfiles do usuário e vinculá-los a uma pasta-fonte (SPEC §16, SECURITY "vincular dotfile"). **Decidido pelo dono em 2026-11-16: vincular entra no MVP.** (Fatia 1, só relatório, ADR 0021, PR #64, CI verde e verificado no Arch real em 2026-11-16 (lista, denylist no config e `--json`; symlink real não): `nodusctl dotfiles [--json]` sobre uma lista curada (padrão no código mais `dotfiles_extra` e `dotfiles_skip` no `config.toml`): existe, arquivo, diretório ou symlink (com o alvo, sem seguir) e tamanho; nunca lê conteúdo; denylist fixa de segredos (`.ssh`, `.gnupg`, `.config/gh` etc.). Fatia 2 (ADR 0022, PR #66, CI verde; verificado no Arch em 2026-11-16: ensaio em HOME temporário e adoção real do `.bash_logout`): `nodusctl dotfiles adopt <caminho>` copia o arquivo real para a pasta-fonte (`dotfiles_dir`, espelha o HOME, repo privado), confere byte a byte e troca o original por symlink com `rename` atômico, com plano, confirmação e histórico v4; nunca sobrescreve. Fatia 3a (ADR 0023, PR #68, CI verde; verificado no Arch em 2026-11-16 em HOME temporário): `nodusctl history undo <n>` numa adoção troca o symlink por uma cópia do conteúdo atual da fonte (a fonte fica), com plano, confirmação e linha v4 `remove` no histórico; recusa se o symlink divergiu, a fonte sumiu ou a adoção já foi desfeita. Fatia 4 (ADR 0024, PR #70, CI verde; verificado no Arch em 2026-11-16 em HOMEs temporários): `nodusctl dotfiles link <caminho>` cria no HOME o symlink para um arquivo já presente na pasta-fonte (máquina nova), com plano, aviso de execução de código, confirmação e linha v4 `link`; cria as pastas-pai (0700); nunca sobrescreve (destino existente recusa; já ligado recusa com mensagem própria). Fatia 5 (ADR 0025, PR #73, CI verde; verificado no Arch em 2026-11-17 em HOMEs temporários): `history undo <n>` numa ligação remove só o symlink (alvo idêntico em texto; recusa se virou arquivo, sumiu ou aponta para outro lugar), mantém a fonte e as pastas-pai, e grava a mesma linha v4 `remove` do undo da adoção (o `history` mostra "ligação desfeita"). Fica: `link` de vários de uma vez.)

## Fechamento do MVP (revisão de 2026-11-17)

Revisão só de leitura de `MVP.md`, `SESSIONS.md`, `PENDING.md`, README e ADR 0010; nada foi rodado nesta revisão. Confiança média: herda o que o `SESSIONS.md` registra.

| # | Estado | Lacuna |
|---|---|---|
| 1 | Pronto | Tag `v0.1.0` assinada, "Verified" no GitHub e instalação medida em container Arch limpo (2026-11-17); o container usa o kernel e o hardware do host |
| 2 | Pronto | Discos não conferidos com `lsblk` |
| 3 | Pronto | Só pacotes e apps; configurações = dotfiles (critério 11), decisão de 2026-11-17 |
| 4 | Pronto | Custo do `plan` com muitos itens faltando não medido |
| 5 | Pronto | Flatpak no `adopt` e no `ignore` não exercitado no Arch real |
| 6 | Pronto | — |
| 7 | Pronto | Sem "sujo/limpo", de propósito |
| 8 | Pronto | Exit code sem pendências não medido |
| 9 | Pronto | O pacman pergunta de novo (`Proceed? [Y/n]`) |
| 10 | Pronto com lacuna | Reversão de Flatpak existe desde o ADR 0029 (CI verde, mutação pega); lacuna: ainda não verificada no Arch real pela CLI |
| 11 | Pronto com lacuna | Logout real, KDE, permissão negada, nano, vim; `link` em lote não existe |

**O que bloqueava declarar o MVP fechado:** só o critério 1, resolvido em 2026-11-17: tag `v0.1.0` (commit `6b2e4ed`, CI verde), `verified: true` na API do GitHub, e `project/spikes/tag-limpa/verificar.sh` deu 7 OK e 0 FALHA num container Arch (chave idêntica na API e no README, `git tag -v` exit 0, chave errada exit 1, `Cargo.toml` da tag em `0.1.0`, instalação `--locked --tag` e `--version`). A P-02 (nome) não bloqueia a tag, que é só código-fonte, mas bloqueia divulgação e pacote. O dono decide se declara o MVP fechado.

**Só refinamento:** `dotfiles dir set`, `link` em lote, nano e vim, hash do commit em `--version`. (A reversão de Flatpak saiu desta lista: ADR 0029.)

## Fora do MVP
Desktop (Fase 1.5), Windows, Core e multi-device (Fase 2), outras distros, periféricos, overclock, pull/reset de Git, sincronização de segredos, IA, métricas e empresa.

## Regras herdadas da SPEC §15
Sem scripts genéricos de otimização; segredos nunca entram em presets; mudanças sempre via plano.
