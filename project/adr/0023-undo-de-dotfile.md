# 0023: `history undo` de dotfile: copiar de volta, manter a fonte, recusar se divergir

**Data:** 2026-11-16

## Contexto
O ADR 0022 entregou `dotfiles adopt`: o arquivo real do HOME vira symlink para a pasta-fonte (`dotfiles_dir`). O `history undo` recusa a entrada v4 (item 8 do 0022), então hoje o dono só desfaz à mão. O ADR 0011 fixou que toda ação do nodus tem reversão simulada e confirmada. Esta fatia fecha a reversão do `adopt`; o `dotfiles link` (máquina nova) fica para a fatia 4, com ADR próprio.

É escrita no HOME do usuário (AGENTS §4). O histórico é texto local sem assinatura (T5 do 0011): `path` e `dir` lidos dele são entrada não confiável.

## Decisão (escolhas do dono em 2026-11-16)
1. **`nodusctl history undo <n>` numa entrada de adoção** troca o symlink do HOME por uma **cópia do conteúdo atual** da pasta-fonte. Não restaura "o conteúdo original": o que vale é o que está na fonte agora, que pode ter sido editado depois da adoção.
2. **A cópia na pasta-fonte é mantida.** O nodus não apaga nada do repo de dotfiles (pode ter edições ou commit); o dono remove à mão se quiser.
3. **Se o estado divergiu, recusa e explica; nada é alterado.** Diverge quando: o caminho no HOME não é symlink (um editor o trocou por arquivo comum, ou sumiu); o symlink não aponta para `<dir>/<path>` registrado; a fonte não existe, não é arquivo comum ou passa de 1 MiB.
4. **Revalidação na hora:** `path` com `validate_path` e `dir` com `validate_dir` (do 0021 e 0022); pai do `path` sem symlink (`check_parents`). Uma linha plantada no histórico não vira escrita fora da regra.
5. **Sequência**, cada passo só depois do anterior:
   1. plano mostrado (caminho, fonte, tamanho, permissão) e `Executar? [s/N]` (só `s`/`sim`; sem terminal, recusa);
   2. cópia da fonte para um temporário `.<nome>.nodus-<pid>` no diretório do symlink, com `create_new`, 9 bits de permissão (sem setuid/setgid/sticky) e `fsync`;
   3. conferência byte a byte contra a fonte; reconferência de que o symlink não mudou (`dev`, `ino`, alvo);
   4. `rename` do temporário por cima do symlink (atômico: quem lê vê o link ou o arquivo, nunca nada).
   Falha antes do 4 remove só o temporário e deixa o symlink intacto.
6. **Registro:** entrada **nova** v4 (append-only, como o undo do pacman): `action = "remove"`, `undoes = <n>`, `manager = "dotfiles"`, `dir`. O `parse_line` passa a aceitar essa combinação; as demais combinações v4 continuam recusadas. A adoção é desfeita uma vez só: se já existe undo para a entrada `n`, recusa.
7. **`main.rs`** desvia a entrada de dotfile **antes** de consultar `pacman -Q`/`-Qe`.
8. Sem dependência nova.

## Alternativas
- **Copiar de volta e apagar da fonte:** descartada; apaga arquivo de um repo que o nodus não gerencia.
- **Só remover o symlink:** descartada; deixa o arquivo ausente do HOME, não é reversão.
- **Perguntar se sobrescreve quando diverge:** descartada; arrisca perder a edição feita no HOME. Quem quiser resolve à mão.
- **`Action::Undo` novo ou v5:** descartada; `remove` + `undoes` já existe no v3 e liga a reversão à adoção com a mesma semântica.
- **Restaurar o conteúdo do momento da adoção:** descartada; o nodus não guarda esse conteúdo além da própria fonte.

## Consequências
- Habilita desfazer a adoção com plano e confirmação, sem janela sem arquivo.
- O formato do histórico muda: um nodus antigo recusa a linha nova (`UnsupportedVersion`), como no 0014 e no 0022.
- A pasta-fonte acumula arquivos que o nodus não limpa.
- O undo não reverte edições feitas na fonte: o HOME fica com o conteúdo atual dela.
- Reabrir se: o `link` pedir outro registro, ou se desfazer "para o conteúdo da adoção" virar necessidade.

## Lacunas
- Resíduo de corrida (igual ao 0022): um processo do mesmo usuário pode trocar o symlink entre a conferência e o `rename`; as checagens estreitam a janela, não a fecham.
- **Implementado no PR #68** (testes primeiro, CI vermelho por item inexistente; mutação de propósito: 3 quebras derrubaram 5 testes). **A reconferência do symlink logo antes do `rename` não tem teste** (cobre corrida; testar exigiria um gancho no código de produção): quebrá-la não derrubou nada.
- Verificado no Arch real em 2026-11-16, em HOME temporário (`HOME=/tmp/hx`): adotar `.bashrc`, `history undo 1` (plano, `s`), arquivo comum de 10 bytes com `diff` igual à fonte, `history` listando a adoção e `desfez [1]`, segundo `undo 1` recusado ("já foi desfeita"). **Não verificado:** num dotfile real do HOME do dono; recusas no binário real (symlink trocado, fonte apagada, sem terminal); comportamento de `git config --global`, editores e KDE com a cópia de volta.
- Ao desfazer, só o dono sabe se a fonte tem segredo que agora voltou ao HOME (o nodus não lê para classificar, 0021).
- Este ADR foi escrito por IA e revisado só pelo dono.
