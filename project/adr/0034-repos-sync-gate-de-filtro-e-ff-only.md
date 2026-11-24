# 0034: `repos sync` só com gate de filtro e `merge --ff-only` isolado (resultado do spike P-20)

**Data:** 2026-11-24
**Estado:** decidido (spike medido); **`repos sync` ainda não implementado** (P-20). Continua o ADR 0009, que proibia olhar a árvore de trabalho e mandava abrir "sujo/limpo" com spike próprio.

## Contexto
O SPEC §6.6 pede "sincronização segura": inspecionar, classificar, planejar e executar só o aprovado, sem `pull` cego, sem sobrescrever alteração local, sem reset, sem trocar de branch e sem resolver conflito sozinho. Para isso o nodus precisa saber se a árvore está limpa (`status`) e avançar com `merge --ff-only`, que faz checkout. O ADR 0009 mediu que o `status` executa o filtro `filter.<nome>.clean` (nome escolhido pelo atacante) mesmo isolado, e descartou "anular `filter.*`" por fragilidade (uma chave esquecida ou nova numa versão futura volta a executar código). O `merge --ff-only` aplica `smudge`/`process` no checkout e dispara hooks.

## O que foi medido
`project/spikes/p10-sync/medir.sh` (bash + `git` 2.56.1, tudo em `mktemp -d`, HOME vazio, sem rede, canário por vetor; baseline sem proteção tem de disparar, o isolado não). A primeira rodada deu 12 de 13 baselines disparando (o `gpg_verify_merge` não: o Git só chama o `gpg` se o commit tiver cabeçalho `gpgsig`) e foi corrigida; as duas rodadas seguintes, com o script final, deram o mesmo resultado.

| Vetor | Comando | Sem proteção | Com gate + isolamento |
|---|---|---|---|
| `filter.*.clean` em `.git/config` | `status` | executa | recusado pelo gate |
| `filter.*.clean` vindo de `include.path` | `status` | executa | recusado |
| `filter.*.clean` em `config.worktree` (`extensions.worktreeConfig`) | `status` | executa | recusado |
| `[FILTER "x"] CLEAN` em maiúsculas, escrito à mão | `status` | executa | recusado |
| `filter.*.smudge` | `merge --ff-only` | executa | recusado |
| `filter.*.process` | `merge --ff-only` | executa | recusado |
| `filter.*.smudge` vindo de `includeIf "gitdir:"` | `merge --ff-only` | executa | recusado |
| `core.fsmonitor` | `status` e `merge` | executa | bloqueado pelo isolamento |
| hook `post-merge` | `merge` | executa | bloqueado |
| hook `reference-transaction` | `merge` | executa | bloqueado |
| `core.hooksPath` do atacante | `merge` | executa | bloqueado |
| `merge.verifySignatures` + `gpg.program` (commit com `gpgsig` falso) | `merge` | executa | bloqueado |

**Controles positivos (10, todos ok):** repositório sem filtro passa no gate; `status` limpo dá saída vazia; árvore suja dá ` M a.txt`; arquivo novo dá `?? untracked.txt`; `merge --ff-only` avança o HEAD até `origin/main`, sem commit de merge e com o conteúdo do remoto; `merge.ff=false` e `pull.rebase=true` na config não vencem o `--ff-only`; histórico divergido faz o Git recusar e o HEAD não muda; arquivo sujo que o ff sobrescreveria faz o Git recusar e preservar a alteração.

**Quebra de propósito:** com `SEM_GATE=1` os 7 vetores de filtro disparam também sob o isolamento do ADR 0009 (código de saída 1). É o gate, não o isolamento, que os barra.

## Decisão
1. **Detectar e recusar, não anular.** Antes de qualquer `status` ou `merge` num repositório, o nodus lê toda a config que o Git leria, com o isolamento do ADR 0009: `git config --list --includes --name-only`, que não executa nada. Se aparecer qualquer chave que case `^filter\..+\.(clean|smudge|process)$` (sem diferenciar maiúsculas), o repositório é **recusado como "não verificável"** e fica fora do `sync`. A falha é fechada: se a config nem puder ser lida, também recusa.
2. **Receita do `merge`:** o isolamento do ADR 0009 (`ISOLATION_ENV` e `ISOLATION_CONFIG` de `crates/nodus-core/src/repos.rs`) mais `-c submodule.recurse=false -c merge.verifySignatures=false` e, nos argumentos, `merge --ff-only --no-edit --no-verify-signatures @{u}`. Só `--ff-only`: nada de `pull`, `reset`, `rebase`, troca de branch ou resolução de conflito.
3. **O Git já protege o conteúdo local:** árvore suja no arquivo que o avanço mexeria, ou histórico divergido, faz o `--ff-only` recusar sem mexer (controles medidos). O nodus usa o `status` só para **mostrar** o estado no plano e para não tentar o que sabe que vai recusar.
4. **Escopo do futuro `repos sync`:** plano por repositório (limpo ou sujo, só atrás, adiantado, divergido, sem upstream, recusado), confirmação `[s/N]` e só então o `merge --ff-only` dos aprovados. Um item de histórico por repositório. Interface e fatiamento ficam para o plano da implementação (CLI primeiro; GUI só "no terminal", ADR 0027).

## Alternativas descartadas
- **Anular `filter.*` e manter `git status`** (alternativa A do ADR 0009): continua frágil; o gate cobre o conjunto inteiro da config, inclusive chaves futuras do mesmo padrão.
- **`gix`/`git2`:** sem necessidade comprovada (ADR 0007 e 0009).
- **Não ter `sync`:** o SPEC §6.6 o pede, e o spike mostrou que dá para fazer com o git CLI isolado.

## Consequências
- Um repositório com filtro `clean`/`smudge`/`process` na config do próprio repositório (por exemplo Git LFS instalado com `git lfs install --local`) **não entra no `sync`**: é o preço de não executar nada do repositório. O `.gitconfig` global não é lido (`GIT_CONFIG_GLOBAL=/dev/null`), então um LFS só global não recusa o repositório (o filtro então não é executado pelo nodus, e o checkout de arquivos LFS fica como ponteiros). O plano mostra o motivo da recusa.
- A conclusão vale para o git 2.56.1 e para os vetores medidos: é "isolado contra o que foi medido", não "seguro". Vetor novo, ou versão nova do Git, pede repetir `medir.sh`.
- `medir.sh` guarda uma cópia em bash da receita; conferir as duas ao mudar uma (como no ADR 0009).

## Não verificado
- **Submódulos no `merge`** (`submodule.recurse` está anulado, mas não medi um submódulo com `update` hostil) e `fetch` com submódulos já tinha `--no-recurse-submodules` (ADR 0009).
- **`credential.helper`** (sem servidor HTTP de teste) e `pre-auto-gc` continuam como no ADR 0009.
- **`core.editor`**: o `--ff-only` nunca abre editor, então não foi medido.
- `.git` como arquivo (`gitdir:` apontando para fora), `commondir` e worktrees ligados; `diff.external`/`textconv` (o `status` e o `ff` não os usam, mas não medi).
- Outras versões do Git, outros SOs, repositório de outro dono (só simulado no ADR 0009).
- Este ADR foi escrito por IA e revisado só pelo dono.
