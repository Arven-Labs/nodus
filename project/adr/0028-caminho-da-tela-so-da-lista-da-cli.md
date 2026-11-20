# 0028: Caminho vindo da tela só vale se a CLI o listou na hora

**Data:** 2026-11-19

## Contexto
O ADR 0027 deixou a GUI abrir um terminal com um comando da CLI, e a emenda do PR #100 permitiu um único argumento vindo da tela: um número (`u32`). Texto e caminho ficaram de fora de propósito (T2, T7), com a nota de que `dotfiles adopt`, `dotfiles link` e `workspace set` pedem desenho próprio. Um caminho tem sintaxe (`..`, `~`, `-x`, aspas, controle, symlink) e a GUI não linka o core (ADR 0026), então reimplementar `validate_path` e a denylist no Rust seria uma segunda cópia das regras, com risco de divergir.

## Decisão
Escolha do dono em 2026-11-19, em plan mode.
1. **Um caminho da tela só vira argumento se for idêntico (texto exato) a um item da lista que a própria CLI produz na hora.** O Rust relê a lista a cada clique, pelo `nodusctl::read` já existente (que confere a versão e o prazo), e compara. Não vira argumento: prefixo, sufixo, barra no fim, espaço, caixa diferente, `..`, relativo, vazio.
2. **Listas por ação:** `workspace_set` usa `candidates[].path` de `nodusctl workspace --json`. `dotfiles_adopt` (entrada com `kind = "file"`) e `dotfiles_link` (`kind = "missing"`) de `nodusctl dotfiles --json` entram nos PRs seguintes.
3. **Argv com `--` antes do caminho** (`nodusctl workspace set -- <pasta>`): uma pasta chamada `-x` é a pasta, não uma opção. Medido no CI do PR #103: o `clap` do `workspace set` aceita o `--` e recusa `-x` sem ele (`UnknownArgument`).
4. **Lista com forma inesperada é erro** (`bad_list`), nunca "sem candidatos"; caminho fora da lista é `not_in_list`; ação com `n` e caminho juntos, ou caminho numa ação que não o define, é `invalid_argument`.
5. **A lista é uma trava a mais, não a única:** a CLI continua revalidando tudo no terminal (existência, tipo, denylist, tamanho, symlink) e, onde pergunta, pede `[s/N]`.
6. **Nada mais muda do ADR 0027:** terminal sem shell, invólucro constante, lista fechada de ações, fallback com o comando.

## Alternativas
- **Campo de texto com filtro sintático no Rust:** descartada. Descreve o que é perigoso (nunca completo) e duplica `validate_path` e a denylist; mesmo com `--`, o filtro e a CLI poderiam divergir.
- **Seletor nativo de pasta/arquivo (plugin do Tauri):** descartada por ora. Dependência nova (avaliação e aprovação, `padrao/dependencias-novas.md`) e ainda exigiria filtro no Rust.
- **Só mostrar o comando para colar:** segue como fallback (ADR 0027, item 5).

## Consequências
- **Custo de reverter: fácil.** É um módulo e botões; nada persistente além do que a CLI já grava.
- Habilita: botões de ação sobre itens que a tela já mostra, sem abrir sintaxe de caminho nova.
- Custa: não dá para escolher pasta arbitrária nem adotar dotfile fora da lista curada pela GUI (segue no terminal). `workspace set` só aparece sobre candidatos, que existem só na detecção: com uma escolha gravada, `candidates` vem vazio e trocar é no terminal. Uma leitura extra (`workspace --json`) por clique.
- A lista vem do disco (nomes de pasta): dado externo, tratado só como texto na tela e como argumento no `argv`, nunca como script (o invólucro é constante).

## Implementado
- **PR #103 (`workspace set`)**: `Action::WorkspaceSet`, `Action::parse_path`, `require_workspace_candidate`; botão "Usar no terminal" em cada candidato da aba Workspace. O `workspace set` da CLI **não pergunta** nada (como o `unset`, ADR 0027), então o texto da tela não promete confirmação.
- **PR #104 (`dotfiles adopt`)**: `Action::DotfilesAdopt`, `require_adoptable_dotfile` (só entrada `kind = "file"`; a leitura da lista e o `bad_list` viraram `read_list`/`bad_list`, compartilhados com o `workspace set`); botão "Adotar no terminal" em cada arquivo comum da aba Dotfiles, escondido acima de 1 MiB (o limite do `adopt`, `MAX_ADOPT_BYTES`, que a GUI vê em `size`). Diferente do `workspace set`, o `adopt` **pergunta** `[s/N]` no terminal, então o texto manda confirmar lá. 
- **PR #105 (`dotfiles link`)**: `Action::DotfilesLink`; `require_adoptable_dotfile` virou `require_dotfile(…, tipo)`: `file` para adotar, `missing` para ligar (o `link` recusa um destino que já existe no HOME, então cada ação tem o seu tipo). Botão "Ligar no terminal" em cada entrada ausente da aba Dotfiles; nenhuma linha tem os dois botões. A tela **não vê a pasta-fonte**: se o arquivo não estiver lá, a CLI recusa no terminal. O `link` pergunta `[s/N]`, então o texto manda confirmar lá. Fecha as três ações de caminho previstas.
- **PR #107 (`preset unignore`)**: `Action::PresetUnignore`, `require_ignored_item`; a primeira ação em que o texto vindo da tela é um **nome** (pacote ou ID Flatpak), não um caminho. A lista de referência é o campo `ignored` de `nodusctl preset suggest --json <NODUS_PRESET>`, relido a cada clique (lista com forma errada é `bad_list`, nome fora dela é `not_in_list`); argv `preset unignore -- <nome>`. Botão "Voltar a sugerir no terminal" por item ignorado na aba Sugestões. Limite aceito: `ignored` só traz o que a CLI esconderia hoje, então um ignorado que não está mais instalado não aparece (esse `unignore` fica no terminal); nenhuma mudança na CLI (um `preset ignored --json` foi descartado). O `unignore` da CLI **não pergunta** (como `set` e `unset`), então o texto da tela não promete confirmação. `preset ignored` é leitura sem `--json` e não virou ação.

## Não verificado
- Comportamento do `konsole` com um nome de pasta hostil num app Tauri real (a medição do ADR 0027 foi no terminal do Claude Code); o teste usa um terminal falso que executa o invólucro de verdade.
- Uma pasta que sai da lista entre a leitura da tela e o clique: o Rust devolve `not_in_list`, mas o texto da mensagem na janela não foi visto.
- Este ADR foi escrito por IA e revisado só pelo dono.
