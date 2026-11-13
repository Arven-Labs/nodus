# 0008: Formato de saída de pacman e Flatpak (resultado do spike P-10)

**Data:** 2026-11-12

## Contexto
O ADR 0007 manda chamar `pacman` e `flatpak` como processos externos e deixou em aberto o formato a interpretar. O MVP precisa de Flatpak no inventário (critério 2) e de verificação de atualizações (critério 8). Este spike mediu a saída real numa máquina Arch (pacman 7.1.0 / libalpm 16.0.1, Flatpak 1.18.4, checkupdates 1.13.1, `LANG=en_US.UTF-8`, 1313 pacotes, 1 remote `flathub` no escopo system). Só leitura. As amostras estão em `crates/nodus-core/tests/fixtures/`.

## O que foi medido

| Comando | Formato observado | Observações |
|---|---|---|
| `pacman -Q` | `nome versão` por linha | Versão pode ter epoch (`1:1.6.10-1`), `+` (`16.2.1+r23+g…`) e `rc` (`0.9rc5-2`) |
| `pacman -Qm` | igual a `-Q` | Só pacotes "foreign" (AUR e fora dos repositórios): 15 na máquina |
| `pacman -Qu` | `nome velha -> nova` | 92 itens; usa o banco **local**, que pode estar velho |
| `checkupdates` | igual a `-Qu` | 134 itens na mesma máquina; sincroniza uma cópia temporária (`/tmp/checkup-db-$UID`) e não altera o banco do sistema (lido no script) |
| `flatpak list --runtime --columns=application,version,branch,origin` | tabulação entre colunas, **sem cabeçalho** quando a saída não é terminal | Campo pode ser **vazio** (tab duplo): `VAAPI.Intel` sem versão. Mesmo ID aparece em ramos diferentes |
| `flatpak remote-ls --updates --columns=application,version,branch` | idem | Não traz a versão instalada; uma linha por ref com atualização |

Exit codes: `pacman -Qu` e `checkupdates` saíram com 0 com atualizações pendentes (medido). `checkupdates` sai com 2 quando não há atualização, lido no script e **não medido**. `flatpak list` e `remote-ls` saíram com 0 (medido).

`vercmp 1:2.0-1 1.9-1` devolveu `1` (epoch vence) e `vercmp 1.0 1.0-1` devolveu `0` (medido).

## Decisão
1. **Instalados:** `pacman -Q` (já em uso) e `flatpak list --columns=application,version,branch,origin,installation`, com a lista de colunas fixada pelo nodus, nunca o padrão do Flatpak.
2. **Atualizações de pacman:** `checkupdates` como fonte principal, por ver o repositório de fato. `pacman -Qu` fica como alternativa offline, rotulada como "pode estar desatualizada". Nenhuma das duas atualiza nada.
3. **Atualizações de Flatpak:** `flatpak remote-ls --updates` com colunas fixas.
4. **O nodus não compara versões no MVP.** `-Qu` e `checkupdates` já entregam o par `velha -> nova`; basta interpretá-lo. Logo, `alpm-types` **não entra agora** (reavaliar se um dia o nodus precisar ordenar versões; `vercmp` como processo é a alternativa antes de uma dependência).
5. **Parser tolerante só no que o formato permite:** pacman exige exatamente os campos esperados, sem caracteres de controle (como já faz o `parse_pacman_q`); Flatpak divide por tabulação **sem descartar campos vazios** e exige o número de colunas pedido. Linha fora do formato é erro tipado, não é pulada.
6. **Saída de texto não é contrato do Flatpak nem do pacman.** O contrato é: colunas pedidas explicitamente, separador tabulação, versão do formato registrada neste ADR. Mudou a versão do programa, os testes de amostra reais são a rede de segurança.

## Alternativas descartadas
- **`pacman -Qu` como fonte única:** a máquina medida mostrou 92 itens contra 134 do `checkupdates`; o banco local velho esconde atualizações.
- **`alpm-types` agora:** cobre `Version` com epoch/pkgver/pkgrel e `vercmp` (docs.rs 0.14.0, licença Apache-2.0 OR MIT), mas sem necessidade no MVP seria dependência sem uso.
- **Formato "máquina" do Flatpak (`--machine-readable`) ou JSON:** `flatpak list` não oferece JSON; a lista de colunas é o mais próximo de um formato estável.
- **Comparar versões como texto:** `1:2.0-1` é maior que `1.9-1`; ordenar strings erraria.

## Consequências
- O próximo marco (Flatpak no inventário e verificação de atualizações) tem amostras reais para testar antes de implementar.
- Dependência dos executáveis `checkupdates` (pacote `pacman-contrib`, conferido com `pacman -Qo`) e `fakeroot` (o script aborta sem ele); sem eles, o nodus cai para `pacman -Qu` e avisa.
- `checkupdates` usa a rede e grava em `/tmp`: pertence à classe "leitura com rede" e precisa aparecer assim no plano de execução (SECURITY.md).

## Não verificado
- **`flatpak list --app`:** no spike a máquina não tinha nenhum app Flatpak (0 linhas). **Atualização (marco 4, 2026-11-12):** com `org.nongnu.lingot` instalado via `--user`, saiu `org.nongnu.lingot<TAB>1.1.1<TAB>stable<TAB>flathub<TAB>user`, as 5 colunas pedidas. Ainda **não medido**: instalação de sistema para apps e app sem versão (o campo vazio vem dos runtimes).
- Instalações `--user` e outras instalações `--installation=NAME` (a `--user` veio vazia).
- Locale: `LC_ALL=C` deu saída idêntica a `en_US.UTF-8`, mas o locale `pt_BR` não está gerado na máquina, então **não testei idioma diferente**.
- Exit code de `pacman -Qu` e de `checkupdates` sem atualizações pendentes; `IgnorePkg` (linhas com `[ignored]`) não apareceu nas amostras. **Atualização (marco 5, 2026-11-13):** a máquina ainda tinha pendências (os três comandos saíram com 0), então o exit sem pendências **continua não medido**. O `nodus updates` assume `checkupdates` = 2 (lido no script) e `pacman -Qu` = 1 (comportamento conhecido do pacman) como "sem atualizações"; se a suposição estiver errada, o comando falha com erro em máquina em dia, não com lista errada.
- Outras versões de pacman e Flatpak, e outras distros.
