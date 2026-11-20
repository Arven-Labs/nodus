# 0011: Reversão do `preset apply`: `pacman -Rs` simulado, só o que o nodus instalou, sempre confirmado (P-15)

**Data:** 2026-11-14

## Contexto
A fatia 3 (PR #21) grava cada `preset apply` em `~/.local/state/nodus/history.jsonl` com o comando fixo, os pacotes pedidos (`requested`), os que ficaram instalados (`installed`) e os que continuaram faltando (`still_missing`). O critério 10 do MVP também pede "permitir reversão quando suportada". Desfazer um `apply` é **remover pacote**, que é classe privilegiada (`SECURITY.md` §4) e destrutiva, e o histórico em que a reversão se apoia **não é assinado** (T5): quem escreve na conta do usuário pode alterá-lo. A P-15 exigia decidir antes de escrever o código.

Medido em 2026-11-14 (pacman 7.1.0, Arch, só leitura):

| Comando | Resultado |
|---|---|
| `pacman -Rs --print --print-format '%n %v' -- alsa-firmware` (pacote explícito, sem dependências) | **sem root**, exit 0, uma linha: `alsa-firmware 1.2.4-4` |
| `pacman -Rs --print -- glibc` (usado por outros) | exit ≠ 0: `removing glibc breaks dependency 'glibc' required by 7zip` |
| `pacman -Rs --print -- nodus-nao-existe` | exit 1: `target not found` |
| `pacman -Qi alsa-firmware` | `Install Reason : Explicitly installed` |

## Decisão
1. **Comando (a implementar na fatia 4):** `nodus history undo <n>`, com `n` o número da linha mostrado por `nodus history`. Uma entrada por vez; não existe "desfazer tudo".
2. **Candidatos à remoção** = pacotes de `installed` da entrada **que ainda estão instalados** (`pacman -Q`), **com razão de instalação explícita** (`pacman -Qe`) e com nome válido pela regra do pacman. O que cair fora aparece como `ignorado: <motivo>`. Sem candidatos: "nada a desfazer", sem `sudo`.
3. **Plano = resultado real da simulação:** `pacman -Rs --print --print-format '%n %v' -- <candidatos>`, que roda sem root e lista tudo o que sairia, inclusive dependências arrastadas. Se a simulação falhar (dependência quebrada, alvo inexistente), o nodus **para, sem chamar o `sudo`**. Emenda da fatia 4 (medida): o stderr do pacman fica herdado (aparece no terminal), mas a lista de dependências quebradas sai no **stdout** (979 linhas com `glibc`), então o nodus captura o stdout e mostra só as 5 primeiras linhas e `(+N linhas)`. O plano é lido linha a linha (`nome versão`, com época na versão) e todo candidato tem de aparecer nele.
4. **Execução:** `sudo pacman -Rs -- <candidatos>`, com o comando exato à vista e `Executar? [s/N]` do nodus (só `s`/`sim`), depois o `[Y/n]` do próprio pacman. Sem terminal interativo, recusa. **Nunca** `-n` (apagaria os `.pacsave`, único rastro das configurações do pacote), `-c` (cascata), `-d`/`-dd` (ignora dependências) nem `--noconfirm`.
5. **Dependências que vieram junto na instalação:** a fatia 3 não as registrou. O `-Rs` remove apenas as que ficam sem uso e não são explícitas; o que outro pacote passou a usar fica. Essa semântica de `-s` vem do conhecimento do pacman, **não** foi medida aqui (ver "Não verificado").
6. **Registro, sempre acrescentando:** a reversão grava uma **entrada nova** (formato `version = 2`, com `action = "remove"` e `undoes = <n>`); `version = 1` continua sendo lida como instalação. Nenhuma linha existente é editada. Falha do `sudo`/`pacman` também é gravada, como na fatia 3.
7. **Resíduo declarado (T5):** a validação barra o que já não está instalado e o que não é explícito (por exemplo, uma biblioteca de dependência), mas uma linha plantada pedindo a remoção de um pacote **explícito qualquer** (ex.: `linux`) passa nela, porque o pacman protege dependências, não pacotes-folha. A defesa que sobra é o dono **ler o plano antes de confirmar**, e é por isso que a simulação (item 3) é obrigatória e aparece inteira.
8. **O que a reversão não desfaz:** dados e configurações em `$HOME`, serviços habilitados, apps Flatpak (o nodus ainda não instala; *emenda, ADR 0029: a reversão de Flatpak existe, por caminho próprio, sem passar pelo pacman*), dotfiles. *(Emenda, ADR 0023: a reversão de pacman continua não tocando em dotfile; a adoção de dotfile tem a sua própria reversão, `history undo` numa entrada v4, que não passa pelo pacman.)*

## Alternativas descartadas
- **`-Rns`:** também apaga os `.pacsave`; perde o único rastro das configurações do pacote e não dá para refazer.
- **`-Rc`, `-Rd`, `-Rdd`:** cascata ou ignorar a checagem de dependências quebra o sistema para "desfazer" um app.
- **Cruzar com `/var/log/pacman.log`** (exigir `installed <pacote>` perto da hora da entrada): fecharia a linha plantada, mas é parse de log com fuso, falha com log rotacionado e dobra o teste. Pode voltar com ADR próprio se o risco importar.
- **Desfazer por cache (downgrade) ou snapshots de sistema (btrfs, timeshift):** dependem de configuração da máquina e saem do escopo do MVP.
- **Reverter o preset inteiro:** o histórico é por execução; "desfazer tudo" mistura intenções e amplia o raio de dano.
- **Confiar só no histórico, sem `pacman -Qe`:** a linha plantada poderia apontar para qualquer dependência de base.
8. **O histórico é um log de intenção, não de efeito (emenda de 2026-11-14, P-16):** `requested`, `installed` e `still_missing` falam só dos pacotes pedidos. Medido no Arch real: `preset apply` de `abi-dumper` instalou também `ctags` e `vtable-dumper`, e `history undo 1` removeu os 3 (igual ao plano da simulação), mas a entrada de remoção diz `removidos 1`. Mantido assim no MVP porque o `undo` não precisa das dependências (elas nunca são candidatas, senão uma linha plantada ampliaria o raio de dano, T5). **Descartadas:** gravar `also_removed` só na remoção (formato v3, assimétrico com a instalação) e comparar `pacman -Q` antes e depois nos dois lados (v3, duas chamadas extras). Reabrir com ADR novo se o histórico virar trilha de auditoria.

## Consequências
- A fatia 4 tem contrato fechado: candidatos, simulação, comando e registro. Falta medir a semântica de `-s` com um pacote que tenha dependência antes de confiar nela.
- Habilita desfazer um `apply` com um comando e ver antes o que sai. Custa uma chamada extra ao pacman (a simulação) e um formato v2 do histórico.
- `nodus history` passa a mostrar o número de cada linha (necessário para `undo <n>`).
- A segurança contra histórico adulterado continua dependendo de o dono ler o plano. Isso fica registrado em `SECURITY.md` (T5), não escondido.

## Medido na fatia 4 (2026-11-14, pacman 7.1.0, só leitura)
Pacote de teste `abi-dumper` (explícito), instalado com `nodus preset apply`; `ctags` e `vtable-dumper` vieram como dependência (`pacman -Qi`: "Installed as a dependency").

| Caso | Resultado |
|---|---|
| `-Rs --print --print-format '%n %v' -- abi-dumper` | exit 0, 3 linhas: `abi-dumper 1.4-2`, `vtable-dumper 1.2-3`, `ctags 1:6.2.1-1` (a época usa `:`) |
| 2 alvos válidos | exit 0, união sem duplicata |
| alvo repetido, ou alvo que é dependência de outro alvo | exit 0, deduplicado |
| um válido + `glibc` | exit 1; 979 linhas `removing glibc breaks dependency …` no **stdout**, 1 linha no stderr |
| um válido + pacote inexistente | exit 1; `error: target not found` só no stderr, stdout vazio |

## Não verificado
- A **remoção real** com dependência órfã (`sudo pacman -Rs`): só a simulação foi medida; a verificação real pelo `history undo` fica para depois do merge.
- Como a simulação se comporta com o banco do pacman travado por outra execução.
- O `-Rs` real remove exatamente o que o `--print` listou: assumido (mesmo resolvedor), não comparado.
- Este ADR foi escrito por IA e revisado só pelo dono; nenhuma revisão externa.
