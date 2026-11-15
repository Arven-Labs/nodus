# 0012: Verificação da tag no AUR pelo `prepare()` do PKGBUILD (P-14)

**Data:** 2026-11-14

## Contexto
O ADR 0010 decidiu distribuir só código-fonte, com tag Git assinada por SSH (P-13), e deixou como "não verificado" como o `makepkg` verifica essa assinatura. Foi medido em 2026-11-14 (`makepkg` 7.1.0, git 2.56.0, gpg 2.4.9, PKGBUILD de teste com clone `git+file://` e chaves descartáveis; detalhes na P-14 e no `SESSIONS.md`):

- O `?signed` nativo roda `git verify-tag --raw` e só enxerga linhas de status do GnuPG, comparando com `validpgpkeys`. Tag assinada com GPG (controle): `Passed`. Tag assinada com SSH, que o `git verify-tag` aceita: `SIGNATURE NOT FOUND`, exit 1.
- Verificar no `prepare()` com `git verify-tag` e um `allowed_signers` como `source` funciona: tag da chave confiada passa (exit 0); tag de outra chave (`No principal matched`) e tag sem assinatura (`no signature found`) falham com exit 4.

O AUR não é revisado: quem instala executa o `PKGBUILD` do mantenedor (ADR 0010).

## Decisão
1. O PKGBUILD do nodus verifica a tag no **`prepare()`**: `git -c gpg.format=ssh -c gpg.ssh.allowedSignersFile=<allowed_signers> verify-tag <tag>`, falhando o build se o código de saída não for 0.
2. O `allowed_signers` (só a chave pública, a mesma do `README.md`) vai como `source` do PKGBUILD, com `sha256sums`.
3. O `prepare()` confere também que o `HEAD` do checkout é o commit apontado pela tag.
4. O README do pacote avisa que `makepkg --noprepare` desliga a verificação.
5. A chave de assinatura continua a da P-13 (SSH). Nenhuma chave GPG é criada.
6. O PKGBUILD real só é escrito depois que o MVP fechar (P-14), e depende da P-02 (nome) e da P-12 (licença). Antes de publicar, repetir a medição contra o GitHub e com um AUR helper.

## Alternativas
- **Tag assinada com GPG e `?signed` + `validpgpkeys`:** usa o mecanismo nativo e não há flag que pule a checagem. Descartada: obriga a refazer a P-13 com chave GPG nova (cadastro, publicação, backup, custódia) para ganhar só a proteção contra `--noprepare`. Volta se "o usuário não pode desligar a checagem" virar requisito.
- **Sem verificação no `makepkg`** (`sha256sums` do tarball ou `SKIP`; o usuário confere com `git tag -v`): descartada, a assinatura deixaria de ser exigida por qualquer passo e viraria convenção.
- **`?signed` com a chave SSH:** não funciona (medido).

## Consequências
- Mantém a chave, o backup e o `README.md` da P-13 sem mudança.
- Três fraquezas aceitas: (a) `makepkg --noprepare` pula o `prepare()` e, com ele, a verificação (o `?signed` nativo não tem esse furo); (b) `makepkg --verifysource` sozinho não verifica; (c) a raiz de confiança passa a ser o PKGBUILD no AUR, que carrega a chave pública e o `sha256sums` dela, e o AUR não é revisado. A tag assinada protege contra adulteração do código no GitHub, não contra um PKGBUILD malicioso.
- O código de verificação é nosso, dentro do PKGBUILD, e precisa de revisão e de teste a cada mudança.
- Atualiza o item 2 do ADR 0010 (que falava em `sha256sums` do tarball da tag); o resto daquele ADR continua valendo.

## Não verificado
- Contra o GitHub (a medição usou clone local) e em outras versões do `makepkg`.
- **AUR helper, medido em 2026-11-15** (`yay` 13.1.0 e `paru` 2.2.0, `-B <pasta>` sem instalar, PKGBUILD de teste com a verificação do item 1, chaves descartáveis, `HOME` isolado): os dois rodam o `prepare()`. Tag da chave confiada: pacote gerado, exit 0. Tag de outra chave: o `prepare()` falha com exit 4 e nenhum pacote é gerado, com o mesmo resultado do `makepkg` direto. Com `--mflags=--noprepare` os dois geram o pacote da tag de outra chave sem verificar, o que confirma a fraqueza (a) e mostra que ela atinge quem usa helper. Lacunas: só `-B` em pasta local (não o fluxo `-S` a partir do AUR, que clona e mostra o diff do PKGBUILD); o `yay` imprimiu `Skipping verification of source file PGP signatures` (não investiguei se ele pula a verificação PGP nativa, o que importaria só para o `?signed`, que esta decisão não usa); a configuração real do dono (`~/.config/yay`, `paru.conf`) não foi usada.
- O caso "o checkout aponta para outro commit que não o da tag": não foi montado, porque o `makepkg` faz checkout da própria tag; a conferência do item 3 não foi exercitada com um desvio.
- Este ADR foi escrito por IA e revisado só pelo dono; nenhuma revisão externa.
