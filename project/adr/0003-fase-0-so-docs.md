# 0003: Fase 0 só de documentos e ADRs

**Data:** 2026-11-12

## Contexto
A SPEC §14 define a Fase 0 como pesquisa e provas técnicas. A §17 pede que a primeira entrega seja arquitetura, MVP, roadmap, segurança e ADRs, com esqueleto só depois de validar a arquitetura.

## Decisão
A Fase 0 não tem código. Entrega:
- `MVP.md` e `ROADMAP.md` (feitos).
- ADRs de stack e corte do MVP (0004, 0005).
- Modelo de ameaças e permissões (`SECURITY.md`).
- Decisão de monorepo vs múltiplos repos.
- Levantamento de libs e licenças para inventário, pacman, Flatpak e dotfiles.

**Critério de saída:** os itens acima aprovados e o primeiro marco da Fase 1 descrito de forma testável.

## Alternativas descartadas
- Spike descartável em Rust (inventário do Arch): útil, mas a stack ainda é hipótese; fica para o início da Fase 1.
- Ir direto ao esqueleto: contraria a §17.

## Consequências
- Nenhum risco de código inseguro nesta fase.
- Atraso até o primeiro código; aceito para não fixar arquitetura sem validar.
