# 0007: Libs do MVP e integração com pacman e Flatpak

**Data:** 2026-11-12

## Contexto
O ADR 0005 deixou Rust como stack provisória, a confirmar na Fase 0. O levantamento (`LIBS.md`) viu só metadados do crates.io. O achado relevante: `alpm`, a lib Rust para libalpm, é GPL-3.0, e o nodus é MIT (ADR 0002).

## Decisão
1. **Rust continua a stack do MVP.** Nenhum requisito do `MVP.md` apareceu que o contradiga. Isto confirma o ADR 0005, que segue valendo.
2. **pacman e Flatpak por processo externo**, com lista de argumentos e nunca por string de shell (SECURITY.md T7). Sem linkar `alpm` nem `libflatpak`.
3. **Núcleo adotado em princípio:** `clap`, `serde`, `toml`, `thiserror`, `anyhow`, `rusqlite`, `sysinfo`; `tempfile` só em testes. `alpm-types` é candidata para tipos e versões, a confirmar.
4. **Git em aberto:** `gix` ou `git` CLI isolado. Decide um spike no início da Fase 1 que prove qual não executa hooks nem `core.fsmonitor` no `fetch` (SECURITY.md §6). `git2` descartado.
5. **CI:** `cargo-deny` verifica licenças e avisos assim que houver código.
6. **Cada dependência exige aprovação do dono ao entrar** (AGENTS.md §E). Esta lista é uma pré-aprovação de direção, não de versões.

## Alternativas descartadas
- **`alpm` linkado:** a GPL-3.0 se impõe ao binário distribuído.
- **Trocar o nodus para GPL ou AGPL:** reabriria o ADR 0002 e prejudicaria a adoção.
- **`libflatpak`:** pouco usado e sem publicação há cerca de um ano.
- **`git2`:** código C nativo, mais superfície.
- **Trocar a linguagem (Go, Python):** sem requisito que justifique; ver ADR 0005.

## Consequências
- Parsing de saída de texto de `pacman` e `flatpak`: o risco passa a ser formato instável e injeção de argumento, tratados com formatos de máquina quando existirem e validação estrita de nomes.
- Sem dependência nativa de libalpm: o binário roda sem a lib instalada, mas depende dos executáveis `pacman` e `flatpak`.
- A fronteira "processo externo não contamina a licença" é prática comum, **não parecer jurídico**: confirmar antes da primeira distribuição.
- O levantamento não prova segurança das libs; a auditoria vem do CI e da triagem (ADR 0006 do workspace).
