# nodus: Log de sessões

Mais recente no topo.

### 2026-11-12 — Sessão 2 (P-01: Fase 0 e MVP)

- **Pedido**: resolver P-01 (cortar um primeiro marco pequeno).
- **Decisões do dono**: Fase 0 só docs e ADRs; MVP = CLI + agente local, sem GUI; registro em ADRs + `MVP.md` + `ROADMAP.md`.
- **Feito**: ADRs 0003, 0004 e 0005 (stack provisória), `MVP.md`, `ROADMAP.md`; P-01 resolvida; novas pendências P-04 a P-06.
- **CI**: a conferir no PR desta sessão (`guard`).
- **Não verificado**: Rust como stack é hipótese da SPEC, sem validação; sem código nesta fase.
- **Fica**: P-04 (monorepo), P-05 (`SECURITY.md`), P-06 (libs e licenças), P-02, P-03; ruleset e segurança do repo, se ainda não ligados.

### 2026-11-12 — Sessão 1 (criação do projeto)

- **Pedido**: criar o projeto a partir da spec "Universal", com o nome nodus, dentro da Arven Labs.
- **Decisão do dono**: repo público, MIT, só governança e docs; spec movida da raiz do workspace para `project/SPEC.md`.
- **Feito**: repo `Arven-Labs/nodus`, commit inicial na `main`, PR #1 com template, `guard` e `project/`.
- **CI**: a conferir no PR #1 (`guard`).
- **Não verificado**: ruleset e segurança ainda não ligados (dependem do `guard` verde na `main`).
- **Fica**: ligar ruleset, secret scanning, push protection e alertas do Dependabot depois do merge.
