# 0002: Repositório público na Arven-Labs, licença MIT

**Data:** 2026-11-12

## Contexto
É preciso escolher dono, visibilidade e licença antes do primeiro commit. Os demais projetos da organização são públicos.

## Decisão
- Repositório `Arven-Labs/nodus`, **público**.
- Licença **MIT**, titular igual aos demais repos.

## Alternativas descartadas
- Repo privado: não há código nem segredo, e a spec foi revisada antes de publicar.
- AGPL-3.0: o projeto tem agentes e bibliotecas que outros vão linkar; AGPL atrapalharia a adoção.
- Conta pessoal em vez da organização: sem motivo.

## Consequências
- Público e MIT são praticamente irreversíveis para o que já foi publicado.
- A MIT não impede fork fechado; aceito, porque o objetivo é adoção.
- Qualquer agente com privilégios no dispositivo (AGENTS.md §4) exige revisão de segurança antes de existir.
