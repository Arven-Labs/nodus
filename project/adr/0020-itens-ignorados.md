# 0020: Itens ignorados: `i` no `preset adopt`, lista no `config.toml`, `suggest` esconde e conta

**Data:** 2026-11-15

## Contexto
O `preset adopt` (ADR 0019) pergunta de novo, a cada rodada, o que o dono pulou, e o `preset suggest` lista dezenas de itens com um preset pequeno (ruído registrado no ADR 0016). Faltava "lembrar o que foi ignorado", o último pedaço do critério 5 do MVP. O ADR 0019 já havia fixado a direção (lista no `config.toml` do ADR 0018, versão 1); este ADR detalha e fecha.

## Decisão
1. **Lista no `config.toml`**, em chaves opcionais `ignore_pacman` e `ignore_flatpak` (arrays de nomes), a versão do arquivo continua 1. Valem para o usuário e para todos os presets (não são por preset). **Escolha do dono.**
2. **Leitura estrita, igual ao preset:** cada item é texto, passa na regra de nome do pacman ou do ID Flatpak (`nodus_core::preset::is_valid_pacman_name` e `is_valid_flatpak_id`, agora públicas) e não repete; senão o arquivo é recusado com o caminho. Um config com lista inválida derruba também o `workspace` e o `repos`, porque o arquivo é lido por inteiro.
3. **Toda escrita é leitura-modificação-escrita sobre `config::Config`** (`workspace`, `ignore_pacman`, `ignore_flatpak`), gravada de forma atômica como no ADR 0018. Antes, `write_workspace` regravava só `version` e `workspace`, o que **apagaria os ignorados** a cada `workspace set`. O arquivo só existe se há algo nele: `workspace unset` e `preset unignore` apagam o arquivo quando não sobra campo nenhum.
4. **`adopt`:** a resposta `i` (ou `ignorar`) manda ignorar o item; o prompt vira `[s/N/i/q]`. O que foi ignorado antes não é perguntado. No fim, grava o preset primeiro e depois o config; se o config falhar, o erro diz que o preset já foi escrito. `q` e o fim da entrada guardam os `i` já dados.
5. **`suggest`:** esconde os ignorados e termina com `N ignorado(s) (use --all para ver).`; `--all` mostra tudo e diz quais dos listados estão ignorados; `--json` ganha o campo `ignored` (os nomes escondidos, ou os marcados com `--all`). **Escolha do dono.** O filtro é `nodus_core::preset::hide_ignored`, pura; `ignore_pacman` vale também para a lista "fora dos repositórios" (por nome), embora esses itens nunca sejam perguntados.
6. **Desfazer:** `nodusctl preset ignored` lista e `nodusctl preset unignore <nome>...` tira. Nome que não está ignorado é erro e nada é gravado. **Escolha do dono.**

## Alternativas
- **Lista dentro do preset (`ignore = [...]`):** descartada; muda o formato do preset e mistura "quero instalar" com "não pergunte".
- **Arquivo de estado próprio:** descartada; um segundo arquivo com as mesmas regras de segurança.
- **`suggest` sem esconder (só o `adopt` respeita):** descartada pelo dono; o ruído do relatório continuaria.
- **Só editar o `config.toml` para desfazer:** descartada pelo dono; um `i` por engano exigiria abrir o arquivo.
- **Adicionar um campo ao `Suggestions` do núcleo:** descartada; mexeria em todos os construtores. O núcleo só ganhou `hide_ignored`, que devolve os nomes escondidos à parte.

## Consequências
- Fecha o critério 5 do MVP (detectar, perguntar, escrever e lembrar o ignorado).
- O ignorado vale para todos os presets do usuário e por nome: se o mesmo nome for de pacman e de Flatpak, as listas são separadas, mas `unignore` tira de qualquer uma que o tenha.
- Dois processos do nodus gravando ao mesmo tempo podem perder uma atualização (sem trava); aceito, é uso interativo de um dono só.
- O limite de 64 KiB do config comporta cerca de 2 mil nomes; passando disso o config é recusado.
- Reabrir se: o ignorado precisar ser por preset, ou se a lista ficar grande a ponto de pedir outro formato.

## Lacunas
- Verificado só no CI até o registro da sessão; o comando no Arch real é conferido à parte (ver `SESSIONS.md`).
- Sem teste de CLI de ponta a ponta de `ignored`/`unignore` e do fluxo `adopt` com `i` (o `main.rs` só liga as peças testadas); sem teste de falha do `rename`.
- `suggest` agora exige um `HOME` ou `XDG_CONFIG_HOME` absoluto (para achar o config), o que antes não precisava.
- `unignore` de nome em duas listas (pacman e Flatpak) não tem teste dedicado.
