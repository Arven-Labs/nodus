# 0033: Exceções por máquina no preset (`[machines.<rótulo>]`)

**Data:** 2026-11-22
**Estado:** decidido e implementado (PR #148, fatia 3 da Fase 2); **não verificado no Arch real**. Segue o ADR 0031 (snapshot e compare por git) e o ADR 0032 (rótulo da máquina).

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
2. **Só pular.** `add_*` por máquina fica para uma fatia futura, se a dor aparecer.
3. **Rótulo:** o mesmo do snapshot (`[a-z0-9][a-z0-9._-]{0,62}`, ADR 0032). `plan`, `apply` e `suggest` aceitam `--machine <rótulo>`; sem a flag, usam o hostname, **mas só se o preset tem `[machines]`**. Sem tabelas o hostname nem é lido, então nada muda para quem não usa o recurso. `status` usa o nome do snapshot.
4. **Preset efetivo:** `PresetFile::for_machine(rótulo)` devolve o preset sem os itens pulados, e é ele que `plan`, `suggest` e `status` (`status_for`) comparam. Nenhuma dessas funções sabe que existem exceções.
5. **Validação no parse (entrada não confiável):** rótulo, nomes e repetição nas mesmas regras das listas principais; **item em `skip_*` que não está na lista principal é erro**, e o arquivo todo é recusado mesmo numa máquina sem tabela (um typo não pode virar "pulei" que não pulou).
6. **Efeito de um item pulado e instalado:** aparece em "sobram" no `status` e é sugerido pelo `suggest`, porque o preset efetivo da máquina não o tem.
7. O `apply` e o `plan` (texto) imprimem `Itens pulados nesta máquina (<rótulo>): ...`; o `plan --json` não ganha campo novo.

## Alternativas
- **`version = 2`:** descartada; obrigaria migrar todo preset existente sem ganho, já que o campo é opcional.
- **Passar as exceções adiante em `plan`, `suggest` e `status`:** descartada; três lugares para esquecer de tratar. Filtrar antes deixa um só.
- **Só `--machine`, sem hostname por padrão:** descartada; esquecer a flag faria o `apply` instalar o que devia pular.
- **Pular e acrescentar (`add_*`):** descartada nesta fatia; dobra a superfície (validação, `adopt`, `suggest`, `status`) sem dor concreta.

## Consequências
- **`preset adopt` com tabelas:** adotar numa lista existente funciona e deixa as tabelas intactas (testado). Adotar o que exigiria **criar** uma lista ausente (`pacman`/`flatpak`) num arquivo com `[machines]` recusa e não grava: o texto novo cairia dentro da última tabela. Contorno: criar a lista à mão antes.
- **A GUI** chama `preset plan` e `preset suggest` sem `--machine`. Com `[machines]` no preset ela depende do hostname; se o hostname não servir de rótulo (maiúsculas, por exemplo), a aba mostra o erro pedindo `--machine`. Não verificado na janela real. A aba Status não é afetada.
- Custo de reverter: baixo enquanto ninguém tiver `[machines]` no preset; depois disso, um nodus sem o recurso recusa o arquivo.

## Lacunas
- Verificação no Arch real pendente: `--machine`, hostname como padrão e o aviso de pulados.
- O hostname vem de `/proc/sys/kernel/hostname`, igual ao `snapshot`; não testei hostname com maiúsculas, nem a GUI.
- Mutações pegas por testes (CI): `for_machine` sem filtrar (9), `status_for` ignorando as exceções (2), `skip` fora da lista principal aceito (2), CLI filtrando com rótulo vazio (3), `--machine` não validado (1).
- Este ADR foi escrito por IA e revisado só pelo dono.
