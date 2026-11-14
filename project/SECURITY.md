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
| T5 | Histórico ou reversão adulterado | Histórico em área do usuário com permissão 0600; reversão valida o estado antes de restaurar e falha em vez de adivinhar | SPEC §16.10 |
| T6 | Repositório Git hostil ao rodar `fetch` | Seção 6: sem hooks, sem config do repo que execute código | SPEC §6.6, §15 |
| T7 | Nome de pacote ou repositório injeta argumento (`-x`, `;`) | Executar processos com lista de argumentos, nunca por string de shell; validar nomes contra padrão estrito | SPEC §10.1 |
| T8 | Reaplicação duplica ou corrompe configuração | Idempotência com plano (desejado vs atual) antes de executar; teste de reaplicação | SPEC §16.4 |
| T9 | Binário do nodus adulterado ou rebaixado | Assinatura e proteção contra downgrade: **lacuna** (seção 7) | SPEC §10.1 |

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
- **Implementado (preset fatia 2, `nodus preset apply`):** um único comando, `sudo pacman -S --needed -- <faltantes>`, mostrado por inteiro antes de `Executar? [s/N]` (só `s`/`sim` confirmam; sem terminal interativo, recusa). Nunca `--noconfirm` (o `[Y/n]` do pacman mostra as dependências) nem `-y` (atualização parcial). Nomes validados na leitura do preset e `--` antes deles (T7). `sudo` e `pacman` herdam o terminal: o nodus não vê a senha, só o status. Depois da execução o plano é recalculado para conferir. Flatpak ainda não é instalado.
- Overclock, undervolt e scripts de otimização genéricos não existem no MVP (SPEC §15).

## 5. Segredos

- O preset referencia um segredo por nome; o valor nunca entra no arquivo.
- Caminhos proibidos para leitura e cópia (lista inicial, a ampliar): `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.config/gcloud`, `.env*`, arquivos de token de CLIs.
- Logs e histórico registram **o que mudou**, não o conteúdo dos arquivos.
- Integração com gerenciador de segredos fica para depois do MVP (SPEC §10.2).

## 6. Git: somente leitura e `fetch`

Rodar `git` num repositório que não é seu não é automaticamente seguro: o Git pode executar código vindo de `.git/config` (por exemplo `core.fsmonitor`) e de hooks. Regras:
- Nunca `pull`, `reset`, `checkout` ou `merge` (SPEC §15).
- Executar `git` com configuração isolada, na lista de argumentos: `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_ALLOW_PROTOCOL=file:https:ssh`, `GIT_TERMINAL_PROMPT=0`, `-c core.fsmonitor=false -c core.hooksPath=/dev/null -c core.sshCommand=ssh -c core.gitProxy= -c credential.helper= -c protocol.ext.allow=never` e, no `fetch`, `--upload-pack=git-upload-pack`. Medido contra um repositório hostil no git 2.56.0 (ADR 0009).
- **Nunca `git status`** nem comando que olhe a árvore de trabalho: um filtro `clean` com nome escolhido pelo atacante o executa mesmo com o isolamento (medido). O nodus lê só refs (`for-each-ref`, `rev-list`, `rev-parse`).
- Tratar nomes de branch, remoto e tag como entrada não confiável (T7).
- **Não verificado:** `credential.helper` (não medido), `pre-auto-gc` (não consegui disparar o vetor), outros vetores e outras versões do Git. A receita vale para o que foi medido, não é garantia de segurança. Repetir `project/spikes/p09-git/medir.sh` ao trocar a versão do Git.

## 7. Supply chain do próprio nodus

Lacuna conhecida da Fase 0: assinatura dos binários, verificação de integridade e proteção contra downgrade (SPEC §10.1) ainda não têm desenho. Devem ser decididas antes da primeira versão distribuída. Dependências Rust passam pela triagem do workspace (ADR 0006) e pelo CI.

## 8. Fora deste modelo (lacunas conhecidas)

- **Core remoto e multi-device (Fase 2):** enrollment, autenticação mútua, rotação e revogação de credenciais, tarefas remotas com expiração (SPEC §8.3, §8.4).
- **Plugins (SPEC §11):** isolamento, assinatura, permissões declaradas.
- **Integração com IA (SPEC §7):** escopos, aprovação humana, prompt injection em logs e nomes.
- **Atualização do agente:** canal e assinatura.
- **Política pública de reporte de vulnerabilidade** (repo público): ainda não existe.
- **Revisão externa:** nenhuma. O modelo foi escrito por IA e revisado só pelo dono.
