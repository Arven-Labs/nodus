# 0033: Exceções por máquina no preset (`[machines.<rótulo>]`)

**Data:** 2026-11-22
**Estado:** decidido e implementado (PR #148, fatia 3 da Fase 2); verificado no Arch real em 2026-11-22 (ver Lacunas). Segue o ADR 0031 (snapshot e compare por git) e o ADR 0032 (rótulo da máquina).

## Contexto
O preset é um arquivo só, compartilhado por git entre as máquinas do dono. O `preset status` (PR #127) e o `compare` mostram o que difere, mas não distinguem "faltando por esquecimento" de "esta máquina não quer isso": um item que só vale no desktop aparece como divergência no notebook para sempre, e o `preset apply` o instalaria lá. O critério 2 do SPEC §16 (aplicar o mesmo preset "respeitando capacidades diferentes") só estava coberto para Flatpak (`--skip-flatpak`, ADR 0015).

## Decisão (escolhas do dono em 2026-11-22, mais o que a implementação fixou)
1. **Formato:** tabelas opcionais no próprio preset, `version = 1` mantida.
   ```toml
   [machines.notebook]
   skip_pacman = ["steam"]
   skip_flatpak = ["org.gnome.Calculator"]
   ```
   Um nodus antigo recusa o arquivo (`deny_unknown_fields`), nunca ignora a exceção em silêncio.
2. **Só pular nesta fatia.** `add_*` ficou para depois; **entrou na fatia 4 (ver "Emenda", abaixo)**.
3. **Rótulo:** o mesmo do snapshot (`[a-z0-9][a-z0-9._-]{0,62}`, ADR 0032). `plan`, `apply` e `suggest` aceitam `--machine <rótulo>`; sem a flag, usam o hostname, **mas só se o preset tem `[machines]`**. Sem tabelas o hostname nem é lido, então nada muda para quem não usa o recurso. `status` usa o nome do snapshot.
4. **Preset efetivo:** `PresetFile::for_machine(rótulo)` devolve o preset sem os itens pulados, e é ele que `plan`, `suggest` e `status` (`status_for`) comparam. Nenhuma dessas funções sabe que existem exceções.
5. **Validação no parse (entrada não confiável):** rótulo, nomes e repetição nas mesmas regras das listas principais; **item em `skip_*` que não está na lista principal é erro**, e o arquivo todo é recusado mesmo numa máquina sem tabela (um typo não pode virar "pulei" que não pulou).
6. **Efeito de um item pulado e instalado:** aparece em "sobram" no `status` e é sugerido pelo `suggest`, porque o preset efetivo da máquina não o tem.
7. O `apply` e o `plan` (texto) imprimem `Itens pulados nesta máquina (<rótulo>): ...`; o `plan --json` não ganha campo novo.

## Alternativas
- **`version = 2`:** descartada; obrigaria migrar todo preset existente sem ganho, já que o campo é opcional.
- **Passar as exceções adiante em `plan`, `suggest` e `status`:** descartada; três lugares para esquecer de tratar. Filtrar antes deixa um só.
- **Só `--machine`, sem hostname por padrão:** descartada; esquecer a flag faria o `apply` instalar o que devia pular.
- **Pular e acrescentar (`add_*`):** adiada nesta fatia, porque dobra a superfície (validação, `adopt`, `suggest`, `status`) sem dor concreta; o dono a pediu na fatia 4 (ver "Emenda").

## Consequências
- **`preset adopt` com tabelas:** adotar numa lista existente funciona e deixa as tabelas intactas (testado). Adotar o que exigiria **criar** uma lista ausente (`pacman`/`flatpak`) num arquivo com `[machines]` recusa e não grava: o texto novo cairia dentro da última tabela. Contorno: criar a lista à mão antes.
- **A GUI** chamava `preset plan` e `preset suggest` sem `--machine`. Com `[machines]` no preset ela dependia do hostname; se ele não servisse de rótulo (maiúsculas, por exemplo), a aba mostrava o erro pedindo `--machine`, sem como passá-lo. **Resolvido na fatia 5 (ver "Emenda 2")**. A aba Status não é afetada.
- Custo de reverter: baixo enquanto ninguém tiver `[machines]` no preset; depois disso, um nodus sem o recurso recusa o arquivo.

## Emenda (2026-11-22, fatia 4): `add_*` por máquina
Escolhas do dono em plan mode; implementado no PR #156 e verificado no Arch real em 2026-11-22 (item 7).
1. `[machines.<rótulo>]` aceita também `add_pacman` e `add_flatpak`. O preset efetivo é a lista principal **sem os pulados, seguida dos acrescentados** (na ordem escrita). `version = 1` continua.
2. **Acrescentar o que já está na lista principal é erro no parse** (`AddAlreadyInPreset`); isso cobre também o mesmo item em `skip_*` e `add_*` da mesma máquina. Descartado: ignorar em silêncio, que esconde engano e deixa lixo no arquivo. Nomes e repetição em `add_*` seguem as regras das listas principais.
3. `status`, `plan`, `apply` e `suggest` não mudaram: já comparavam o efetivo. O aviso passa a ter até duas linhas: `Itens pulados nesta máquina (...)` e `Itens acrescentados nesta máquina (...)`.
4. **`preset adopt` aceita `--machine`** (hostname por padrão se há `[machines]`) e usa o efetivo da máquina, mas continua gravando só nas listas compartilhadas e **não oferece o que já está na lista compartilhada** (um item pulado e instalado ficaria fora do efetivo; `add_to_preset` o ignoraria e o relatório diria "adotado" sem gravar). Descartado: deixar `adopt` e `suggest` inconsistentes.
5. **Consequência conhecida:** adotar para a lista compartilhada um item que **outra** máquina acrescenta é recusado sem gravar (o texto novo viraria redundante e não passa no parse); a mensagem traz a causa (`add_pacman` da máquina "outra" cita "7zip", que já está na lista principal do preset), mas só aparece **depois** de responder a todas as perguntas, porque a releitura acontece ao gravar. Contorno: tirar o item do `add_*` da outra máquina antes. Visto no Arch real: seis `s` seguidos de `q`, erro, arquivo intacto.
6. Mutações pegas por testes (CI): `for_machine` sem acrescentar (11), `add` já na lista principal aceito (4), `adopt` sem filtrar a lista compartilhada (1), `added()` devolvendo os pulados (22), aviso trocando pulados e acrescentados (4), `adopt` ignorando `--machine` (2).
7. **Verificado no Arch real em 2026-11-22** (binário da `main` em `5bfc279`, hostname `archlinux`): `plan` pelo hostname (aviso de acrescentados, `sl` a instalar) e com `--machine outra-maquina` (sem aviso, nada a fazer); `add_pacman` repetindo a lista principal (erro citando `"sl"`, saída 1); `add_pacmna` (erro de campo desconhecido listando os quatro válidos, saída 1); `apply` respondendo `n`; `suggest` com `sl` acrescentado e instalado (0 ocorrências, ou seja, conhecido); `adopt --machine` com um item pulado e instalado (`7zip`: a primeira pergunta passou a ser `alsa-firmware`) e com um item que outra máquina acrescenta (recusa sem gravar, item 5).
   **Não verificado:** a GUI (continua sem `--machine`, e o `adopt` não está nela); `status` com o acrescentado instalado só foi visto por ausência (`"sl"` não apareceu em `missing` nem em `extra`; `in_sync` é só uma contagem, então eu esperava "1 ou mais" ocorrências e errei o esperado); hostname com maiúsculas; flatpak em `add_flatpak` (só em teste). O primeiro passo do `adopt` do roteiro (`q` no primeiro item, com `sl` ainda não instalado) não provava nada sobre o filtro; os casos do `7zip` provam.

## Emenda 2 (2026-11-22, fatia 5): `NODUS_MACHINE` na GUI
Escolha do dono: variável de ambiente lida no Rust, como `NODUS_PRESET` e `NODUS_TERMINAL`; implementado no PR #164, **ainda não verificado na janela real**.
1. Se `NODUS_MACHINE` está definida, a GUI passa `--machine <rótulo>` (dois argumentos, nunca `--machine=<valor>`) a `preset plan` e `preset suggest` (leituras), a `preset apply` e `preset adopt` (ações no terminal) e à releitura de `ignored` do `preset unignore`, que usa `preset suggest`. O comando do `unignore` em si não leva a flag (a CLI não a tem). `preset status` não a recebe (usa o nome do snapshot; a CLI a recusa). Sem a variável, o argv é o de antes.
2. Valor: a regra do rótulo (ADR 0032). Valor ruim, inclusive vazio, é erro `invalid_machine` (nunca "usa o hostname", que pode ser o que o dono quis evitar), antes de rodar qualquer coisa, e só nas abas e ações que a usam. A ordem das conferências é: arquivo do preset, máquina, terminal, versão da CLI.
3. Descartado: campo de texto na tela; o ADR 0028 só deixa vir da tela o que a CLI lista na hora.
4. Mutações pegas por testes (CI do desktop): plan e suggest sem a flag (2 testes), `status` recebendo a flag (1), apply e adopt sem a flag (1), releitura do `unignore` sem a máquina (2), regra do rótulo só exigindo não vazio (3), `--machine=<valor>` num argumento só (2).
5. **Não verificado:** a janela real; `NODUS_MACHINE` vazia (`NODUS_MACHINE=` no shell) quebra as abas em vez de ser ignorada (decisão acima, que o dono pode querer mudar).

## Emenda 3 (2026-11-23, fatia 8): o `preset status` imprime o trecho da exceção
Escolhas do dono em plan mode; implementado no PR #201 (mesclado em `c6e5a06`), **verificado no Arch real em 2026-11-23** (item 6).
1. Por máquina, o `preset status` (texto) imprime, depois das listas, o trecho para colar: `add_pacman`/`add_flatpak` para o que está instalado de propósito e fora do preset efetivo, e `skip_pacman`/`skip_flatpak` para o que está no preset efetivo e não instalado. Só leitura; o dono cola. Mora no `status` e não no `compare`, porque só o status conhece o preset. `status --json` não ganha campo.
2. **Só sugere o que o parse aceita:** item da lista principal instalado numa máquina que o pulou **não** vira `add_` (seria `AddAlreadyInPreset`: pulo deliberado); item que a própria máquina acrescenta e não instalou **não** vira `skip_` (seria `SkipNotInPreset`; o conserto é tirá-lo do `add_*`); lista não comparável (sem a capacidade) não gera sugestão; nome que o parse recusaria fica de fora.
3. Sem tabela da máquina, o trecho traz `[machines.<rótulo>]` inteira. **Com a tabela já existente, o trecho só diz em qual lista acrescentar** (`em \`add_pacman = [...]\``), porque repetir a chave seria TOML inválido. Descartado: imprimir sempre a tabela inteira.
4. Implementação: `exceptions_for` e `MachineExceptions` em `nodus-core/src/status.rs`; `PresetFile::has_machine`; `render_status` passa a receber o `PresetFile` (o cabeçalho usa `file.preset`). Descartados: só `add_*` e uma flag `--snippet`.
5. Mutações pegas por testes (CI, PRs #202 a #206, fechados): `add_` sem o filtro da lista principal (2 testes), `skip_` sem exigir a lista principal (2), `add_` e `skip_` trocados (13), nome inválido aceito (2), tabela da máquina nunca existente (3).
6. **Verificado no Arch real em 2026-11-23** (binário instalado com `cargo install --locked --git ... --rev c6e5a06`, hostname `archlinux`, preset de teste em `/tmp` gerado pelo `preset init` com `git` removido e `item-que-nao-existe` acrescentado): (a) o `status` mostrou `item-que-nao-existe` em "faltando", `git` em "sobrando" e o trecho `[machines.archlinux]` com `add_pacman = ["git"]` e `skip_pacman = ["item-que-nao-existe"]`; (b) colado o trecho no preset, o `status` deu "em dia com o preset" (Pacman 208, Flatpak 1) e nenhum trecho novo; (c) removida a linha `add_pacman`, o `status` disse que a tabela já existe e mandou acrescentar `"git",` em `add_pacman = [...]`, sem repetir o cabeçalho; (d) o `status --json` saiu sem campo de exceção. Antes da reinstalação o `status` não imprimia o trecho: o binário instalado era de uma cópia local antiga (`~/nodus-build`).
   **Não verificado:** a GUI (a aba Status não mudou); o trecho com Flatpak (só em teste); máquina sem Flatpak no Arch real.

## Emenda 4 (2026-11-23, fatia 10, PR 1 de 2): `preset status --json` ganha `exceptions`
Escolha do dono em 2026-11-23, entre "campo aditivo no `--json`", "GUI sem o trecho" e "GUI mostra o texto". **Inverte o "`status --json` não ganha campo" da emenda 3**: a GUI só consome JSON (ADR 0026/0028) e a aba Status não tinha como mostrar o trecho.
1. Cada item de `machines[]` ganha `exceptions`: `{"table_exists": bool, "add": {"pacman": [], "flatpak": []}, "skip": {"pacman": [], "flatpak": []}}`. A chave está **sempre presente**, com listas vazias quando não há exceção. Os nomes são os de `exceptions_for` (só o que o parse do preset aceita); o rótulo não entra no objeto (já está em `machine`).
2. Implementação: `Serialize` em `Items` e `MachineExceptions`; `status_json` em `crates/nodus-cli/src/render.rs` monta o relatório e `preset_status` o chama. **`MachineStatus` não muda** (`status()` não conhece o `PresetFile`), e `o_json_tem_este_formato` ficou intocado.
3. Descartado: pôr `exceptions` dentro de `MachineStatus` (muda o tipo puro e o teste de formato); omitir a chave quando vazia (o parser da GUI teria de aceitar dois formatos).
4. O PR 2 (desktop) faz o parser **exigir** `exceptions`: um `nodusctl` antigo com a GUI nova dá erro citando `machines[0].exceptions`, em vez de esconder o descompasso. Quem atualiza a GUI atualiza o `nodusctl` (conferir com `cargo install --list`).
5. Mutações pegas por testes (CI, PRs #219 a #221, fechados): `table_exists` sempre falso (4 testes), `machine` aparecendo dentro de `exceptions` (2), ilegíveis fora do JSON (1). Todas compilaram e passaram o clippy.
6. **Não verificado:** o `--json` no Arch real (o dono roda `nodusctl preset status <preset> --json` com o binário novo).
7. **PR 2 (desktop, PR #222, 2026-11-24):** `parsePresetStatus` **exige** `exceptions` (campo ou tipo errado falha citando o caminho, ex.: `machines[0].exceptions`); `exceptionSnippet` em `desktop/src/lib/presetStatus.ts` monta o trecho com as mesmas frases do `exceptions_section` do terminal; `MachineStatusView` mostra o bloco `status-excecao` (`<pre>`, só texto) quando há exceção. Sem botão "copiar" (`navigator.clipboard` na webview do Tauri não foi verificado): o dono seleciona o texto. Mutações pegas por testes (PRs #223 a #225, fechados): `add` e `skip` trocados (3 testes), parser sem exigir `exceptions` (1), bloco sempre mostrado (1). **Não verificado:** a janela real, com um `nodusctl` que já tenha o #218.

## Lacunas
- **Verificado no Arch real em 2026-11-22** (binário da `main` em `ed0451b`, hostname `archlinux`): `plan` pelo hostname e com `--machine`, `--json`, rótulo inválido, typo em `skip` (erro, saída 1), `apply` respondendo `n`, `adopt` num preset com tabelas (só a linha de `pacman` mudou) e `status` com e sem a exceção (0 contra 1 ocorrência de `"sl"`).
- **Não verificado:** a GUI (abas Preset e Sugestões sem `--machine`); `suggest --machine`; `apply` real confirmando com `s`; hostname com maiúsculas (o `archlinux` é um rótulo válido); o conteúdo do `--json` do `status` (só contei ocorrências).
- O hostname vem de `/proc/sys/kernel/hostname`, igual ao `snapshot`.
- Mutações pegas por testes (CI): `for_machine` sem filtrar (9), `status_for` ignorando as exceções (2), `skip` fora da lista principal aceito (2), CLI filtrando com rótulo vazio (3), `--machine` não validado (1).
- Este ADR foi escrito por IA e revisado só pelo dono.
