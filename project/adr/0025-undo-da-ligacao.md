# 0025: `history undo` de ligação de dotfile: remover só o symlink, sem mudar o formato do histórico

**Data:** 2026-11-16

## Contexto
`adopt` (ADR 0022) e `link` (ADR 0024) criam symlink no HOME. Só o `adopt` tem volta (ADR 0023); o `history undo` numa entrada `link` recusa (item 8 do 0024). Isso quebra a regra do ADR 0011: toda escrita do nodus é reversível, simulada e confirmada. É remoção no HOME do usuário (AGENTS §4) e o histórico não é assinado (T5).

## Decisão
1. **`nodusctl history undo <n>` numa entrada `link`** remove o symlink `~/<caminho>` e **só ele**. Nunca toca no alvo (a pasta-fonte não muda) nem segue o link. O caminho deixa de existir no HOME.
2. **Só se ainda for o que o `link` criou:** o caminho precisa ser symlink e o alvo (`read_link`) **idêntico em texto** a `<dir>/<caminho>`, com `dir` e `path` vindos da entrada. Qualquer outra coisa (arquivo comum, pasta, ausente, symlink para outro lugar, alvo com barra final ou relativo) **recusa e explica**, sem alterar nada, antes da pergunta. Comparação de texto, não de `Path` (que ignora a barra final).
3. **Revalidação (T5):** `path` com `validate_path`, `dir` com `validate_dir`, pais sem symlink (`check_parents`). Uma linha plantada no histórico não vira remoção fora da regra.
4. **Sequência:** plano ("remover o symlink X -> Y; o arquivo na pasta-fonte não é alterado") e `Executar? [s/N]` (só `s`/`sim`; sem terminal, recusa); reconferir o `read_link`; `remove_file`; gravar o histórico.
5. **Pastas-pai criadas pelo `link` ficam** (escolha do dono em 2026-11-16). O histórico não registra quais foram criadas, e remover pasta "vazia" poderia apagar uma que o usuário já tinha. Uma pasta vazia pode sobrar.
6. **Sem mudança de formato do histórico.** A linha é a mesma v4 do undo da adoção: `action = "remove"`, `undoes = <n>`, `manager = "dotfiles"`, `dir`, um caminho em `requested` e em `still_missing`. O `parse_line` já a valida. Quem distingue as duas reversões é a leitura: o `history` consulta a entrada apontada por `undoes` e mostra "ligação desfeita" ou "dotfile restaurado".
7. **Seleção:** `select_adoption` vira `select_dotfile_entry`, que aceita `adopt` e `link` (devolvendo qual) e recusa o resto. A recusa de "já desfeita" continua por `undoes == n`, para os dois.
8. Sem dependência nova.

## Alternativas
- **`Action::Unlink` nova:** descartada; muda o formato (leitor antigo recusa a linha) sem ganho, porque `undoes` já liga a reversão à entrada original.
- **Remover as pastas-pai vazias:** descartada pelo dono; pode apagar o que o usuário já tinha e é uma escrita destrutiva a mais.
- **Perguntar se remove quando o alvo divergiu:** descartada; o nodus só desfaz o que criou.
- **Apagar também o arquivo na pasta-fonte:** descartada; é de um repo que o nodus não gerencia.

## Consequências
- Fecha a reversão do critério 11: toda escrita de dotfile do nodus tem volta.
- Uma pasta-pai vazia pode sobrar depois do undo.
- Duas formas de reversão compartilham o mesmo registro; ler o histórico exige olhar a entrada apontada.
- Reabrir se: o `link` em lote ou um histórico de efeito (pastas criadas) pedirem outro registro.

## Lacunas
- Resíduo de corrida: o symlink pode ser trocado entre a reconferência e o `remove_file`; as checagens estreitam a janela, não a fecham.
- **Implementado no PR #73** (testes primeiro, CI vermelho por item inexistente; a implementação passou de primeira; mutação de propósito: aceitar qualquer symlink, ignorar "já desfeita", `remove_file` no alvo em vez do symlink e inverter o render derrubaram **11 testes**). **A reconferência do `read_link` dentro do `remove_link` não tem teste** (cobre corrida; exigiria gancho no código de produção). `UndoLinkError` duplica `NotALink` e `WrongTarget` do `UndoAdoptError`, de propósito, para não mexer no undo da adoção já em produção.
- Verificado no Arch real em 2026-11-17, em HOMEs temporários (`/tmp/ub`, pasta-fonte `/tmp/udot`; a 1ª rodada foi com o `!`, sem terminal, e não provou nada): dois `link` (um aninhado com pastas criadas), `undo 1` e `undo 2` com plano e `s` (symlinks removidos, **pastas `.config` e `.config/git` ficaram com 0700**, fontes intactas), segundo `undo 1` recusado, `history` com "ligação desfeita" nas linhas 3 e 4, e `undo` de um link cujo caminho virou arquivo comum recusado sem tocar nele. **Achado na verificação:** a mensagem de "já desfeita" dizia "a adoção da entrada 1" para uma ligação; corrigida para "a entrada N já foi desfeita" (com teste). **Não verificado:** num dotfile real do HOME do dono; permissão negada na remoção; `git config --global`, editores e KDE.
- Este ADR foi escrito por IA e revisado só pelo dono.
