# nodus: Log de sessões

Mais recente no topo.

### 2026-11-12 — Sessão 6 (Fase 1, marco 1: esqueleto e CI)

- **Pedido**: iniciar a Fase 1. Escolhas do dono: primeiro marco = esqueleto + CI; binário `nodus`, crates `nodus-*`.
- **Feito**: Cargo workspace (`nodus-core`, `nodus-cli`), `nodus --version` e `--help` com `clap`, `.github/workflows/ci.yml` (modelo `padrao/workflows/ci-rust.yml`), `.gitignore` com `target/`, `GUIDELINES.md`.
- **CI (resultado real, PR #6)**: commit 1 (só testes) falhou no clippy por `cannot find value VERSION` (erro de compilação, não asserção); commit 2 passou: 4 testes ok, clippy limpo, `guard` verde, Rust 1.100.0 no runner.
- **Desvio do plano**: não criei `rust-toolchain.toml`. Com `channel = "stable"` não fixaria nada, e o kin também não tem. Fixar a versão fica como decisão futura.
- **Não verificado**: o CI roda sem `--locked` e não há `Cargo.lock` no repo; o build não é reproduzível até o lock entrar. Nada de lógica do MVP foi escrita. O check `Rust (clippy and tests)` ainda não é obrigatório no ruleset.
- **Fica**: `Cargo.lock` (gerar uma vez), exigir o check no ruleset depois do merge, P-09 a P-12, P-02, P-03, P-07, P-08.

### 2026-11-12 — Sessão 5 (P-06: libs e licenças)

- **Pedido**: resolver a P-06 e fechar a Fase 0.
- **Feito**: `LIBS.md` (levantamento no crates.io) e ADR 0007. Achado: `alpm` é GPL-3.0, incompatível com linkar num binário MIT; pacman e Flatpak passam a ser processos externos. Rust confirmado. P-06 resolvida; novas P-09 a P-12.
- **CI**: PR #4 da sessão 4 mesclado, `guard` verde na `main`; `guard` do PR desta sessão a conferir.
- **Não verificado**: só metadados; sem leitura de código, `cargo audit` ou teste das libs. A fronteira de licença "processo vs link" não é parecer jurídico. `alpm-types` e `gix` não foram testados.
- **Fica**: P-09 a P-12 (Fase 1), P-02, P-03, P-07, P-08. O fechamento formal da Fase 0 depende de o dono aprovar.

### 2026-11-12 — Sessão 4 (P-04: monorepo)

- **Pedido**: resolver a P-04 (monorepo vs múltiplos repos).
- **Decisão**: monorepo com Cargo workspace, como o `kin` (ADR 0006). Plano aprovado pelo dono antes de escrever.
- **Feito**: ADR 0006; P-04 resolvida; `OVERVIEW.md` com a estrutura prevista. Nenhuma pasta nem `Cargo.toml` criados.
- **CI**: PRs #2 e #3 mesclados, `guard` verde na `main` nos dois; `guard` do PR desta sessão a conferir.
- **Não verificado**: nada de código; a decisão depende do contrato CLI/agente mudar mais rápido que o ritmo de releases, o que é previsão, não medição.
- **Fica**: P-06 (libs e licenças), P-07, P-08, P-02, P-03.

### 2026-11-12 — Sessão 3 (P-05: SECURITY.md)

- **Pedido**: ligar as proteções do repo; depois escrever o modelo de ameaças (P-05).
- **Conferido (sem mudar nada)**: ruleset `protect-main` ativo e idêntico ao do `arven`; secret scanning, push protection e alertas do Dependabot ligados. Security updates (PRs automáticos) desligado, igual ao `arven`, pelo padrão de triagem manual (ADR 0006 do workspace). Não testei um push direto na `main`, só a configuração.
- **Decisão assumida**: o modelo cobre só o MVP local; Core remoto, plugins e IA ficam como lacunas. A pergunta de escopo não teve resposta explícita.
- **Feito**: `SECURITY.md` (ativos, fronteiras de confiança, 9 ameaças, 3 classes de permissão, segredos, Git, supply chain, lacunas); P-05 resolvida; novas pendências P-07 e P-08.
- **CI**: PR #2 da sessão 2 mesclado; `guard` do PR desta sessão a conferir.
- **Não verificado**: as opções exatas para isolar o `git` (`SECURITY.md` §6) e a cobertura real das ameaças; sem revisão externa.
- **Fica**: P-04 (monorepo), P-06 (libs e licenças), P-07, P-08, P-02, P-03.

### 2026-11-12 — Sessão 2 (P-01: Fase 0 e MVP)

- **Pedido**: resolver P-01 (cortar um primeiro marco pequeno).
- **Decisões do dono**: Fase 0 só docs e ADRs; MVP = CLI + agente local, sem GUI; registro em ADRs + `MVP.md` + `ROADMAP.md`.
- **Feito**: ADRs 0003, 0004 e 0005 (stack provisória), `MVP.md`, `ROADMAP.md`; P-01 resolvida; novas pendências P-04 a P-06.
- **CI**: a conferir no PR desta sessão (`guard`).
- **Não verificado**: Rust como stack é hipótese da SPEC, sem validação; sem código nesta fase.
- **Fica**: P-04 (monorepo), P-05 (`SECURITY.md`), P-06 (libs e licenças), P-02, P-03.

### 2026-11-12 — Sessão 1 (criação do projeto)

- **Pedido**: criar o projeto a partir da spec "Universal", com o nome nodus, dentro da Arven Labs.
- **Decisão do dono**: repo público, MIT, só governança e docs; spec movida da raiz do workspace para `project/SPEC.md`.
- **Feito**: repo `Arven-Labs/nodus`, commit inicial na `main`, PR #1 com template, `guard` e `project/`.
- **CI**: a conferir no PR #1 (`guard`).
- **Não verificado**: ruleset e segurança ainda não ligados (dependem do `guard` verde na `main`).
- **Fica**: ligar ruleset, secret scanning, push protection e alertas do Dependabot depois do merge. *(Corrigido na sessão 3: já estavam ligados.)*
