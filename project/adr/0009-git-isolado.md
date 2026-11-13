# 0009: Git isolado: `git` CLI com isolamento, só refs (resultado do spike P-09)

**Data:** 2026-11-13

## Contexto
O critério 7 do MVP roda `git` em repositórios que o usuário não controla. O `SECURITY.md` §6 (T6) avisa que o Git executa código vindo de `.git/config` e de hooks, e o ADR 0007 deixou `gix` vs `git` CLI em aberto até um spike provar qual não executa código do repositório. Este spike mediu o `git` CLI (git 2.56.0, Arch) com canários: cada vetor, se executado, cria um arquivo; cada um roda sem isolamento (**baseline**, tem de disparar, senão o vetor não prova nada) e com o isolamento candidato (**iso**, não pode disparar). A medição é repetível: `project/spikes/p09-git/medir.sh` gera tudo num diretório temporário, sem rede e sem tocar no HOME do usuário.

## O que foi medido

| Vetor | Comando | Sem isolamento | Isolado |
|---|---|---|---|
| `core.fsmonitor` com programa | `status` | executa | bloqueado |
| hook `reference-transaction` | `fetch` | executa | bloqueado |
| `core.hooksPath` para pasta do atacante | `fetch` | executa | bloqueado |
| `remote.origin.uploadpack` | `fetch` | executa | bloqueado |
| `core.sshCommand` | `fetch` | executa | bloqueado |
| `core.gitProxy` (executado como programa, sem shell) | `fetch` | executa | bloqueado |
| transporte `ext::` liberado pela config | `fetch` | executa | bloqueado |
| `include.path` trazendo `fsmonitor` de fora do repo | `status` | executa | bloqueado |
| **filtro `filter.<nome>.clean`** (nome escolhido pelo atacante) | `status` | executa | **continua executando** |
| mesmo filtro, anulando todo `filter.*.{clean,smudge,process}` achado na config | `status` | executa | bloqueado |
| mesmo filtro, sem `status` (só `for-each-ref`) | `for-each-ref` | não executa | não executa |
| repositório de outro dono | `status` | recusa (`dubious ownership`) | recusa (**simulado** com `GIT_TEST_ASSUME_DIFFERENT_OWNER`) |
| hook `pre-auto-gc` | `fetch` | **não consegui disparar** | inconclusivo |
| `credential.helper` | `fetch` | **não medido** | não medido |

**Prova de que a medição testa algo:** a primeira rodada deu 9 de 11 baselines "limpo" e foi tratada como inválida (dois defeitos do próprio script, corrigidos). Na versão final, 10 vetores disparam no baseline. Quebra de propósito: tirar `core.hooksPath=/dev/null` do isolamento fez `hook_reftx` e `hookspath` dispararem também isolados.

## Decisão
1. **`git` CLI como processo externo, com isolamento**, na lista de argumentos (nunca shell, T7): `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_ALLOW_PROTOCOL=file:https:ssh`, `GIT_TERMINAL_PROMPT=0`, e `-c core.fsmonitor=false -c core.hooksPath=/dev/null -c core.sshCommand=ssh -c core.gitProxy= -c credential.helper= -c protocol.ext.allow=never`. No `fetch`, `--upload-pack=git-upload-pack`.
2. **O nodus lê só refs no MVP** (branch, ahead/behind, último commit) com `for-each-ref` e `rev-list`/`rev-parse`. **Nada de `git status` nem de comando que olhe a árvore de trabalho**: o filtro `clean` tem nome escolhido pelo atacante, não há chave fixa para anular, e o `status` o executa mesmo isolado (medido). "Sujo/limpo" fica fora do MVP.
3. **`gix` não entra.** O CLI isolado cobre os vetores medidos; `gix` só voltaria se uma lacuna abaixo exigir. `git2` segue descartado (ADR 0007).

## Alternativas descartadas
- **`gix`:** não foi medido (exigiria compilar só para medir; build roda só no CI, ADR 0009 do workspace). Sem necessidade comprovada, é dependência sem uso.
- **Anular `filter.*` dinamicamente e manter `git status`:** medido bloqueado, mas depende de enumerar chaves de comando; uma chave esquecida ou nova numa versão futura do Git volta a executar código. Descartado por fragilidade, não por falha.
- **Confiar só em `-c`:** `-c` anula só as chaves listadas, a config do repo continua lida.

## Consequências
- O critério 7 do MVP entrega estado de **refs**, não de árvore de trabalho. Reabrir "sujo/limpo" pede spike próprio.
- A lista de opções vale **para o git 2.56.0** e para os vetores listados: não é "seguro", é "isolado contra o que foi medido". Vetores novos exigem repetir `medir.sh`.
- O isolamento do `fetch` usa o remoto `origin` na medição; no produto os remotos têm nomes arbitrários (T7), então `--upload-pack` na linha de comando vale para qualquer remoto, mas `remote.<nome>.uploadpack` precisa de teste com outros nomes.

## Não verificado
- **`credential.helper`:** não medido (exige um servidor HTTP de teste que peça credencial). A única proteção é `-c credential.helper=`, não provada.
- **`pre-auto-gc`:** o `fetch` chamou `git maintenance run --auto`, mas o `gc` e o hook não subiram nesta versão, nem com objetos soltos plantados. A proteção esperada vem de `core.hooksPath=/dev/null`, que **foi** provada com outros hooks.
- **Dono diferente:** simulado com `GIT_TEST_ASSUME_DIFFERENT_OWNER=1` (variável de teste do Git); não há como criar outro dono sem root.
- Outros vetores não listados (por exemplo `diff.external`, `textconv`, atributos de merge, `submodule`), outras versões do Git e outros SOs.
