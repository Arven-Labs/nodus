# nodus: Pendências

Decisões em aberto. Marque como resolvida citando o ADR.

## Em aberto

| ID | Pendência | Observação |
|---|---|---|
| P-13 | Chave de assinatura de tags | Antes da primeira tag (ADR 0010): escolher a chave SSH, cadastrá-la como signing key no GitHub, testar `git tag -v` numa máquina limpa e publicar a chave pública |
| P-14 | PKGBUILD no AUR | Depois que o MVP fechar (ADR 0010). Conferir antes na doc do `makepkg` como a assinatura da tag é verificada; o AUR não é revisado |
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
| P-11 | `cargo-deny` no CI | Job `cargo-deny` (não obrigatório) e `deny.toml` com `allow = MIT, Apache-2.0, Unicode-3.0`; ver `LIBS.md`. Lacuna: a regra de avisos depende da base do RustSec do dia. As licenças de `ntapi` e `objc2*` foram conferidas no registry em 2026-11-13 (MIT, Apache-2.0 ou Zlib, sempre com MIT ou Apache como opção) |
| P-09 | Spike do Git isolado | ADR 0009 e `project/spikes/p09-git/medir.sh`: `git` CLI isolado, só refs (sem `git status`). Lacunas: `credential.helper` não medido, `pre-auto-gc` não disparou, vetores e versões não listados |
| P-07 | Política pública de reporte de vulnerabilidade | `.github/SECURITY.md` (inglês) e private vulnerability reporting do GitHub ligado em 2026-11-14 (`gh api` confirmou `enabled: true`). Melhor esforço, sem SLA, só a `main`. Lacuna: o dono precisa ter as notificações de advisory ativas e conferir na aba Security que o botão aparece |
| P-08 | Assinatura, integridade e downgrade do binário | ADR 0010: só código-fonte, tag assinada, `--locked`, sem `self-update`; binário só com ADR novo. Lacunas: nada foi implementado (viram P-13 e P-14) e a verificação da tag pelo `makepkg` e pelo `cargo install` não foi conferida |
| P-15 | Semântica da reversão (fatia 4) | ADR 0011: `nodus history undo <n>`, `sudo pacman -Rs` (nunca `-n`, `-c`, `-d`, `--noconfirm`) depois de simular com `pacman -Rs --print`, só o que o nodus instalou e ainda é explícito, confirmação por operação, entrada nova no histórico (v2). Lacunas: nada implementado (é a fatia 4); a semântica de `-s` com dependências não foi medida; uma linha plantada pedindo a remoção de um pacote explícito passa na validação e só a leitura do plano pelo dono a barra |
| P-05 | Modelo de ameaças e permissões | `SECURITY.md` (escopo MVP local); lacunas viraram P-07 e P-08 |
