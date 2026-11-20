# PKGBUILD do nodus (AUR)

Pacote `nodus`: compila a CLI (`nodusctl`) a partir da tag Git assinada. **Ainda não está publicado no AUR** (depende da P-02, busca de marca). Decisões: [ADR 0010](../../project/adr/0010-distribuicao-e-integridade.md) e [ADR 0012](../../project/adr/0012-verificacao-da-tag-no-aur.md).

## Como a verificação funciona
O `makepkg` só entende assinatura GPG (`?signed`), e a tag do nodus é assinada com SSH. Por isso o `prepare()` roda `git verify-tag` com o `allowed_signers` desta pasta (só a chave pública, a mesma do `README.md` do repo) e **falha o build se o código de saída não for 0**. O `allowed_signers` entra como `source` com `sha256sums`.

## Leia antes de instalar
1. **`makepkg --noprepare` desliga a verificação** (e `--mflags=--noprepare` nos helpers `yay` e `paru`). Não use.
2. `makepkg --verifysource` sozinho não verifica a assinatura.
3. A raiz de confiança é este PKGBUILD: ele carrega a chave pública e o `sha256sums` dela. A tag assinada protege contra código adulterado no GitHub, **não** contra um PKGBUILD malicioso no AUR, que não é revisado. Leia o PKGBUILD antes de construir e confira a chave por um segundo canal: `https://api.github.com/users/masterlxz/ssh_signing_keys` (fingerprint `SHA256:KNurUVWF0KhmHv42A3OYw6spZenOE0DrQCZ0jjNFnxI`).
4. O pacote instala só a CLI. A GUI desktop não faz parte dele.

## Atualizar a versão
Trocar `pkgver`, `pkgrel=1`, e conferir `sha256sums` (o do `allowed_signers` só muda se a chave mudar). Depois rodar `bash project/spikes/pkgbuild/verificar.sh` na raiz do repo.
