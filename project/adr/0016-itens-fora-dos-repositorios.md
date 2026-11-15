# 0016: Itens fora dos repositórios: `plan` e `apply` simulam e separam; `suggest` só relata

**Data:** 2026-11-15

## Contexto
O `preset init` lê `pacman -Qe`, que inclui pacotes de fora dos repositórios (AUR e locais, os que `pacman -Qem` lista). O `apply` roda `sudo pacman -S --needed -- <faltantes>`, e o `pacman -S` só enxerga os repositórios configurados. Isso apareceu em duas fatias: o `preset suggest` (critério 5 do MVP, PR #48) e a P-18.

Medido em 2026-11-15, nesta máquina, só leitura (`pacman -S --print`, `pacman -Si`, `pacman -Qe/-Qen/-Qem`):
- 154 pacotes explícitos, 141 nativos e **13 de fora dos repositórios** (`brave-bin`, `google-chrome`, `kwin-x11`, `ledger-live-bin`, `paru`, `yay`, ...); `pacman -Si` falha nos 13.
- `pacman -S --print --needed -- brave-bin` dá `error: target not found: brave-bin`, exit 1, **mesmo com o pacote já instalado**.
- No pedido misto (`sl`, que existe, mais `brave-bin`) a transação inteira aborta e o `sl` também não é instalado. Um único item assim trava o passo do pacman inteiro, em qualquer preset (gerado por `init`, escrito à mão ou de terceiro).
- `pacman -S --print --needed -- java-runtime`, nome que só existe como *provides*, resolve com exit 0. Uma lista de pacotes (`pacman -Slq`) o marcaria como inexistente.
- Na falha o stdout fica vazio e o nome do culpado só sai no stderr, que `RealRunner::run` descarta (devolve só o stdout).
- `-S --print` roda sem root e não instala nada.

## Decisão
1. **`plan` e `apply` separam por simulação.** `collect_plan` chama `pacman -S --print --needed -- <faltantes>`. Exit 0: todos instaláveis (uma única chamada, caminho comum). Se falhar, uma chamada por item; exit 0 é instalável, falha é indisponível. Erro de início (sem `pacman`) propaga e não vira "indisponível". Nada é consultado se não falta nada do pacman.
2. **Resultado em `Plan::pacman_unavailable`**, separado de `pacman_missing`. O plano mostra uma seção "Indisponíveis nos repositórios, o nodus não instala (N)"; `Plan::is_empty()` olha só o que há para instalar; `pacman_present` não muda.
3. **O `apply` pula e avisa.** O comando mostrado antes do `Executar?`, o histórico e o "Depois:" usam só `pacman_missing`. Um preset gerado por `init` nesta máquina continua utilizável.
4. **`init` e o formato do preset não mudam.** O preset continua registrando o que o dono usa, inclusive o que o nodus não instala.
5. **`suggest` só relata**, sem escrever no preset e sem estado persistido (escolha do dono para a fatia mínima do critério 5). Os de fora dos repositórios (`pacman -Qem`) saem numa seção própria e **fora do trecho para colar**, porque colados num `pacman = [...]` virariam indisponíveis no `apply`.
6. **O nodus não instala do AUR** (nem chama helper nem `makepkg`).

## Alternativas
- **`init` omitir os de fora e avisar:** descartada; não protege presets existentes nem escritos à mão, e quebra "`init` seguido de `suggest` dá vazio" (o `suggest` listaria os do AUR para sempre).
- **Formato v2 do preset com lista `foreign`:** descartada; maior (parser, `to_toml`, plano) e uma versão antiga do nodus recusaria o arquivo. Pode voltar se o dono quiser que o preset *registre* o que o nodus não instala de forma estruturada.
- **Classificar por lista de pacotes (`pacman -Slq`) ou `pacman -Si`:** descartada; erra grupos e *provides* (medido com `java-runtime`), que o `pacman -S` aceita.
- **Recusar o `apply` inteiro se houver indisponível:** descartada pelo dono; um preset do `init` nunca rodaria sem edição manual.
- **Capturar o stderr do pacman para ler o nome do culpado em uma só chamada:** descartada; mudaria `RunnerError` e o `CommandRunner` para todos os chamadores.
- **Instalar do AUR por helper:** descartada; o AUR não é revisado e rodar `makepkg` é execução de código de terceiro (supply chain), fora do que o MVP assume.
- **`suggest` interativo, escrevendo no preset, com lista de ignorados:** adiado para a fatia seguinte do critério 5 (precisa de estado e de escrita; o TOML sem perder comentários pede dependência nova).

## Consequências
- Uma chamada extra ao `pacman` por `plan`/`apply` quando falta algo; N chamadas só se o lote falhar. O custo em máquina nova com muitos itens faltando não foi medido.
- **Imprecisão aceita:** na chamada por item, qualquer exit 1 vira "indisponível", sem distinguir "não encontrado" de outro erro (por exemplo dependência insolúvel), porque o stderr não chega ao nodus. O item aparece no plano, nunca em silêncio.
- O `plan` deixa de prometer "instalar" algo que o `apply` não cumpriria.
- O `suggest` fica com ruído enquanto não houver "ignorar": com um preset pequeno ele lista dezenas de itens.
- Reabrir se: o `CommandRunner` passar a devolver o stderr (a classificação ficaria exata); o MVP passar a suportar AUR (exige ADR de segurança próprio); o dono quiser o formato v2.

## Não verificado
- O `apply` real com um item do AUR (só a simulação `--print` foi medida).
- **Verificado no Arch real em 2026-11-15** (binário novo, `pacman` desta máquina): `preset plan` com `sl` (ausente), `brave-bin` (AUR, instalado), um nome inexistente e `java-runtime` (só por *provides*) deu `pacman_missing = [sl, java-runtime]`, `pacman_present = 1` (`brave-bin`) e `pacman_unavailable = [nome inexistente]`, em texto e em `--json`; `preset init` seguido de `preset suggest` deu "Nada fora do preset".
- Ainda não verificado: o `apply` real com um item indisponível (precisa de terminal; deve mostrar o comando só com os instaláveis); a seção "Fora dos repositórios" do `suggest` com um preset que não contém os pacotes do AUR; um item do AUR **ausente** na máquina (aqui o `brave-bin` estava instalado, então contou como presente).
- Custo da varredura por item em máquina nova; outras versões do `pacman` além da desta máquina.
- Este ADR foi escrito por IA e revisado só pelo dono; nenhuma revisão externa.
