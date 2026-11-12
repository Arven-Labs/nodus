# 0006: Monorepo (um repo, Cargo workspace)

**Data:** 2026-11-12

## Contexto
A SPEC §17 e §18 deixam em aberto "monorepo versus múltiplos repositórios". O MVP (ADR 0004 e 0005) é uma CLI com agente local em Rust; desktop e Core só aparecem nas Fases 1.5 e 2. Nos outros projetos do workspace, cada projeto é um repo e já é um monorepo por dentro: `kin` (Cargo workspace com `crates/*`), `warden` (crates Rust mais desktop, web, extension e mobile) e `anchor`/`truthid` (contracts, desktop e mobile juntos).

## Decisão
Um único repo, `Arven-Labs/nodus`, com um Cargo workspace (`crates/*`), como o `kin`.

Estrutura prevista (nada é criado agora; fica para a Fase 1):
- `crates/`: modelo de preset, plano, adaptadores (pacman, Flatpak), CLI e agente.
- `desktop/` na Fase 1.5; Core na Fase 2 (crate ou pasta própria, a decidir quando existir).

## Alternativas descartadas
- **Vários repos** (`nodus-core`, `nodus-agent`, `nodus-desktop`, plugins): isolam releases e permissões, mas hoje há um único artefato. O contrato CLI/agente vai mudar com frequência no começo, e mudá-lo entre repos exige PRs coordenados e versão publicada. Custo sem benefício agora.
- **Monorepo com ferramenta de build pesada** (Bazel, Nx): o Cargo workspace já resolve o caso Rust.

## Consequências
- Um CI e um ruleset só; reaproveita `padrao/workflows/ci-rust.yml` (ADR 0009 do workspace: build e teste só no CI).
- Mudança de contrato entre CLI e desktop é um PR só, com teste dos dois lados.
- O repo e o CI crescem juntos; mitigar com cache e execução por crate.
- Reavaliar na Fase 6: plugins de terceiros (SPEC §11) e adaptadores comunitários podem viver em repos próprios sem desfazer este ADR.
- Extrair um crate para outro repo depois é possível (preservando histórico com `git filter-repo`), mas dá trabalho.
