# 0010: Distribuição por código-fonte; integridade por tag assinada; binário só com ADR novo (P-08)

**Data:** 2026-11-14

## Contexto
O `SECURITY.md` (T9, §7) deixou como lacuna a assinatura, a verificação de integridade e a proteção contra downgrade do binário do nodus (SPEC §10.1), a decidir antes da primeira versão distribuída. Hoje não existe canal de distribuição: nenhum workflow de release, nenhuma tag, nenhum binário. O MVP (critério 1) diz só "instalar a CLI em um Arch Linux". O git local do dono não assina nada (verificado: `commit.gpgsign` e `tag.gpgsign` vazios).

O nodus roda comandos privilegiados (`sudo pacman`, ADR 0007 e `SECURITY.md` §4) e vai ler e escrever configuração do usuário. Um binário adulterado, ou rebaixado para uma versão vulnerável, tem o mesmo poder que o usuário que o roda. Por isso a decisão vem antes do primeiro canal.

## Decisão
1. **Primeiro canal: código-fonte.** O usuário compila na própria máquina: PKGBUILD no AUR (alvo) ou `cargo install --locked --git <repo> --tag <tag>`. O projeto **não distribui binário**.
2. **Integridade no canal fonte:**
   - toda versão é uma **tag Git assinada** pelo dono (assinatura SSH, `gpg.format=ssh`), verificável com `git tag -v` e exibida pelo GitHub;
   - o PKGBUILD fixa o `sha256sums` do tarball da tag;
   - `Cargo.lock` versionado e instalação com `--locked`, para compilar exatamente as dependências que passaram pelo `cargo-deny` e pelo CI (P-11);
   - build reprodutível **não** é prometido.
3. **Downgrade:** o nodus **não tem `self-update`**. Instalar e atualizar é do pacman/AUR helper/cargo, que já controlam versão. O projeto não implementa proteção própria contra downgrade enquanto não houver atualizador próprio.
4. **Binário distribuído** (GitHub Releases, instalador, agente que se atualiza): **proibido sem um ADR novo** que defina formato de assinatura (Sigstore ou minisign, a comparar), custódia e rotação de chaves, revogação e proteção contra downgrade. Código de atualização é classe do `AGENTS.md` §4 e exige confirmação explícita.
5. **Base de confiança:** ruleset `protect-main` + CI verde + tag assinada pelo dono. Quem não confia no dono não deve instalar; o nodus não cria um segundo ponto de confiança.

## Alternativas descartadas
- **Binário no GitHub Releases já, com Sigstore/cosign ou minisign:** obriga a desenhar chaves, verificação no cliente e `self-update` antes de o MVP existir. É criptografia e atualização (§4) sem dor concreta (YAGNI). Volta com o ADR do item 4.
- **Só "não assinamos nada":** deixa o usuário sem como detectar tag ou tarball adulterado; a tag assinada custa quase nada.
- **Assinar todo commit:** exige a chave em toda máquina de trabalho e não acrescenta nada à verificação que o usuário de fonte faz (ele verifica a tag).
- **`cargo install` do crates.io:** o nome `nodus` pode ter homônimo (P-02); publicar no crates.io fica para depois de P-02 e P-12.

## Consequências
- O MVP continua sem código novo de segurança; T9 muda de "lacuna" para "decidido, com condição".
- Antes da **primeira tag**: escolher e configurar a chave de assinatura (P-13), testar `git tag -v` numa máquina limpa e publicar a chave pública (por exemplo no README e no perfil do dono).
- PKGBUILD no AUR é um trabalho à parte (P-14), depois que o MVP fechar; o AUR não é revisado e quem instala executa o `PKGBUILD` do mantenedor.
- Habilita: qualquer pessoa verifica a origem de uma versão com um comando. Custa: compilar no PC (Rust, minutos) e depender da custódia da chave do dono.

## Não verificado
- Não testei como o AUR helper, o `makepkg` ou o `cargo install --tag` verificam (ou não) a assinatura da tag: o `makepkg` checa o `sha256sums`, e a verificação da assinatura exige `validpgpkeys` ou fonte `git+...#tag=...?signed` conforme o caso. **Conferir na documentação do `makepkg` e do PKGBUILD antes do P-14.**
- Assinatura SSH de tags no GitHub depende da chave estar cadastrada como "signing key" na conta. Conferido em 2026-11-14 (P-13): a chave está cadastrada e listada pela API pública. **Não** conferi o selo "Verified" numa tag real, porque nenhuma tag foi criada.
- Este ADR foi escrito por IA e revisado só pelo dono; nenhuma revisão externa.
