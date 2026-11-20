# 0014: Histórico v3 com `manager` (pacman ou flatpak)

**Data:** 2026-11-15

## Contexto
O histórico (`~/.local/state/nodus/history.jsonl`) só registra operações do pacman, e a leitura valida cada nome com a regra do pacman (`[a-z0-9@._+-]`). O próximo passo do MVP é instalar apps Flatpak no `preset apply`, cujos IDs têm maiúsculas (`org.gnome.Gedit`). Gravar um ID assim hoje faria a próxima leitura recusar o arquivo inteiro (a leitura é estrita, T5) e o `nodusctl history` pararia de funcionar. O arquivo só cresce, então o formato tem de mudar antes de qualquer gravação Flatpak.

## Decisão
1. **Formato v3:** a entrada ganha o campo `manager` (`"pacman"` ou `"flatpak"`), serializado por último. O nodus passa a gravar sempre v3, inclusive para o pacman.
2. **Leitura:** aceita v1, v2 e v3. Em v1 e v2 o `manager` não existe e vale `pacman`; `manager` em v1 ou v2 é recusado. Em v3 o `manager` é obrigatório.
3. **Nomes:** validados pela regra do gerenciador da própria entrada (pacman ou `is_valid_flatpak_id`). Um ID Flatpak numa entrada `pacman` é recusado.
4. **Reversão:** `remove` só existe com `manager = pacman`. `history undo` de uma entrada Flatpak recusa com mensagem clara; a reversão de Flatpak fica fora (ADR 0011, item 8). *(Emenda, ADR 0029: a v3 passou a aceitar `remove` com `manager = flatpak` e `undoes >= 1`, e a reversão de Flatpak existe.)*
5. **Escrita só acrescenta:** nenhuma linha existente é reescrita. O arquivo do dono continua com linhas v2 e passa a ter linhas v3 depois delas.
6. `nodusctl history` marca as entradas Flatpak com `flatpak` na linha; as do pacman ficam como estavam.

## Alternativas
- **Arquivo separado para Flatpak:** descartada; quebra a ordem única e a numeração do `history undo`.
- **Relaxar a validação para qualquer nome visível:** descartada; a validação estrita é a defesa de T5 contra escape de terminal e opção injetada.
- **Inferir o gerenciador pelo `command`:** descartada; é inferir de campo livre, que é justamente o que a leitura estrita evita.
- **Manter v2 e acrescentar `manager` opcional:** descartada; `deny_unknown_fields` e a regra "versão determina os campos" ficariam ambíguas.

## Consequências
- Um nodus anterior (que lê só até a v2) recusa uma linha v3 com `UnsupportedVersion`, e não mostra o histórico. É o comportamento desejado: melhor recusar do que interpretar errado.
- Todos os testes e fixtures com `version: 2` passaram a v3; a leitura de v1 e v2 continua coberta por linhas literais.
- Abre caminho para o PR de instalação Flatpak (ADR 0015, a escrever).

## Não verificado
- O `history.jsonl` real do dono (a compatibilidade com linhas v2 é testada com linhas literais, não com o arquivo dele).
- Este ADR foi escrito por IA e revisado só pelo dono; nenhuma revisão externa.
