# 0024: `dotfiles link`: ligar um arquivo da pasta-fonte ao HOME, sem nunca sobrescrever

**Data:** 2026-11-16

## Contexto
O `adopt` (ADR 0022) leva um arquivo real do HOME para a pasta-fonte e deixa um symlink; o `undo` (ADR 0023) desfaz. Falta a **máquina nova**: a pasta-fonte (repo clonado) já tem os arquivos e o HOME não. Sem o `link`, o `adopt` só serve de backup. É escrita no HOME (AGENTS §4) e o histórico não é assinado (T5).

## Decisão (escolhas do dono em 2026-11-16)
1. **`nodusctl dotfiles link <caminho>`**, um caminho por vez (caminho relativo ao HOME, `validate_path` do 0021, inclusive a denylist). Ligar tudo (`--all`) fica para fatia futura, como laço sobre este comando. *(Emenda, ADR 0030: o `--all` existe, sobre a lista curada, com um plano e uma confirmação.)*
2. **Fonte:** `<dotfiles_dir>/<caminho>` precisa ser arquivo comum de até 1 MiB (não pasta, não symlink). `dotfiles_dir` como no 0022 (`validate_dir`, `canonicalize`).
3. **Destino existente no HOME: sempre recusa**, seja arquivo comum, pasta, outro tipo ou symlink para outro lugar. Nada de `.bak`, nada de mover. Se o destino já é um symlink **exatamente** para `<dir>/<caminho>` (comparação de texto, não de `Path`), recusa com "já está ligado".
4. **Pasta-pai ausente no HOME: cria**, uma a uma, modo 0700, sem seguir symlink (o `ensure_dirs` do 0022). Pasta-pai que é symlink ou não é pasta: recusa (T2). Idem para as pastas do caminho dentro da pasta-fonte.
5. **Sequência**, cada passo só depois do anterior: validar e observar (só leitura); plano e `Executar? [s/N]` (só `s`/`sim`; sem terminal, recusa); criar as pastas-pai; reconferir a fonte; **`symlink(2)` direto** em `~/<caminho>`, apontando para o caminho real da pasta-fonte. Sem temporário nem `rename`: o `rename` sobrescreveria um destino que apareceu no meio; o `symlink` falha com `EEXIST`, então a própria syscall garante "nunca sobrescreve".
6. **Aviso no plano:** ligar um arquivo de shell (`.bashrc`, `.profile`) de uma pasta-fonte faz o shell executar o conteúdo dele no próximo login. O nodus não lê o conteúdo; só ligue de pasta em que você confia.
7. **Histórico v4:** `action = "link"`, `manager = "dotfiles"`, `dir`; mesma forma do `adopt` (um caminho, `installed` igual a `requested`, sem `still_missing`, resultado ok). A leitura continua estrita; nodus antigo recusa a linha (`UnsupportedVersion`).
8. **`history undo` numa entrada `link` recusa** (`NotAnAdoption`). Remover o symlink é a fatia seguinte. *(Emenda, ADR 0025: o `undo` da ligação existe e remove só o symlink; a recusa passou a ser `NotADotfileEntry`, só para o que não é adoção nem ligação.)*
9. Sem dependência nova.

## Alternativas
- **`link --all`:** descartada por ora; mais superfície (percorrer a pasta, N confirmações ou uma só) e plano maior.
- **Mover o destino para `.bak` e ligar:** descartada; deixa lixo no HOME e é uma escrita destrutiva a mais (o 0022 já descartou `.bak`).
- **Recusar pasta-pai ausente:** descartada; em máquina nova `.config/git/config` não seria ligável sem criar as pastas à mão.
- **Temporário + `rename` (como no `adopt`):** descartada; `rename` sobrescreve em silêncio.
- **Reusar `Action::Adopt`:** descartada; o `undo` trataria a ligação como adoção e restauraria cópia sobre um arquivo que nunca foi adotado.

## Consequências
- Habilita preparar um HOME novo a partir da pasta-fonte, com plano e confirmação, sem perder dado.
- Um repo de terceiros na pasta-fonte vira vetor de execução de código ao ligar arquivos de shell (item 6); a defesa é o dono.
- `Action::Link` novo muda o formato do histórico (leitor antigo recusa).
- Pastas-pai criadas ficam se a ligação falhar depois (não perdem dado).
- Reabrir se: `link --all` ou o `undo` da ligação pedirem outro registro.

## Lacunas
- Resíduo de corrida: destino criado entre o plano e o `symlink` é pego pelo `EEXIST`; fonte trocada entre a reconferência e o `symlink` não.
- **Implementado no PR #70** (testes primeiro, CI vermelho por item inexistente; mutação de propósito: aceitar destino arquivo comum e comparar o alvo por `Path` derrubaram 3 testes). A terceira quebra (tirar o braço "pai é symlink" de `missing_home_dirs`) **não derrubou teste e não é lacuna**: o braço seguinte (`is_dir` sobre `symlink_metadata`, que não segue symlink) recusa do mesmo jeito; o braço existe pela mensagem. A reconferência da fonte antes do `symlink` não tem teste (cobre corrida; exigiria gancho no código de produção).
- Verificado no Arch real em 2026-11-16, em HOMEs temporários (`/tmp/ha` adotando, `/tmp/hb` limpo ligando, pasta-fonte `/tmp/dot`): `link .bashrc` e `link .config/git/config` (plano com "criar pastas: .config, .config/git", pastas 0700, symlinks certos, conteúdo lido pelo link), segundo `link .bashrc` recusado ("já está ligado"), `link .profile` com destino existente recusado (arquivo intacto), `history` com duas linhas "dotfile ligado", `history undo 1` recusado. **Não verificado:** num dotfile real do HOME do dono; permissão negada na criação das pastas; `git config --global`, editores e KDE com o symlink.
- Este ADR foi escrito por IA e revisado só pelo dono.
