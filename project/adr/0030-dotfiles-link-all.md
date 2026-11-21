# 0030: `dotfiles link --all`: ligar em lote a lista curada, um plano, uma confirmação

**Data:** 2026-11-21

## Contexto
O `dotfiles link <caminho>` (ADR 0024) liga um arquivo por vez e deixou o `--all` para "fatia futura, como laço sobre este comando". Em máquina nova, com a pasta-fonte clonada, o caso real é ligar vários de uma vez. É escrita no HOME (AGENTS §4).

## Decisão (escolhas do dono em 2026-11-21, plan mode)
1. **`nodusctl dotfiles link --all`**, exclusivo com o caminho (o clap exige um dos dois). Sem GUI nesta fatia (ADR 0028).
2. **Candidatos = a lista curada** do `nodusctl dotfiles` (`effective_list`: padrão mais `dotfiles_extra` menos `dotfiles_skip`), só o que existe na pasta-fonte. O lote nunca percorre a pasta e nunca liga nada fora da lista; a denylist de segredos vale como no `link`.
3. **Um plano e um `Executar? [s/N]`.** O plano lista o que liga (caminho, tamanho, destino e alvo, pastas a criar sem repetir) e o que pula, com o motivo. O aviso de arquivo de shell aparece uma vez. Sem terminal, recusa depois do plano; sem nada a ligar, diz "Nada a ligar." e não pergunta.
4. **O que não dá para ligar é pulado, não aborta o lote:** destino já existe (inclusive o `.bashrc` de fábrica de uma máquina nova), já ligado, fonte ausente, grande demais ou que não é arquivo, caminho inválido ou da denylist. O nodus continua sem mover nem sobrescrever o que existe (ADR 0024, item 3).
5. **Para no primeiro erro de execução.** Os itens já ligados ficam, e a mensagem diz onde o lote parou e o que foi ligado antes. Não é atômico, de propósito: criar symlinks em vários lugares não tem transação.
6. **Uma linha v4 `link` por arquivo**, igual ao `link` de hoje: o `history undo <n>` (ADR 0025) desfaz um a um, sem formato novo.
7. Sem dependência nova. O código do `link` foi dividido em `prepare` (só leitura) e `execute` (pastas, reconferência da fonte, `symlink(2)`, histórico), usados pelo `link` e pelo `link_all`; os testes do `link` não mudaram.

## Alternativas
- **Percorrer a pasta-fonte:** descartada; puxa `.git`, README e segredos e exige um filtro novo.
- **`link a b c` explícito:** descartada; pouco útil numa máquina nova.
- **Um `[s/N]` por arquivo:** descartada; com 9 itens vira reflexo.
- **Continuar apesar de erro:** descartada; deixa o HOME num estado misturado se algo estiver errado.

## Consequências
- Prepara um HOME novo com uma confirmação, mantendo "nunca sobrescreve".
- Numa máquina nova, os arquivos de fábrica (`.bashrc`, `.profile`) aparecem como pulados e o dono os resolve à mão.
- Reabrir se: o lote precisar de um botão na GUI (ADR 0028) ou de `undo` em lote.

## Lacunas
- **Implementado no PR #116** (testes primeiro, CI vermelho por item inexistente; depois 378 + 305 + 2 testes verdes). Quebras propositais no CI, em branches descartáveis: (A) o lote continuar após erro derrubou 1 teste; (B) aceitar destino existente derrubou 4; (C) tirar `validate_path` do `plan_link` e do `prepare` derrubou 4. A denylist é conferida em dois lugares (`prepare` e `plan_link`), então tirar só um não derruba teste e não é lacuna.
- **Não verificado:** no Arch real, em HOME temporário e num HOME real (verificação do dono); permissão negada na criação das pastas; o texto dos "pulados" lido por uma pessoa.
- Este ADR foi escrito por IA e revisado só pelo dono.
