#!/usr/bin/env bash
# Testa o PKGBUILD de packaging/aur/ num container Arch descartável (P-14, ADR 0012).
# Rode num terminal real, da raiz do repo nodus:
#   bash project/spikes/pkgbuild/verificar.sh
#
# Casos:
#   1. tag real, chave certa: o makepkg gera o pacote e `nodusctl --version` bate com o pkgver.
#   2. controle negativo: allowed_signers com OUTRA chave -> o prepare() falha e nenhum pacote sai.
#   3. fraqueza (a) do ADR 0012: com --noprepare e a chave errada o makepkg NÃO verifica (documentada).
#   4. shellcheck e namcap no PKGBUILD.
#
# O container não tem a sua config do git nem chave privada. A compilação roda UMA vez (caso 1), por
# exceção ao ADR 0009 concedida pelo dono (mesmo molde de project/spikes/tag-limpa/verificar.sh).
# Baixa a imagem `archlinux`, pacotes do pacman e as dependências do crates.io.
set -u
DIR=$(cd "$(dirname "$0")/../../.." && pwd)/packaging/aur
[ -f "$DIR/PKGBUILD" ] || { echo "falta $DIR/PKGBUILD"; exit 2; }
command -v docker >/dev/null || { echo "docker não encontrado"; exit 2; }

# --network host: mesmo motivo do tag-limpa/verificar.sh (rede em ponte do Docker falhou em 2026-11-17).
docker run --rm -i --network host -v "$DIR":/aur:ro archlinux:latest bash -s <<'CONTAINER'
set -u
PASS=0; FAIL=0
ok()  { PASS=$((PASS+1)); echo "  [OK]    $1"; }
bad() { FAIL=$((FAIL+1)); echo "  [FALHA] $1"; }
step(){ echo; echo "=== $1"; }

step "0. Preparar o container (base-devel, git, rust, openssh, namcap, shellcheck) e o usuário builder"
pacman -Sy --noconfirm --needed base-devel git rust openssh namcap shellcheck pacman-contrib >/tmp/pacman.log 2>&1 \
  && ok "pacotes instalados" || { bad "pacman falhou"; tail -5 /tmp/pacman.log; exit 1; }
useradd -m builder
install -d -o builder -g builder /home/builder/ok /home/builder/errada
cp /aur/PKGBUILD /aur/allowed_signers /home/builder/ok/
cp /aur/PKGBUILD /home/builder/errada/
ssh-keygen -q -t ed25519 -N '' -f /tmp/outra -C controle
echo "nodus $(cut -d' ' -f1,2 /tmp/outra.pub)" > /home/builder/errada/allowed_signers
chown -R builder:builder /home/builder

step "1. Tag real, chave certa: makepkg gera o pacote"
su builder -c "cd /home/builder/ok && makepkg -f --noconfirm" >/tmp/ok.log 2>&1
RC=$?
PKG=$(ls /home/builder/ok/*.pkg.tar.zst 2>/dev/null | head -1)
[ $RC -eq 0 ] && [ -n "$PKG" ] && ok "makepkg exit 0 e pacote $(basename "$PKG")" || { bad "makepkg exit $RC, pacote '${PKG:-nenhum}'"; tail -15 /tmp/ok.log; }
if [ -n "$PKG" ]; then
  pacman -U --noconfirm "$PKG" >/dev/null 2>&1
  WANT=$(grep -m1 '^pkgver=' /aur/PKGBUILD | cut -d= -f2)
  GOT=$(nodusctl --version 2>&1)
  [[ "$GOT" == *"$WANT"* ]] && ok "nodusctl --version: $GOT" || bad "nodusctl --version deu '$GOT', esperado conter $WANT"
fi

step "2. Controle negativo: allowed_signers com outra chave deve barrar no prepare()"
(cd /home/builder/errada && su builder -c "updpkgsums" >/dev/null 2>&1)
su builder -c "cd /home/builder/errada && makepkg -f --noconfirm" >/tmp/errada.log 2>&1
RC=$?
NPKG=$(ls /home/builder/errada/*.pkg.tar.zst 2>/dev/null | wc -l)
[ $RC -ne 0 ] && [ "$NPKG" -eq 0 ] && grep -q 'No principal matched' /tmp/errada.log \
  && ok "chave errada: makepkg exit $RC, sem pacote, 'No principal matched'" \
  || { bad "chave errada: exit $RC, pacotes $NPKG"; tail -15 /tmp/errada.log; }

step "3. Fraqueza (a): --noprepare com a chave errada NÃO verifica (esperado passar; está documentado)"
rm -rf /home/builder/errada/src
su builder -c "cd /home/builder/errada && makepkg -f --noconfirm --noprepare --nobuild" >/tmp/noprep.log 2>&1
RC=$?
[ $RC -eq 0 ] && ok "--noprepare --nobuild exit 0: a verificação foi pulada (confirma a fraqueza a)" \
  || { bad "--noprepare deu exit $RC (esperado 0)"; tail -10 /tmp/noprep.log; }

step "4. shellcheck e namcap no PKGBUILD"
# SC2034: variáveis do PKGBUILD parecem não usadas; SC2154: srcdir/pkgdir/pkgver vêm do makepkg;
# SC2164: o makepkg roda as funções com errexit, então `cd` que falha já aborta (convenção dos PKGBUILDs).
shellcheck -s bash -e SC2034,SC2154,SC2164 /aur/PKGBUILD && ok "shellcheck sem achados" || bad "shellcheck achou problemas"
su builder -c "namcap /home/builder/ok/PKGBUILD" >/tmp/namcap.log 2>&1
if grep -q ' E: ' /tmp/namcap.log; then bad "namcap com erros"; cat /tmp/namcap.log; else ok "namcap sem erros (E:)"; cat /tmp/namcap.log; fi

echo; echo "Resultado: $PASS OK, $FAIL falha(s)"
[ $FAIL -eq 0 ]
CONTAINER
