# nodus: Modelo de ameaças e permissões (MVP)

**Escopo:** MVP da Fase 1 (ADR 0004): CLI + agente local no Arch Linux, sem rede, sem Core, sem multi-device. Core remoto, plugins e IA estão fora deste modelo (seção 8).
**Status:** rascunho da Fase 0, não revisado por terceiros. Aplica os princípios da SPEC §7, §10, §11 e §15; não cria decisão nova. Este documento não prova que o código futuro é seguro: ele define o que o código precisa provar.

## 1. Ativos

| Ativo | Por que importa |
|---|---|
| Dotfiles e configs do usuário | O nodus escreve neles; erro ou abuso quebra o ambiente |
| Lista de apps e hardware (inventário) | Dado pessoal; revela a máquina |
| Repositórios Git do usuário | Código e histórico; o nodus só lê e faz `fetch` |
| Estado local do nodus (presets, histórico, reversão) | Se adulterado, a reversão restaura o estado errado |
| Segredos presentes na máquina (`~/.ssh`, `.env`, tokens) | O nodus **não deve ler, copiar nem registrar** (SPEC §10.2) |
| Privilégio de root (via `sudo`/`pacman`) | O maior dano possível se o nodus for induzido a usá-lo |

## 2. Fronteiras de confiança

Tudo abaixo é **entrada não confiável** e deve ser validado antes de uso:
- **Preset**: arquivo editável, pode vir de terceiro (outro dispositivo, repositório, download).
- **Saída de pacman e Flatpak**: nomes de pacote e metadados.
- **Repositórios Git analisados**: `.git/config`, hooks, nomes de branch e de remoto.
- **Caminhos** vindos do preset ou do sistema de arquivos (symlinks, `..`).
- **Plugins futuros** (SPEC §11): código não confiável até validação.

Confiável por definição: o binário do nodus instalado pelo usuário e o usuário que o executa.

## 3. Ameaças e mitigações

| # | Ameaça | Mitigação exigida | Origem |
|---|---|---|---|
| T1 | Preset malicioso executa comando arbitrário | Preset é **dado**: conjunto fechado de ações tipadas (instalar pacote, vincular arquivo). Sem campo de shell livre, sem `eval` | SPEC §10.1, §15 |
| T2 | Preset escreve fora do escopo (path traversal, symlink, `..`) | Resolver caminho real; aceitar só dentro de raízes declaradas (home do usuário, diretórios do preset); recusar symlink que escape | SPEC §10.1 |
| T3 | Segredo entra em preset, log ou histórico | Preset só **referencia** segredo; lista de caminhos e padrões proibidos; logs sem valores | SPEC §10.2 |
| T4 | Elevação indevida via `sudo`/`pacman` | Classe privilegiada (seção 4): o comando exato é mostrado e confirmado; nunca rodar a CLI inteira como root | SPEC §10.1, §10.3 |
| T5 | Histórico ou reversão adulterado | Histórico em área do usuário com permissão 0600; reversão valida o estado antes de restaurar e falha em vez de adivinhar. **Histórico implementado (fatia 3); formato v2 com `action` e `undoes` e `nodusctl history` numerando as linhas (fatia 4, PR 1; v1 segue legível; versão antiga do nodus não lê v2); **formato v3 com `manager` (`pacman` ou `flatpak`, ADR 0014): a leitura valida cada nome pela regra do gerenciador da entrada, `remove` só existe no pacman e `history undo` recusa entrada Flatpak; v1 e v2 seguem legíveis como pacman.** Reversão implementada (`nodusctl history undo <n>`, fatia 4 PR 2, ADR 0011):** só remove o que está na entrada, ainda instalado e explícito (`pacman -Q` e `-Qe`), depois de simular com `pacman -Rs --print`. **Não protege** contra linha plantada que peça a remoção de um pacote explícito qualquer (o histórico não é assinado): o que barra é o dono ler o plano antes de confirmar | SPEC §16.10 |
| T6 | Repositório Git hostil ao rodar `fetch` | Seção 6: sem hooks, sem config do repo que execute código | SPEC §6.6, §15 |
| T7 | Nome de pacote ou repositório injeta argumento (`-x`, `;`) | Executar processos com lista de argumentos, nunca por string de shell; validar nomes contra padrão estrito | SPEC §10.1 |
| T8 | Reaplicação duplica ou corrompe configuração | Idempotência com plano (desejado vs atual) antes de executar; teste de reaplicação | SPEC §16.4 |
| T9 | Binário do nodus adulterado ou rebaixado | Só código-fonte, tag Git assinada e `--locked`; sem binário nem `self-update` até um ADR novo (seção 7, ADR 0010) | SPEC §10.1 |

