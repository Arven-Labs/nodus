# nodus: Pendências

Decisões em aberto. Marque como resolvida citando o ADR.

## Em aberto

| ID | Pendência | Observação |
|---|---|---|
| P-07 | Política pública de reporte de vulnerabilidade | Repo público; criar `.github/SECURITY.md` (ou usar private vulnerability reporting) antes de divulgar |
| P-08 | Assinatura, integridade e downgrade do binário | Lacuna do `SECURITY.md` §7; decidir antes da primeira versão distribuída |
| P-11 | `cargo-deny` no CI | Licenças e avisos, assim que houver código (ADR 0007) |
| P-12 | Confirmar a fronteira de licença "processo externo vs link" | Antes da primeira distribuição; não é parecer jurídico |
| P-02 | Identidade da marca e domínio | O nome `nodus` é latim comum, há homônimos; checar antes de publicar pacotes |
| P-03 | Renomear a spec | O `SPEC.md` ainda diz "Universal"; atualizar quando a Fase 0 começar |

## Resolvidas

| ID | Pendência | Resolução |
|---|---|---|
| P-01 | Fases, stack e MVP | ADRs 0003 (Fase 0 só docs), 0004 (MVP CLI sem GUI) e 0005 (stack provisória); ver `MVP.md` e `ROADMAP.md` |
| P-04 | Monorepo vs múltiplos repos | ADR 0006: monorepo com Cargo workspace |
| P-06 | Libs e licenças | `LIBS.md` e ADR 0007: Rust confirmado; pacman e Flatpak por processo externo (`alpm` é GPL-3.0); Git por processo externo isolado (P-09, ADR 0009) |
| P-10 | Spike de parsing de `pacman` e `flatpak` | ADR 0008 e amostras em `crates/nodus-core/tests/fixtures/`; `alpm-types` fora do MVP. Lacunas: nenhum app Flatpak medido, sem `pt_BR`, exit code sem atualizações não medido |
| P-09 | Spike do Git isolado | ADR 0009 e `project/spikes/p09-git/medir.sh`: `git` CLI isolado, só refs (sem `git status`). Lacunas: `credential.helper` não medido, `pre-auto-gc` não disparou, vetores e versões não listados |
| P-05 | Modelo de ameaças e permissões | `SECURITY.md` (escopo MVP local); lacunas viraram P-07 e P-08 |
