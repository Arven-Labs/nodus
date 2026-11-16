# 0019: `preset adopt`: perguntar e acrescentar ao preset por inserção de texto

**Data:** 2026-11-15

## Contexto
O critério 5 do MVP pede "detectar app recém-instalado e perguntar se entra no preset". O `preset suggest` (ADR 0016) detecta e relata, sem escrever. Faltam a pergunta, a escrita no arquivo e lembrar o que foi ignorado. O ADR 0016 adiou isso dizendo que escrever sem perder comentários "pede dependência nova". Esta é a primeira vez que o nodus **edita um arquivo que o dono escreveu e comentou à mão**.

Medido no código, só leitura: `toml` 1.2.1 já está no `Cargo.lock` e depende de `serde_spanned`, então `toml::Spanned` (a posição em bytes de cada valor no texto) está disponível sem crate novo.

## Decisão
1. **Duas fatias, escolha do dono em plan mode.** Este ADR cobre a fatia A: `nodusctl preset adopt <arquivo>` pergunta e escreve. A fatia B (ignorar para sempre) vem depois.
2. **Comando novo, não flag do `suggest`:** o `suggest` segue só leitura e com `--json`. O `adopt` exige terminal interativo (sem ele é erro e nada é escrito), pergunta `Adotar <nome> (pacman|flatpak)? [s/N/q]`, pacman antes de Flatpak: `s` ou `sim` adota, `q`, `sair` e fim da entrada param (o já adotado fica), qualquer outra coisa pula. Sem "sim para todos", como no `apply`. Os de fora dos repositórios (AUR e locais) são listados e **não perguntados** (o `apply` não os instala, ADR 0016).
3. **Escrita por inserção de texto** (`nodus_core::preset::add_to_preset`, função pura). Lê o preset com `toml::Spanned`, acha o `[`…`]` de cada lista e insere só o texto novo: multilinha, uma linha por nome antes da linha do `]`, com a vírgula do último elemento se faltar (olhando só espaço e comentário `#` até o próximo caractere); lista na mesma linha, `, "nome"` depois do último; lista vazia, dentro dos colchetes; lista ausente, criada no fim do arquivo. O fim de linha segue o do arquivo (LF ou CRLF). Nome já presente é ignorado e repetido entre os novos entra uma vez. **Escolha do dono**, entre esta, `toml_edit` e reescrever com `to_toml`.
4. **Defesa em profundidade:** os nomes novos passam pela mesma validação do `apply` (sem aspas, sem barra invertida, sem começar por `-`); o resultado é reparseado e precisa ter os nomes antigos seguidos dos novos, senão `PresetError::Edit` e nada é gravado.
5. **Gravação** (`preset_file::write_preset`): resolve symlink com `canonicalize` e grava no **alvo real** (um `rename` por cima do link o trocaria por um arquivo comum e quebraria um preset que vive em dotfiles); exige arquivo comum; preserva as permissões do original; só escreve se o arquivo ainda tem **exatamente** o texto que foi lido (comparação de conteúdo, não de `mtime`: determinística); troca atômica por temporário no mesmo diretório e `rename`, reusando `config::commit`. Falha apaga o temporário.
6. **Direção do PR B (sem código agora):** a lista de ignorados fica no `config.toml` do ADR 0018, em chaves opcionais (`ignore_pacman`, `ignore_flatpak`), com a versão ainda 1. **Escolha do dono.**

## Alternativas
- **`toml_edit`:** descartada pelo dono; edita TOML preservando comentários com menos código nosso, mas é crate novo com a própria árvore e passaria pelo `padrao/dependencias-novas.md`.
- **Reescrever com `to_toml`:** descartada; apaga os comentários e a ordem do dono (serviria só para preset sempre gerado por `init`).
- **Flag `--adopt` no `suggest`:** descartada; mistura relatório (com `--json`) e escrita.
- **Backup `.bak`:** descartada por ora; a mudança só acrescenta linhas, o diff do Git do dono a cobre.
- **Ignorados dentro do preset ou em arquivo de estado próprio:** descartadas pelo dono (mistura "quero instalar" com "não pergunte"; ou um segundo arquivo com as mesmas regras de segurança).

## Consequências
- Fecha, no PR A, a pergunta e a escrita do critério 5; falta o "ignorar" (PR B), sem o qual o `adopt` pergunta de novo a cada rodada o que foi pulado.
- Risco médio, aceito: primeira edição de arquivo do dono. Mitigado por inserção só (resto byte a byte, testado), reparse do resultado, gravação atômica, checagem de mudança concorrente e confirmação item a item. Não instala nada: instalar continua sendo o `apply`.
- A inserção usa 4 espaços de indentação fixos, mesmo que o arquivo use tabs.
- Um arquivo que o dono edite durante a conversa interativa faz o `adopt` falhar no fim, sem escrever (o dono responde de novo).
- `config::commit` passa a `pub(crate)` e é compartilhado com o config.

## Lacunas
- Verificado só no CI até o registro da sessão; o comando no Arch real é conferido à parte (ver `SESSIONS.md`).
- Sem teste de CLI de ponta a ponta do `adopt` (o `main.rs` só liga as peças testadas); sem teste de falha do `rename`; sem teste com um preset de verdade com muitos comentários além dos de unidade.
- Preset com a lista escrita de forma exótica (strings multilinha, tabelas inline) não foi exercitado além dos casos de unidade.