## 4. Modelo de permissões

Toda operação pertence a uma de três classes:

| Classe | Exemplos | Regra |
|---|---|---|
| Leitura | inventário, listar apps, estado dos repos, verificar atualizações | Sem confirmação; nunca escreve |
| Escrita de usuário | vincular dotfile, gravar preset ou histórico | Plano obrigatório; confirmação |
| Privilegiada | instalar ou remover pacote (pacman), mexer em serviço, firewall, rede, drivers | Plano obrigatório; confirmação explícita **por operação**; elevação só no passo, com o comando exato à vista |

Regras gerais:
- **Plano antes de executar** (SPEC §16.9): nada muda sem mostrar o que vai mudar.
- Remover app ou dado, desativar serviço, alterar firewall, rede ou drivers e rodar comandos administrativos são sempre privilegiados (SPEC §10.3).
- Sem modo "sim para tudo" na classe privilegiada.
- **GUI (ADR 0027; primeira ação implementada no PR #99, `preset apply`):** a GUI não executa escrita no próprio processo, privilegiada ou não. Um botão de ação só abre um terminal rodando o comando da CLI, que continua dona do plano, da confirmação e do `sudo`; a GUI não vê senha e nunca passa `--yes`/`--noconfirm`. Lista de ações fechada (`actions.rs`) e separada da de leitura (`Allowed`), lançada por `argv` sem shell; o único `sh -c` é um script **constante** que recebe o `nodusctl` e o arquivo como parâmetros posicionais (um caminho com aspas e `;` chega como dado, coberto por teste com o `sh` real). O terminal é só o medido (`konsole`), por `NODUS_TERMINAL` absoluto ou `PATH`; sem ele, a GUI mostra o comando para colar. Segunda ação (PR #100): `history undo <n>`, com `n` como único argumento vindo da tela, `u32` estrito (`n >= 1`; `preset_apply` recusa `n`); `sudo pacman -Rs` continua sendo da CLI, com a simulação e a confirmação no terminal. **Não verificado na janela real.**
- **Implementado (preset fatia 2, `nodusctl preset apply`):** um único comando, `sudo pacman -S --needed -- <faltantes>`, mostrado por inteiro antes de `Executar? [s/N]` (só `s`/`sim` confirmam; sem terminal interativo, recusa). Nunca `--noconfirm` (o `[Y/n]` do pacman mostra as dependências) nem `-y` (atualização parcial). Nomes validados na leitura do preset e `--` antes deles (T7). `sudo` e `pacman` herdam o terminal: o nodus não vê a senha, só o status. Depois da execução o plano é recalculado para conferir. **Flatpak (ADR 0015):** segundo passo, `flatpak install --user -- flathub <ids>`, com o próprio comando e a própria confirmação; nunca `-y`, `--noninteractive` nem `--system`, `--` antes dos IDs (T7). Antes de montar o comando, o plano simula `pacman -S --print --needed -- <faltantes>` (sem root, não instala nada) para separar os itens que o pacman não encontra (P-18): eles ficam fora do comando e do histórico e aparecem como "indisponíveis". Sem o Flatpak ou sem o remote `flathub` no `--user`, o apply para antes de qualquer pergunta; falha do pacman impede o passo Flatpak; uma entrada de histórico por passo executado.
- **Implementado (fatia 3, histórico):** cada `preset apply` que chega a executar grava uma linha JSON em `$XDG_STATE_HOME/nodus/history.jsonl` (ou `~/.local/state/nodus/`), só acrescentando; `nodusctl history` lista. Diretório 0700 e arquivo 0600; a gravação e a leitura recusam symlink, arquivo que não é comum e permissão mais larga que 0600; a leitura recusa linha inválida (com o número), campo desconhecido, versão diferente, nome de pacote fora da regra do pacman (a lista vai para a tela) e arquivo acima de 8 MiB. Guarda só o comando fixo, os nomes dos pacotes pedidos e o que `pacman -Q` mostrou depois; nada de ambiente, saída do pacman ou caminhos (T3). Grava também quando o `sudo`/`pacman` falha; não grava em recusa, plano vazio, só Flatpak nem sem terminal. O `apply` resolve o caminho antes de executar: sem lugar para gravar, nem começa. Se a gravação falhar depois de instalar, o erro diz que a instalação foi feita. A data é a do início do `apply` (antes da confirmação). **Config do usuário, listas de ignorados (ADR 0020, PR #61):** o mesmo `config.toml` guarda `ignore_pacman` e `ignore_flatpak`, lidos com a regra de nome do preset (nome inválido ou repetido recusa o arquivo inteiro). Toda escrita lê o config, muda o campo certo e regrava tudo de forma atômica; sem trava entre dois processos (uma atualização pode se perder). Um config plantado só esconde itens do `adopt` e do `suggest`. **Config do usuário (ADR 0018, PR #57):** `nodusctl workspace set` grava `~/.config/nodus/config.toml` (ou `$XDG_CONFIG_HOME/nodus/`), a única escrita do nodus em configuração. Mesmo cuidado do histórico: diretório 0700, arquivo 0600, recusa symlink, arquivo que não é comum e permissão mais larga; leitura com limite de 64 KiB e formato estrito; gravação atômica (temporário ao lado e `rename`). O config não é assinado: quem escreve na conta pode trocar a pasta, o que só muda qual pasta o `repos` lê (só refs, com o isolamento do ADR 0009). **Edição do preset (ADR 0019, PR #59):** `nodusctl preset adopt` é a única escrita do nodus num arquivo que o dono escreveu à mão. Só acrescenta nomes (validados como no `apply`: sem aspas, barra invertida nem `-` inicial) por inserção de texto, o resto do arquivo fica byte a byte; reparseia o resultado antes de gravar; grava no alvo real se o arquivo for um symlink, preserva as permissões, recusa se o conteúdo mudou desde a leitura e troca por temporário e `rename`. Exige terminal interativo e confirma item a item. Não instala nada. O histórico **não é assinado**: quem escreve na sua conta pode alterá-lo (reversão futura deve validar contra `pacman -Q`, não confiar nele).
- **Implementado (reversão, fatia 4 PR 2, ADR 0011):** `nodusctl history undo <n>` remove só o que a entrada marca como instalado e que continua instalado e explícito. Mostra o resultado de `pacman -Rs --print --print-format '%n %v'` (roda sem root; inclui o que sairia junto, medido: `abi-dumper` arrasta `ctags` e `vtable-dumper`) e, se ele falhar, para sem `sudo` (mostra as 5 primeiras linhas do stdout e quantas faltam: o pacman lista as dependências quebradas no stdout, 979 linhas com `glibc`). A saída da simulação é entrada externa: cada linha é validada (`nome versão`) e todo candidato tem de aparecer nela. Executa `sudo pacman -Rs -- <pacotes>` com `Executar? [s/N]` e o `[Y/n]` do pacman; **nunca** `-n` (apaga os `.pacsave`), `-c`, `-d`/`-dd` nem `--noconfirm`. Sem terminal, recusa. Grava uma entrada nova (v2) em vez de editar a antiga. Remoção é privilegiada: a confirmação é por operação.
- **Implementado no PR #66; verificado no Arch em 2026-11-16 (ensaio em HOME temporário e adoção real do `.bash_logout`) (dotfiles, ADR 0022):** `nodusctl dotfiles adopt <caminho>` é escrita de usuário em arquivo real, sem privilégio. Plano e `Executar? [s/N]` antes de qualquer passo; cópia para a pasta-fonte com `create_new` (nunca sobrescreve), conferência byte a byte e conferência de que o original não mudou, e só então `rename` atômico de um symlink por cima do original. Recusa origem symlink, pasta ou acima de 1 MiB, pai com symlink (T2), caminho da denylist de segredos (T3) e destino existente. **O conteúdo do arquivo é lido e copiado para uma pasta que vira repositório**: o plano avisa para não publicar a pasta; o nodus não varre o conteúdo à procura de segredo. Resíduo: um processo do mesmo usuário pode correr contra o nodus entre a conferência e o `rename`.
- Overclock, undervolt e scripts de otimização genéricos não existem no MVP (SPEC §15).

## 5. Segredos

- O preset referencia um segredo por nome; o valor nunca entra no arquivo.
- Caminhos proibidos para leitura e cópia (lista inicial, a ampliar): `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.config/gcloud`, `.env*`, arquivos de token de CLIs.
- Logs e histórico registram **o que foi pedido** (nomes de pacote e comando), não o conteúdo dos arquivos nem o efeito completo: dependências instaladas ou removidas junto não aparecem (ADR 0011, item 8, P-16). Não use o histórico como trilha de auditoria.
- Integração com gerenciador de segredos fica para depois do MVP (SPEC §10.2).

## 6. Git: somente leitura e `fetch`

Rodar `git` num repositório que não é seu não é automaticamente seguro: o Git pode executar código vindo de `.git/config` (por exemplo `core.fsmonitor`) e de hooks. Regras:
- Nunca `pull`, `reset`, `checkout` ou `merge` (SPEC §15).
- Executar `git` com configuração isolada, na lista de argumentos: `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_ALLOW_PROTOCOL=file:https:ssh`, `GIT_TERMINAL_PROMPT=0`, `-c core.fsmonitor=false -c core.hooksPath=/dev/null -c core.sshCommand=ssh -c core.gitProxy= -c credential.helper= -c protocol.ext.allow=never` e, no `fetch`, `--upload-pack=git-upload-pack`. Medido contra um repositório hostil no git 2.56.0 (ADR 0009).
- **Nunca `git status`** nem comando que olhe a árvore de trabalho: um filtro `clean` com nome escolhido pelo atacante o executa mesmo com o isolamento (medido). O nodus lê só refs (`for-each-ref`, `rev-list`, `rev-parse`).
- Tratar nomes de branch, remoto e tag como entrada não confiável (T7).
- **Não verificado:** `credential.helper` (não medido), `pre-auto-gc` (não consegui disparar o vetor), outros vetores e outras versões do Git. A receita vale para o que foi medido, não é garantia de segurança. Repetir `project/spikes/p09-git/medir.sh` ao trocar a versão do Git.

## 7. Supply chain do próprio nodus

Decidido no ADR 0010 (P-08): o primeiro canal é código-fonte (PKGBUILD no AUR ou `cargo install --locked --tag`); cada versão é uma tag Git assinada pelo dono, com `sha256sums` no PKGBUILD e `Cargo.lock` versionado. O nodus não tem `self-update`, então downgrade fica com o gerenciador de pacotes. **Binário distribuído, instalador ou agente que se atualiza estão proibidos sem ADR novo** (assinatura, chaves, rotação, revogação, downgrade). Ainda **não existe** chave de assinatura configurada nem tag (P-13), nem PKGBUILD (P-14). Dependências Rust passam pela triagem do workspace (ADR 0006) e pelo CI.

## 8. Fora deste modelo (lacunas conhecidas)

- **Core remoto e multi-device (Fase 2):** enrollment, autenticação mútua, rotação e revogação de credenciais, tarefas remotas com expiração (SPEC §8.3, §8.4).
- **Plugins (SPEC §11):** isolamento, assinatura, permissões declaradas.
- **Integração com IA (SPEC §7):** escopos, aprovação humana, prompt injection em logs e nomes.
- **Atualização do agente:** canal e assinatura.
- **Reporte de vulnerabilidade:** política em `.github/SECURITY.md` (private vulnerability reporting do GitHub, melhor esforço, sem SLA; P-07).
- **Revisão externa:** nenhuma. O modelo foi escrito por IA e revisado só pelo dono.
