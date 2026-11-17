#!/usr/bin/env bash
# Verifica uma tag assinada do nodus numa máquina LIMPA (container Arch descartável).
# ADR 0010, critério 1 do MVP. Rode num terminal real:
#   bash project/spikes/tag-limpa/verificar.sh v0.1.0
#
# O container não tem o seu ~/.gitconfig, ssh-agent nem chave privada: tudo o que passar vem só do
# que está publicado (tag, API do GitHub, README). `--rm` apaga o container ao fim. Baixa a imagem
# `archlinux`, pacotes do pacman e as dependências do crates.io (uns 1 a 2 GB em /home, o Docker daqui
# fica em ~/.docker-data). A compilação roda UMA vez, aqui, por escolha do dono (exceção ao ADR 0009).
set -u
TAG=${1:?uso: verificar.sh <tag>, ex.: v0.1.0}
[[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "tag inesperada: $TAG"; exit 2; }
command -v docker >/dev/null || { echo "docker não encontrado"; exit 2; }

docker run --rm -i -e TAG="$TAG" archlinux:latest bash -s <<'CONTAINER'
set -u
REPO=Arven-Labs/nodus
OWNER=masterlxz
PASS=0; FAIL=0
ok()  { PASS=$((PASS+1)); echo "  [OK]    $1"; }
bad() { FAIL=$((FAIL+1)); echo "  [FALHA] $1"; }
step(){ echo; echo "=== $1"; }

step "0. Preparar o container (pacman: git, rust, openssh, curl)"
pacman -Sy --noconfirm --needed git rust openssh curl >/tmp/pacman.log 2>&1 \
  && ok "pacotes instalados" || { bad "pacman falhou (ver abaixo)"; tail -5 /tmp/pacman.log; exit 1; }
echo "  sem ~/.gitconfig: $([ -e ~/.gitconfig ] && echo NAO || echo sim); sem chave privada: $(ls ~/.ssh 2>/dev/null | wc -l) arquivos em ~/.ssh"

step "1. Chave pública pelos DOIS canais (API do GitHub e README) e comparação"
API_KEY=$(curl -fsS "https://api.github.com/users/$OWNER/ssh_signing_keys" | grep -o '"key": *"[^"]*"' | head -1 | sed 's/.*"\(ssh-[^"]*\)"/\1/' | cut -d' ' -f1,2)
README_KEY=$(curl -fsS "https://raw.githubusercontent.com/$REPO/$TAG/README.md" | grep -m1 -o 'ssh-ed25519 [A-Za-z0-9+/=]*')
echo "  API:    ${API_KEY:0:40}..."
echo "  README: ${README_KEY:0:40}..."
[ -n "$API_KEY" ] && [ "$API_KEY" = "$README_KEY" ] && ok "as duas chaves são idênticas" || bad "chaves diferentes ou vazias"

step "2. Clonar e verificar a tag ($TAG) com a chave do GitHub"
mkdir -p /work
git clone -q "https://github.com/$REPO.git" /work/nodus || { bad "git clone falhou"; exit 1; }
cd /work/nodus || exit 1
echo "nodus $API_KEY" > /work/allowed_signers
git -c gpg.format=ssh -c gpg.ssh.allowedSignersFile=/work/allowed_signers tag -v "$TAG"
RC=$?; [ $RC -eq 0 ] && ok "git tag -v com a chave do GitHub: exit 0" || bad "git tag -v deu exit $RC (esperado 0)"

step "3. Controle negativo: chave errada deve dar exit 1"
ssh-keygen -q -t ed25519 -N '' -f /work/outra -C controle
echo "nodus $(cut -d' ' -f1,2 /work/outra.pub)" > /work/errada
git -c gpg.format=ssh -c gpg.ssh.allowedSignersFile=/work/errada tag -v "$TAG" >/dev/null 2>&1
RC=$?; [ $RC -eq 1 ] && ok "chave errada: exit 1" || bad "chave errada deu exit $RC (esperado 1)"

step "4. A tag aponta para o commit com Cargo.toml na versão da tag"
WANT=${TAG#v}
HAVE=$(git show "$TAG^{commit}:Cargo.toml" | grep -m1 '^version' | cut -d'"' -f2)
[ "$HAVE" = "$WANT" ] && ok "Cargo.toml da tag diz $HAVE" || bad "Cargo.toml da tag diz '$HAVE', esperado $WANT"

step "5. Instalar do código-fonte, como o usuário faria (demora alguns minutos)"
cargo install --locked --git "https://github.com/$REPO" --tag "$TAG" nodus-cli >/work/install.log 2>&1
RC=$?; [ $RC -eq 0 ] && ok "instalação --locked --tag: exit 0" || { bad "instalação deu exit $RC"; tail -15 /work/install.log; }
export PATH="$HOME/.cargo/bin:$PATH"
OUT=$(nodusctl --version 2>&1)
[ "$OUT" = "nodusctl $WANT" ] && ok "nodusctl --version = '$OUT'" || bad "nodusctl --version = '$OUT' (esperado 'nodusctl $WANT')"

step "6. Fumaça (informativo): o binário roda num Arch limpo"
nodusctl inventory 2>&1 | head -5
echo "  (exit ${PIPESTATUS[0]})"

echo; echo "Resultado: $PASS OK, $FAIL FALHA"
[ "$FAIL" -eq 0 ]
CONTAINER
RC=$?
echo "container terminou com exit $RC (o container foi removido)"
exit $RC
