# nodus: Pendências

Decisões em aberto. Marque como resolvida citando o ADR.

## Em aberto

| ID | Pendência | Observação |
|---|---|---|
| P-04 | Monorepo vs múltiplos repos | Decidir na Fase 0 (SPEC §17) |
| P-07 | Política pública de reporte de vulnerabilidade | Repo público; criar `.github/SECURITY.md` (ou usar private vulnerability reporting) antes de divulgar |
| P-08 | Assinatura, integridade e downgrade do binário | Lacuna do `SECURITY.md` §7; decidir antes da primeira versão distribuída |
| P-06 | Libs e licenças para inventário, pacman, Flatpak e dotfiles | Entrega da Fase 0; confirma ou troca a stack (ADR 0005) |
| P-02 | Identidade da marca e domínio | O nome `nodus` é latim comum, há homônimos; checar antes de publicar pacotes |
| P-03 | Renomear a spec | O `SPEC.md` ainda diz "Universal"; atualizar quando a Fase 0 começar |

## Resolvidas

| ID | Pendência | Resolução |
|---|---|---|
| P-01 | Fases, stack e MVP | ADRs 0003 (Fase 0 só docs), 0004 (MVP CLI sem GUI) e 0005 (stack provisória); ver `MVP.md` e `ROADMAP.md` |
| P-05 | Modelo de ameaças e permissões | `SECURITY.md` (escopo MVP local); lacunas viraram P-07 e P-08 |
