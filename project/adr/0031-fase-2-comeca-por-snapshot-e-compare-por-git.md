# 0031: Fase 2 começa por snapshot e compare por git, sem Core

**Data:** 2026-11-21
**Estado:** decidido. Em 2026-11-22 o dono passou a implementar: a fatia 1 (`nodusctl snapshot`) está mesclada (ADR 0032, PR #122); o `compare` (fatia 2) segue sem plano. O item 4 abaixo descreve o estado de 2026-11-21.

## Contexto
O `ROADMAP.md` descreve a Fase 2 como "Windows (WinGet), Core central, multi-device, tarefas remotas com aprovação": quatro frentes. A dor concreta vem do `SPEC.md` §16 (critérios multi-device): dois computadores, o mesmo preset nos dois, mudanças sincronizadas, divergências e falhas visíveis, operação local sem o Core. A pesquisa de reuso do workspace (2026-11-13) deixou em aberto se o nodus orquestra ferramentas prontas (osquery, Ansible, MeshCentral, Headscale) ou reimplementa. O Core remoto está fora do modelo de ameaças (`SECURITY.md` §8) e é área de risco (AGENTS §4): autenticação, enrollment, credenciais. O SPEC §15 diz para não construir solução empresarial antes de validar o uso pessoal e multi-device.

## Decisão (escolha do dono em 2026-11-21)
1. **O primeiro recorte da Fase 2 é "snapshot e compare por git", sem servidor e sem rede:** cada máquina grava um snapshot (inventário e estado do preset, em JSON versionado, sem segredos) no repositório que já guarda o preset e os dotfiles, e um `compare` lê os snapshots e mostra as divergências. Cobre os critérios 2, 3 e 4 do SPEC §16; o 5 (operar local sem Core) vem de graça.
2. **Core, Windows e tarefas remotas ficam para depois** deste recorte, não descartados.
3. **Gatilho para reabrir:** se o recorte mostrar que git não basta (tempo real, tarefas remotas, enrollment), decide-se o Core com um ADR próprio, e só então a pergunta "orquestrar ou reimplementar" (a pesquisa de reuso aponta adaptadores sobre ferramentas prontas, com Headscale/MeshCentral à vista). Antes do Core, atualizar o `SECURITY.md` (enrollment, autenticação mútua, revogação).
4. **Nada foi implementado.** Este ADR só registra a escolha; a fatia seguinte começa por plano e testes primeiro (plan mode).

## Alternativas
- **Não começar a Fase 2 (fechar a 1.5 e usar antes):** descartada pelo dono em favor do recorte A, que é pequeno e reversível; segue válida como argumento (o ROADMAP diz que cada fase só começa com a anterior validada, e a GUI ainda tem itens não vistos na janela real).
- **Core mínimo agora (B):** descartada; é a decisão mais difícil de reverter (protocolo, credenciais), toca segurança, e exigiria antes resolver "orquestrar ou reimplementar" e o modelo de ameaças.
- **Windows primeiro (C, WinGet):** descartada por ora; só vale se o dono usa Windows e o testa à mão (build só no PC dele, ADR 0017 do workspace).

## Consequências
- Valida a necessidade de um Core antes de pagar por ele, sem superfície de rede nova.
- O formato do snapshot é dado persistente: versionar desde o início, como o histórico v1 a v4 (leitura estrita, versão desconhecida recusada), e nunca incluir segredos nem o conteúdo dos arquivos.
- Custo de reverter: médio (um formato novo e um comando; remover é fácil enquanto ninguém depender dele).
- O dono tem as máquinas para verificar à mão; sem uma segunda máquina real, a verificação cairia em HOMEs de mentira.

## Lacunas
- Confiança média: não há protótipo nem medida do que um snapshot útil precisa conter nem de como o `compare` lida com capacidades diferentes entre máquinas (critério 2 do SPEC).
- Este ADR foi escrito por IA e revisado só pelo dono.
