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
- **A GUI** chama `preset plan` e `preset suggest` sem `--machine`. Com `[machines]` no preset ela depende do hostname; se o hostname não servir de rótulo (maiúsculas, por exemplo), a aba mostra o erro pedindo `--machine`. Não verificado na janela real. A aba Status não é afetada.
- Custo de reverter: baixo enquanto ninguém tiver `[machines]` no preset; depois disso, um nodus sem o recurso recusa o arquivo.

## Emenda (2026-11-22, fatia 4): `add_*` por máquina
Escolhas do dono em plan mode; implementado no PR #156, **ainda não verificado no Arch real**.
1. `[machines.<rótulo>]` aceita também `add_pacman` e `add_flatpak`. O preset efetivo é a lista principal **sem os pulados, seguida dos acrescentados** (na ordem escrita). `version = 1` continua.
2. **Acrescentar o que já está na lista principal é erro no parse** (`AddAlreadyInPreset`); isso cobre também o mesmo item em `skip_*` e `add_*` da mesma máquina. Descartado: ignorar em silêncio, que esconde engano e deixa lixo no arquivo. Nomes e repetição em `add_*` seguem as regras das listas principais.
3. `status`, `plan`, `apply` e `suggest` não mudaram: já comparavam o efetivo. O aviso passa a ter até duas linhas: `Itens pulados nesta máquina (...)` e `Itens acrescentados nesta máquina (...)`.
4. **`preset adopt` aceita `--machine`** (hostname por padrão se há `[machines]`) e usa o efetivo da máquina, mas continua gravando só nas listas compartilhadas e **não oferece o que já está na lista compartilhada** (um item pulado e instalado ficaria fora do efetivo; `add_to_preset` o ignoraria e o relatório diria "adotado" sem gravar). Descartado: deixar `adopt` e `suggest` inconsistentes.
5. **Consequência conhecida:** adotar para a lista compartilhada um item que **outra** máquina acrescenta é recusado sem gravar (o texto novo viraria redundante e não passa no parse); a mensagem é a genérica "não consegui acrescentar ao preset". Contorno: tirar o item do `add_*` da outra máquina antes.
6. Mutações pegas por testes (CI): `for_machine` sem acrescentar (11), `add` já na lista principal aceito (4), `adopt` sem filtrar a lista compartilhada (1), `added()` devolvendo os pulados (22), aviso trocando pulados e acrescentados (4), `adopt` ignorando `--machine` (2).
7. **Não verificado:** tudo no Arch real; a GUI (continua sem `--machine`, e o `adopt` não está nela).

## Lacunas
- **Verificado no Arch real em 2026-11-22** (binário da `main` em `ed0451b`, hostname `archlinux`): `plan` pelo hostname e com `--machine`, `--json`, rótulo inválido, typo em `skip` (erro, saída 1), `apply` respondendo `n`, `adopt` num preset com tabelas (só a linha de `pacman` mudou) e `status` com e sem a exceção (0 contra 1 ocorrência de `"sl"`).
- **Não verificado:** a GUI (abas Preset e Sugestões sem `--machine`); `suggest --machine`; `apply` real confirmando com `s`; hostname com maiúsculas (o `archlinux` é um rótulo válido); o conteúdo do `--json` do `status` (só contei ocorrências).
- O hostname vem de `/proc/sys/kernel/hostname`, igual ao `snapshot`.
- Mutações pegas por testes (CI): `for_machine` sem filtrar (9), `status_for` ignorando as exceções (2), `skip` fora da lista principal aceito (2), CLI filtrando com rótulo vazio (3), `--machine` não validado (1).
- Este ADR foi escrito por IA e revisado só pelo dono.
