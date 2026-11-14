# nodus: Pendências

Decisões em aberto. Marque como resolvida citando o ADR.

## Em aberto

| ID | Pendência | Observação |
|---|---|---|
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
| P-15 | Semântica da reversão (fatia 4) | ADR 0011: `nodus history undo <n>`, `sudo pacman -Rs` (nunca `-n`, `-c`, `-d`, `--noconfirm`) depois de simular com `pacman -Rs --print`, só o que o nodus instalou e ainda é explícito, confirmação por operação, entrada nova no histórico (v2). Implementada na fatia 4 (PRs #24 e #25); `-s` com dependência medido em 2026-11-14. Verificado no Arch real em 2026-11-14 (instalou `abi-dumper`, `undo 1` removeu os 3 pacotes do plano). Lacunas: banco do pacman travado não medido; o histórico registra só o pedido (ver P-16); uma linha plantada pedindo a remoção de um pacote explícito passa na validação e só a leitura do plano pelo dono a barra |
| P-16 | O histórico registra só o pedido, não as dependências | ADR 0011, item 8: o histórico é um log de **intenção**, não de efeito; fica assim no MVP. Medido em 2026-11-14: `undo 1` removeu 3 pacotes e a entrada diz `removidos 1`. Lacunas: nenhuma opção de log de efeito (diff de `pacman -Q` antes e depois, formato v3) foi implementada; reabrir com ADR novo se o histórico passar a servir de auditoria (frotas) |
| P-13 | Chave de assinatura de tags | Chave ed25519 dedicada (`SHA256:KNurUVWF0KhmHv42A3OYw6spZenOE0DrQCZ0jjNFnxI`), com passphrase, cadastrada como signing key no GitHub em 2026-11-14 e publicada no `README.md`. `tag.gpgsign` e `gpg.format=ssh` só na config local do repo. Testado em repo descartável com `HOME` limpo: chave publicada dá `Good signature` e exit 0; chave errada, lista vazia e tag adulterada dão exit 1. Lacunas: não é uma máquina de verdade; nenhuma tag real foi criada; sem rotação nem revogação desenhadas; backup da chave privada é decisão do dono, sem registro; a chave foi gerada sem passphrase e a passphrase foi adicionada depois (`ssh-keygen -p`), antes de qualquer uso ou cadastro |
| P-05 | Modelo de ameaças e permissões | `SECURITY.md` (escopo MVP local); lacunas viraram P-07 e P-08 |
