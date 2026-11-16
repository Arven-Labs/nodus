# 0018: Config do usuário com a pasta de workspace gravada (`workspace set`)

**Data:** 2026-11-15

## Contexto
O ADR 0017 resolve o workspace por argumento, `NODUS_WORKSPACE` ou detecção, e deixou "salvar a escolha" de fora de propósito. A escolha manual valia por chamada ou presa à variável do shell. O critério 6 do MVP e a SPEC §6.6 pedem "detectar ou solicitar e lembrar". Esta é a primeira vez que o nodus escreve num arquivo de configuração do usuário (o histórico é estado e só acrescenta).

## Decisão
1. **Arquivo:** `$XDG_CONFIG_HOME/nodus/config.toml` (ou `~/.config/nodus/config.toml`; só valem caminhos absolutos, como no histórico). TOML com `version = 1` e `workspace = "/caminho/absoluto"`. Escolhido pelo dono em plan mode.
2. **Comandos:** `nodusctl workspace set <pasta>` valida (existe, é pasta, a mesma conferência da pasta informada), grava o **caminho real** e diz quantos repositórios há; `nodusctl workspace unset` esquece a pasta e apaga o config **só se não sobrar mais nada nele** (**emenda, PR #61:** o config ganhou as listas de ignorados, ADR 0020, que `set` e `unset` preservam). `workspace <pasta>` e `workspace --json` seguem como antes; `set` e `unset` não aceitam `--json` nem a pasta posicional (`args_conflicts_with_subcommands`: uma pasta chamada `set` exige `./set`).
3. **Precedência:** argumento, `NODUS_WORKSPACE`, **gravada**, detecção. O config só é lido se não houver argumento nem variável. Escolhido pelo dono em plan mode.
4. **Pasta gravada inválida é erro**, não detecção em silêncio (a pasta sumiu, virou arquivo, ou o config está corrompido): a mensagem diz para informar outra pasta, ou usar `set`/`unset`. Mesma regra da variável (ADR 0017, item 2).
5. **Mesma disciplina do histórico (SECURITY.md T5):** diretório 0700, arquivo 0600; gravação e leitura recusam symlink, o que não é arquivo comum e permissão mais larga que 0600 (reusa `history::check_file`, agora `pub(crate)`); leitura com limite de 64 KiB, UTF-8, chave desconhecida, versão diferente e caminho relativo são erro.
6. **Gravação atômica:** temporário `.config.toml.<pid>.tmp` no mesmo diretório (`create_new`, 0600), `sync_all` e `rename`; em falha o temporário é apagado e o config antigo fica inteiro.
7. **Dependência:** o `toml` (já aprovado e usado no core, ADR 0007) passa a ser herdado também pelo crate da CLI; sem crate novo no `Cargo.lock`.

## Alternativas
- **Arquivo de uma linha com só o caminho:** descartada pelo dono; não precisa de parser, mas a segunda chave exigiria migrar o formato.
- **Gravar no estado XDG (`~/.local/state/nodus/`):** descartada; a escolha do usuário é configuração, não estado (o XDG separa os dois).
- **Gravada acima da variável de ambiente:** descartada pelo dono; a variável é a mais específica ao shell atual.
- **Cair na detecção se a pasta gravada sumiu:** descartada; esconderia o engano, como no ADR 0017.
- **Escrever direto com `File::create`:** descartada; trunca antes de escrever e deixa o config vazio se falhar no meio.
- **Config por projeto ou outras chaves:** fora de escopo; entram com ADR quando houver a segunda chave.

## Consequências
- Habilita o `repos` sem argumento e sem variável em qualquer shell. Fecha o critério 6 do MVP.
- Primeira superfície de escrita em config do usuário: um config plantado por quem já escreve na conta só muda qual pasta o `repos` lê (só leitura de refs, com o isolamento do ADR 0009), sem execução nem escrita. O config não é assinado.
- `resolve` e `pick_dir` ganham o parâmetro `stored` (uma closure, para o config só ser lido quando decide).
- O `rename` só é atômico no mesmo sistema de arquivos; o temporário fica ao lado do config, então vale. `sync_all` no arquivo e não no diretório: uma queda de energia logo após o `rename` pode perder a troca (o config antigo continua inteiro).
- Reabrir se: surgir uma segunda chave de config (formato e migração), ou se quisermos o config por projeto. **Emenda (PR #61):** as chaves opcionais `ignore_pacman` e `ignore_flatpak` entraram (ADR 0020) dentro da versão 1, sem migração; toda escrita virou leitura-modificação-escrita sobre `Config`.

## Lacunas
- Verificado no Arch real em 2026-11-15 (`set`, `unset`, precedência, arquivo 0600, diretório 0700; ver `SESSIONS.md`). O conteúdo do `config.toml` não foi lido lá, só no teste de ida e volta do CI. O `unset` deixa o diretório `~/.config/nodus` vazio, de propósito.
- Sem teste de CLI de ponta a ponta de `set`/`unset` (o `main.rs` só liga as peças testadas); sem teste de falha de `rename` nem de temporário já existente.
- `workspace set` numa pasta com 0 repositórios grava e avisa em stderr; não foi medido o que o usuário espera nesse caso.
