# 0032: Formato e gravação do snapshot da máquina (v1; v2 na emenda abaixo)

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

## Emenda (2026-11-22, fatia 6): formato v2, dotfiles
Escolha do dono em plan mode (os quatro estados); implementado no PR #171 e verificado no Arch real em 2026-11-22 (item 9).
1. `version: 2` acrescenta `capabilities.dotfiles` e a lista `dotfiles: [{"path", "state"}]`, ordenada por caminho (mesmo determinismo do item 2). O `state` é um de `linked` (symlink cujo alvo é, **em texto**, `<pasta-fonte>/<caminho>`, a regra do `undo`, ADR 0025), `file`, `missing` ou `other` (pasta, symlink para outro lugar ou outro tipo). **Nunca** o alvo do symlink (tem o caminho do HOME), o tamanho (suja o `git diff`) ou o conteúdo.
2. A lista é a efetiva **desta** máquina (padrão mais `dotfiles_extra`, menos `dotfiles_skip`); os extras do dono entram, então ficam visíveis se o repo for público. A denylist de segredos vale na escrita (`with_dotfiles`) e na leitura (o arquivo é entrada externa).
3. **Leitura dos dois formatos** (como previa a seção Consequências): o v1 vira `capabilities.dotfiles: false` e lista vazia; versão diferente de 1 e 2 é recusada ("entende a 1 e a 2"); a versão é conferida antes do resto; o v1 continua estrito (`dotfiles` nele é campo desconhecido). `build` sozinho produz v2 com `dotfiles: false`; `with_dotfiles` liga a capacidade.
4. **Compare:** só compara dotfiles se **as duas** máquinas têm a capacidade; v1 contra v2 (ou v1 contra v1) é "não comparável" e cita quem não olhou, nunca "diferente" (o mesmo raciocínio do Flatpak ausente). Entre v2, estado diferente ou caminho só na lista de uma das máquinas (`fora da lista`) é divergência; igual, inclusive `missing` nos dois lados, conta em `same`. O `compare --json` ganhou o campo `dotfiles`. Duas máquinas com listas diferentes não chamam de `missing` o que a outra não monitora.
5. `nodusctl snapshot` passa a exigir `$HOME` válido (como o `dotfiles`): sem ele erra e não grava, em vez de gravar `dotfiles: false` em silêncio. Os testes existentes que fixavam o v1 mudaram de propósito.
6. `preset status` não mudou (o preset não tem dotfiles); só lê os snapshots v1 e v2.
7. Descartado: gravar só presente/ausente (não distingue gerenciado de solto, que é a divergência que importa); gravar o alvo (expõe o HOME); `compare` exigir v2 (quebraria os snapshots que o dono já tem no repo).
8. Mutações pegas por testes (CI): qualquer symlink absoluto como `linked` (3), v1 lido como se tivesse olhado (8), denylist fora da validação (2), `compare` ignorando a capacidade (7), caminho de um lado só como igual (4), lista em ordem inversa (3), `extra` e `skip` trocados (1), estados de `a` e `b` trocados no texto (2).
9. **Verificado no Arch real em 2026-11-22** (binário da `main` em `d1c60b5`, máquina `archlinux`): o snapshot antigo era v1 e `nodusctl snapshot` o regravou em v2, com os 9 caminhos ordenados e cada estado batendo com `nodusctl dotfiles` (`.bash_logout` symlink para a pasta-fonte = `linked`; `.bashrc`, `.bash_profile`, `.profile` e `.gitconfig` arquivo = `file`; os outros quatro ausentes = `missing`); `grep` do HOME e de `target|size` no arquivo deu 0 e 0; `compare archlinux archlinux` deu `Dotfiles: 9 em comum` e `Sem divergências.`; um v1 forjado (`antigo`) contra o v2 deu "não comparável" citando só o v1; uma cópia forjada com um `missing` virado `file` deu `diferem (1)` em `.config/git/config`, no texto e no `--json` (`same: 8`, `a: missing`, `b: file`); `preset status` leu v1, v2 e a cópia sem aviso de ilegível; regravar deu o mesmo sha256.
   **Não verificado:** o `compare` entre duas máquinas reais (os snapshots comparados saíram da mesma máquina, um deles forjado); a GUI (sem aba Compare); o `snapshot` sem `HOME` (só no CI: sem `HOME` e sem `XDG_CONFIG_HOME` o erro vem antes, do config); um `dotfiles_extra` real. O snapshot lista os pacotes instalados de propósito (154 aqui, com `bitwarden`, `ledger-live-bin` e `rustdesk-bin`), o que vale revisar antes de um push para um repo público.

## Emenda 2 (2026-11-22, fatia 7): `compare --all`
Escolha do dono em plan mode; implementado no PR #181 (CLI), **ainda não verificado no Arch real**. O PR 2 da fatia (aba Compare na GUI) vem depois.
1. `nodusctl compare --all [--json]` compara **todos os pares** de snapshots legíveis da pasta-fonte, em ordem de rótulo (`read_all` já ordena), cada par com o mesmo `compare` de dois rótulos. `--all` conflita com os rótulos; sem `--all` e sem dois rótulos é erro de uso, como antes. `compare <a> <b>` não mudou.
2. **Snapshot ilegível vira aviso** (mesmo `read_all` do `preset status`), nunca derruba os outros. **Menos de dois legíveis não é erro** (código 0): lista de pares vazia e, no texto, "Preciso de pelo menos dois snapshots legíveis para comparar (há N)." (um ilegível não conta no N).
3. JSON: `{"comparisons": [<Comparison>...], "unreadable": [{"label", "reason"}]}`. Texto: um bloco por par separado por linha em branco e os avisos no fim. Com exatamente dois snapshots, a saída é idêntica à do `compare` dos dois rótulos.
4. **Motivo:** a GUI só manda o nome do comando (ADR 0026/0028); comparar todos os pares evita um rótulo livre vindo da tela. Descartado: escolher dois rótulos na GUI.
5. **Custo conhecido:** os pares crescem com N² (10 máquinas, 45 blocos), e cada bloco lista os pacotes só de um lado. Para uso pessoal (2 a 5 máquinas) é aceitável; sem limite nesta fatia.
6. Mutações pegas por testes (CI): pares incluindo cada snapshot consigo mesmo (4), pares sem linha em branco (2), mensagem de poucos snapshots só para zero (3), avisos de ilegível sumindo (3), `label` trocado por `name` no JSON (1), `--all` sem conflito com os rótulos (1), lados do par trocados (2).
7. **Não verificado:** o Arch real (`compare --all` com snapshots de verdade); a GUI (PR 2).

## Lacunas
- **Verificado no Arch real em 2026-11-22:** duas gravações seguidas deram o mesmo sha256 (o `git diff` vazio não serviria, a pasta ainda não era versionada), arquivo 0600 e o hostname como rótulo.
- `os.version` é `null` no Arch (o `os-release` dele não tem `VERSION_ID`); não medido em outra distribuição.
- `pacman -Qe` como conjunto de comparação, medido em 2026-11-22: um item do preset instalado só como dependência aparece como "não instalado de propósito" no `preset status`, enquanto o `preset plan` (que usa `pacman -Q`) o vê como presente. **Decisão do dono: manter** (opção A). Descartado: gravar também `pacman -Q` (um formato que mudaria o conteúdo; o v2 que existe hoje é o dos dotfiles, na emenda), porque o snapshot passaria a mudar a cada atualização de dependência e contraria o conteúdo mínimo. Não escolhido: sugerir `pacman -D --asexplicit` na nota.
- Este ADR foi escrito por IA e revisado só pelo dono.
