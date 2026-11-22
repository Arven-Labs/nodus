# 0032: Formato e gravação do snapshot da máquina (v1)

**Data:** 2026-11-22
**Estado:** decidido e implementado (PR #122, `nodusctl snapshot`); **não verificado no Arch real**. Detalha a fatia 1 do ADR 0031.

## Contexto
O ADR 0031 escolheu "snapshot e compare por git, sem Core" como primeiro recorte da Fase 2. A fatia 1 só **grava** o snapshot; o `compare` é a fatia 2 e vai ler arquivos de **outras máquinas**, possivelmente de um nodus mais novo ou editados à mão. O arquivo vai para o repositório git do dono, que pode ser público. Isso o torna um dado persistente e exposto, então o formato e a gravação precisam de decisões explícitas (o ADR 0031 já pedia: versionar desde o início, sem segredos, sem conteúdo de arquivo).

## Decisão (escolhas do dono em 2026-11-22, mais o que a implementação fixou)
1. **Conteúdo mínimo (resumo do preset + capacidades):**
   ```json
   {"version": 1, "name": "meu-pc",
    "os": {"name": "Arch Linux", "version": null},
    "capabilities": {"pacman": true, "flatpak": true},
    "pacman": ["base", "git"], "flatpak": ["org.a.A"]}
   ```
   `pacman` são os pacotes instalados de propósito (`pacman -Qe`, o mesmo que o `preset init` lê) e `flatpak` os IDs dos apps. **Sem** hora, versão de pacote, hardware, kernel, hostname (a menos que o dono o use como rótulo) nem conteúdo de arquivo.
2. **Determinístico:** listas ordenadas, campos na ordem da struct, `\n` no fim. Dois snapshots iguais dão os mesmos bytes, então o `git diff` só mostra mudança real. Por isso não há `at_unix`.
3. **Capacidades explícitas:** `flatpak: true` com lista vazia (instalado, sem apps) é diferente de `flatpak: false` (ausente). O `compare` não pode acusar "faltam apps" onde nem há Flatpak. `pacman` é sempre `true` hoje; a chave existe para máquinas com outro gerenciador (Windows, ADR 0031).
4. **Rótulo = nome do arquivo:** `[a-z0-9][a-z0-9._-]{0,62}`. Sem `/`, `..`, maiúsculas ou vazio, para o nome não escapar da pasta. Sem `--name`, usa o hostname; se ele não servir de rótulo, o comando recusa e pede `--name`, em vez de trocar o nome em silêncio.
5. **Leitura estrita e versionada**, como o histórico (v1 a v4): `deny_unknown_fields`, e a **versão é conferida antes do resto** (um arquivo de um nodus mais novo dá "versão não suportada", não "campo desconhecido"). A leitura revalida rótulo, nomes de pacote e de app e duplicatas, como a escrita: o arquivo é entrada externa.
6. **Local:** `<dotfiles_dir>/.nodus/snapshots/<rótulo>.json`, reaproveitando o `dotfiles_dir` do `config.toml` e o repositório que o dono já tem. Sem chave nova no config.
7. **Gravação segura:** arquivo 0600, pastas 0700, temporário `create_new` e `rename` atômico. Recusa symlink em `.nodus`, em `snapshots` e no destino (o snapshot é do dono e não pode parar fora do repositório); recusa destino que seja pasta.
8. **Regravar substitui sem `[s/N]`:** atualizar o snapshot da própria máquina é o uso normal, o arquivo é um relatório no repo do dono e o git desfaz.
9. **O nodus nunca roda git de escrita.** O commit e o push são do dono.

## Alternativas
- **Inventário completo** (todos os pacotes com versão, hardware, discos): descartada. Lista longa e mutável (diff ruidoso no git) e expõe hardware num repositório possivelmente público; o `compare` do critério 2 do SPEC §16 só precisa de preset e capacidades.
- **Pasta própria no config (`snapshot_dir`):** descartada, é mais uma chave e uma validação sem dor concreta. Reabrir se o dono quiser snapshots fora do repo de dotfiles.
- **Hora no arquivo (`at_unix`):** descartada, quebraria o diff a cada gravação. Se for preciso saber quando, o git já sabe (`git log`).
- **Exigir `--name` sempre:** descartada por ora; o hostname como padrão é mais simples, com recusa clara quando não serve. Reabrir se o hostname num repo público incomodar.
- **Leitura tolerante (ignorar campos desconhecidos):** descartada. O `compare` leria um arquivo de outra versão e compararia errado em silêncio.
- **`[s/N]` antes de substituir:** descartada pelo item 8.

## Consequências
- Formato fixado em `version: 1`. Mudar exige `version: 2` e leitura dos dois (como o histórico). Remover o recurso é fácil enquanto o `compare` não existir.
- A fatia 2 (`nodusctl compare`, PR #125) herda as regras de leitura e decidiu as capacidades (critério 2 do SPEC §16): uma lista só é comparada se **as duas** máquinas têm a capacidade; sem ela, a lista é "não comparável" (cita quem não tem), nunca "faltam apps", e a capacidade manda mesmo que o arquivo traga a lista. SO diferente é aviso e conta como divergência. A regra foi decidida e testada, **não medida** com snapshots reais. A leitura do `compare` acrescenta o que o ADR não previa: `.nodus`, `snapshots` e o arquivo sem symlink, teto de 1 MiB e o `name` de dentro do arquivo igual ao rótulo pedido. A leitura do core **não valida `os.name`**: o `compare` troca byte de controle por `?` ao mostrar.
- O snapshot não prova que o preset foi aplicado: guarda o estado instalado, não o desejado. Comparar com um preset é outra decisão.

## Lacunas
- **Verificado no Arch real em 2026-11-22:** duas gravações seguidas deram o mesmo sha256 (o `git diff` vazio não serviria, a pasta ainda não era versionada), arquivo 0600 e o hostname como rótulo.
- `os.version` é `null` no Arch (o `os-release` dele não tem `VERSION_ID`); não medido em outra distribuição.
- `pacman -Qe` como conjunto de comparação, medido em 2026-11-22: um item do preset instalado só como dependência aparece como "não instalado de propósito" no `preset status`, enquanto o `preset plan` (que usa `pacman -Q`) o vê como presente. **Decisão do dono: manter** (opção A). Descartado: gravar também `pacman -Q` (formato v2), porque o snapshot passaria a mudar a cada atualização de dependência e contraria o conteúdo mínimo. Não escolhido: sugerir `pacman -D --asexplicit` na nota.
- Este ADR foi escrito por IA e revisado só pelo dono.
