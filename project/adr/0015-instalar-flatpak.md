# 0015: `preset apply` instala apps Flatpak

**Data:** 2026-11-15

## Contexto
O `preset apply` só instalava pacotes do pacman e avisava que os apps Flatpak do preset ficavam de fora. O histórico já aceita `manager = flatpak` (ADR 0014) e a etapa 0 mediu `flatpak install --user` (Flatpak 1.18.4, sessão 19): o `--` é aceito antes do remote; recusar o prompt e falhar saem os dois com exit 1; o prompt pergunta primeiro sobre o runtime.

## Decisão
1. **Comando:** `flatpak install --user -- flathub <ids...>`, mostrado por inteiro antes de `Executar? [s/N]`. Nunca `-y`, `--assumeyes`, `--noninteractive` (o prompt do Flatpak mostra runtimes e dependências, e pode perguntar mais de uma vez) nem `--system` (sem privilégio). `--` antes do remote e dos IDs (T7). O Flatpak herda o terminal, como o `sudo pacman`.
2. **Remote fixo `flathub`**, conferido por `flatpak remotes --user --columns=name` (nome exato, uma linha por remote). Outros remotes ficam fora do MVP.
3. **Dois passos em sequência**, pacman e depois Flatpak, cada um com o seu comando e a sua confirmação (confirmação por operação, ADR 0011). Recusar um passo não impede o outro. A falha de um passo interrompe o resto: se o pacman falha, o Flatpak não é oferecido.
4. **Pré-condições antes de qualquer pergunta:** com app Flatpak a instalar, se o Flatpak não está instalado ou o `flathub` não está no `--user`, o apply para com erro claro **antes** do passo do pacman, para não instalar um e só então descobrir que o outro é inviável. O nodus não instala o Flatpak nem adiciona remote. *(Emenda de 2026-11-22, PR #130: `preset apply --skip-flatpak` tira os apps Flatpak do plano antes do apply, então essa pré-condição não se aplica e só o passo do pacman é oferecido, com os apps pulados listados antes do plano. Sem a flag vale o que está escrito acima. Motivo: o preset é compartilhado por git entre máquinas, e uma máquina sem Flatpak ficava sem como aplicar nem a parte do pacman, a não ser editando o arquivo e tirando os apps da outra máquina (critério 2 do SPEC §16). Descartado: pular sempre, sem a flag, porque reverteria a razão deste item (apply parcial sem pedir). Verificado no Arch real em 2026-11-22 com Flatpak instalado, respondendo `n`; numa máquina sem Flatpak, não.)*
5. **Histórico:** uma entrada v3 por passo executado (`manager` do passo), gravada mesmo se o comando falhou; passo recusado não grava. `requested`, `installed` e `still_missing` vêm do lado do gerenciador (`entry_from` e `entry_from_flatpak`).
6. **Conferência pelo estado:** depois de cada passo o plano é recalculado (`flatpak list`, `pacman -Q`); o código de saída não diz o que mudou. O `preset apply` só falha por "ainda faltam" para o gerenciador cujo passo rodou.
7. **Reversão de Flatpak continua fora** (ADR 0011 item 8, ADR 0014 item 4). *(Emenda, ADR 0029: ela passou a existir; `flatpak uninstall --user` não pergunta nada, medido, então o `[s/N]` do nodus é a única confirmação.)*

## Alternativas
- **Uma confirmação única para os dois passos:** descartada; esconderia o segundo comando.
- **Seguir para o Flatpak depois de uma falha do pacman:** descartada; é mais surpreendente e o dono perde o ponto em que parou.
- **Checar o remote só no passo Flatpak:** descartada; o pacman já teria sido instalado.
- **`--noninteractive` ou `-y`:** descartadas; escondem runtimes e dependências.
- **`--system`:** descartada; exigiria privilégio e não foi medida.
- **Adicionar o remote `flathub` automaticamente:** descartada; muda configuração do usuário sem pedido e não foi medida.

## Consequências
- `Outcome::FlatpakOnly` e o aviso "Flatpak ainda não é instalado" deixam de existir.
- A entrada de histórico de um apply com os dois gerenciadores ocupa duas linhas, e o `history undo` numera por linha (a do Flatpak recusa, ADR 0014).
- Instalar um app baixa runtimes grandes; o prompt do Flatpak é quem mostra isso ao dono.

## Não verificado
- **Verificado no Arch real em 2026-11-15** (Flatpak 1.18.4, `--user`, `flathub`): `preset apply` com `org.gnome.Calculator` faltando mostrou o comando, pediu `Executar? [s/N]`, o Flatpak perguntou pelo runtime `org.gnome.Platform//51` e instalou 8 itens; o "Depois:" deu "nada a fazer"; o `history.jsonl` (0600) ganhou uma linha v3 com `manager = flatpak`, `installed` com o ID e `result` ok; `nodusctl history` a listou com o rótulo `flatpak`; `history undo 1` recusou com a mensagem do ADR 0014.
- Ainda não verificado: a recusa do prompt do Flatpak no fluxo real (só a recusa do `Executar?` e a da etapa 0), a falha de rede no meio do download e o apply com pacman e Flatpak juntos na máquina real.
- Apps com extensões ou múltiplos remotes com o mesmo ID; versões do Flatpak além da 1.18.4.
- Este ADR foi escrito por IA e revisado só pelo dono; nenhuma revisão externa.
