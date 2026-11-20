# 0029: Reversão de Flatpak no `history undo`: `flatpak uninstall --user`, sem simulação e sem prompt do Flatpak

**Data:** 2026-11-20

## Contexto
O `history undo <n>` só desfazia instalação do pacman (ADR 0011) e adoção ou ligação de dotfile (ADR 0023, 0025). Uma entrada Flatpak era recusada de propósito (ADR 0011 item 8, ADR 0014 item 4, ADR 0015 item 7), e isso era a lacuna do critério 10 do MVP. Remover app é operação destrutiva (`AGENTS.md` §4) e o histórico não é assinado (T5).

Medido em 2026-11-20 (Flatpak 1.18.4, `--user`):
- `flatpak uninstall --help`: não há `--dry-run` nem `--print`. As opções relevantes são `--user`, `--no-related`, `--unused`, `--delete-data`, `-y`, `--noninteractive`. **Não existe simulação** como o `pacman -Rs --print`.
- `flatpak uninstall --user -- org.nongnu.lingot` num terminal interativo, com o app dado por ID: **nenhum prompt**. Removeu `org.nongnu.lingot` e a extensão `org.nongnu.lingot.Locale` (as duas apareceram numa tabela `ID / Branch / Op`, depois "Uninstall complete."), exit 0. Antes disso eu tinha suposto que o Flatpak perguntaria (como no `install`, ADR 0015) e que responder `n` era uma recusa segura; era falso. O app foi mesmo removido durante a medição.

## Decisão
1. **Comando:** `flatpak uninstall --user -- <ids>`, mostrado por inteiro antes de `Executar? [s/N]`, com o terminal herdado. Nunca `-y`, `--assumeyes`, `--noninteractive`, `--unused`, `--delete-data`, `--system`, `--all`, `--force-remove` nem `--no-related`. O `--` vem antes dos IDs (T7).
2. **O `[s/N]` do nodus é a única confirmação.** Como o Flatpak não pergunta e não simula, o texto na tela diz isso (remove o app e as extensões relacionadas, como `.Locale`, assim que você confirma) e a confirmação é por operação, padrão N, sem "sim para tudo". Sem terminal, recusa.
3. **Candidatos** = IDs de `installed` da entrada que seguem instalados em `flatpak list --app --user --columns=application`. `--app` deixa os runtimes de fora; `--user` casa com o `install --user` (ADR 0015). Não existe "explícito" entre apps. O que cair fora aparece como `ignorado: <id> (não está instalado)`. Sem candidatos: "nada a desfazer", sem comando e sem registro. App instalado que **não** está na entrada nunca é alvo.
4. **Sem "já desfeita":** como no pacman, o estado decide. Depois da remoção o app não é mais candidato, e desfazer de novo dá "nada a desfazer". (Diferente do dotfile, que recusa pela existência de um `undoes` apontando para a entrada.)
5. **Formato: relaxar a v3.** A v3 passa a aceitar `remove` com `manager = flatpak` e `undoes >= 1` (antes só `pacman`). `install` segue sem `undoes`, e `manager = dotfiles` ou `dir` seguem recusados na v3. Um nodus anterior recusa a linha, como no ADR 0014.
6. **Registro:** uma entrada nova v3 `action = remove`, `manager = flatpak`, `undoes = <n>`; `requested` são os candidatos, `installed` o que segue instalado depois e `still_missing` o que saiu. Gravada também quando o comando falha, desde que ele tenha rodado.
7. **Onde mora:** `nodus-core::undo` (`select_flatpak_entry`, `flatpak_undo_candidates`, `flatpak_remove_command`, `entry_for_flatpak_undo`) e `nodus-cli/src/undo_flatpak.rs`, no molde do `dotfiles_undo.rs`; o `history_undo` desvia por `manager == Flatpak` antes de consultar o pacman.

## Alternativas
- **`--noninteractive`/`-y`:** descartada; o Flatpak já não pergunta, e esses flags só esconderiam saída.
- **Simular antes:** impossível (não há dry-run). Mostrar as extensões relacionadas antes exigiria adivinhar as refs (`flatpak list --all` e filtrar por prefixo), frágil; fica como melhoria se importar.
- **Subir para v5 só nessa linha:** descartada; mais cerimônia (constante e ramo de leitura) para o mesmo efeito em nodus antigo.
- **`--unused`:** descartada; remove runtimes órfãos de outros apps e amplia o raio de dano.
- **`--delete-data`:** descartada; os dados em `~/.var/app` ficam, como o `.pacsave` no pacman (ADR 0011). Que o `uninstall` sem a flag mantém os dados é o comportamento documentado do Flatpak, **não medido aqui**.

## Consequências
- Fecha a lacuna do critério 10 do MVP (reversão de Flatpak).
- Custa uma confirmação que é a única barreira: o dono precisa **ler o comando** antes de `s`. Resíduo declarado (T5): uma linha plantada no histórico pedindo o ID de **qualquer app instalado no `--user`** passa na leitura (o ID é válido) e vira candidato; o runtime não, por causa do `--app`. A defesa é o dono ler o comando e o aviso.
- Atualiza o ADR 0011 (item 8), o 0014 (item 4) e o 0015 (item 7): a reversão de Flatpak existe.
- A GUI **não** muda neste PR: o botão "Desfazer no terminal" ainda esconde entradas Flatpak (`canUndo` em `history.ts`); abrir o botão para Flatpak é um PR próprio.

## Não verificado
- Que os dados em `~/.var/app` ficam (o aviso na tela diz que o nodus não os apaga; não medido, e o app de teste talvez nunca tenha sido executado).
- App com mais de uma extensão relacionada, ou instalado em `--system`, e versões do Flatpak além da 1.18.4.

## Verificado no Arch real em 2026-11-20
Binário da branch (`cargo build --release --locked -p nodus-cli`, exceção ao ADR 0009 autorizada pelo dono, `nodusctl 0.1.0`), Flatpak 1.18.4, `--user`, app `org.nongnu.lingot`:
- `preset apply` instalou o app (entrada `[5]` v3 `flatpak`).
- `history undo 5`: a tela mostrou `Comando: flatpak uninstall --user -- org.nongnu.lingot`, o aviso de que o Flatpak não pergunta de novo e `Executar? [s/N]`; com `s`, removeu o app e o `.Locale` (`Uninstall complete.`); o app saiu de `flatpak list --app --user`; o `history` ganhou `[6] ok desfez [5] ... removidos: org.nongnu.lingot`.
- Desfazer a mesma entrada de novo: `ignorado: org.nongnu.lingot (não está instalado)` e `nada a desfazer`, sem rodar nada.
- `history undo 7` antes de a entrada existir: `entrada 7 fora do intervalo (o histórico tem 6)`.
- Reinstalado (entrada `[7]`), `history undo 7` respondendo `n`: `Nada foi executado.`; o app seguiu na lista e não surgiu a entrada `[8]`.
- Observação: a linha `[6]` do `history` é igual à de uma remoção do pacman (não diz `flatpak`); cosmético, fica como está.
- Este ADR foi escrito por IA e revisado só pelo dono; nenhuma revisão externa.
