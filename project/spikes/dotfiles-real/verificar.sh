#!/usr/bin/env bash
# Verificação de `dotfiles adopt`, `link` e `history undo` em DOTFILES REAIS do HOME (critério 11).
#
# Rode num terminal de verdade (não com `!`): o nodusctl pede `s` e recusa sem terminal.
#   NODUSCTL=./target/release/nodusctl bash project/spikes/dotfiles-real/verificar.sh
#
# O que toca no HOME real: só `~/.bash_logout` e `~/.gitconfig` (e um `.bash_logout.pre` temporário).
# Config e histórico ficam em pastas temporárias (XDG_*), a pasta-fonte também; nada disso mexe no
# seu config.toml nem no history.jsonl reais. Antes de qualquer passo, os dois arquivos são copiados
# para o backup. Se algo travar, restaure com:  cp -a "$BK/.bash_logout" "$BK/.gitconfig" ~/
set -u

BIN=${NODUSCTL:-./target/release/nodusctl}
BIN=$(readlink -f "$BIN") || exit 2
[ -x "$BIN" ] || { echo "binário não encontrado: $BIN (build no CI ou 'cargo build --release' na branch)"; exit 2; }
[ -t 0 ] && [ -t 1 ] || { echo "precisa de terminal interativo"; exit 2; }
[ "$(id -u)" -ne 0 ] || { echo "não rode como root"; exit 2; }
[[ "$HOME" = /* ]] || exit 2

T=$(mktemp -d /tmp/nodus-real.XXXXXX)
BK=$T/backup; SRC=$T/fonte; mkdir -p "$BK" "$SRC" "$T/config/nodus" "$T/state"
export XDG_CONFIG_HOME=$T/config XDG_STATE_HOME=$T/state
printf 'version = 1\ndotfiles_dir = "%s"\n' "$SRC" > "$T/config/nodus/config.toml"
chmod 700 "$T/config/nodus"; chmod 600 "$T/config/nodus/config.toml"

PASS=0; FAIL=0
ok()   { PASS=$((PASS+1)); echo "  [OK]   $1"; }
bad()  { FAIL=$((FAIL+1)); echo "  [FALHA] $1"; }
chk()  { local d=$1; shift; if "$@" >/dev/null 2>&1; then ok "$d"; else bad "$d"; fi; }
step() { echo; echo "=== $1"; }
pause(){ read -r -p "--- [enter] para seguir " _; }
nodus(){ echo "\$ nodusctl $*"; "$BIN" "$@"; echo "  (exit $?)"; }

echo "binário: $BIN ($(stat -c %y "$BIN" | cut -c1-16))"; "$BIN" --version
echo "temporários: $T"

step "0. Pré-condições e backup"
for f in .bash_logout .gitconfig; do
  [ -f "$HOME/$f" ] && [ ! -L "$HOME/$f" ] || { echo "~/$f não é arquivo comum; abortando"; exit 2; }
  cp -a "$HOME/$f" "$BK/$f"
done
chk "~/.bash_logout.pre não existe (será usado no passo 5)" test ! -e "$HOME/.bash_logout.pre"
git config --global --list > "$T/gitconfig.antes" 2>&1
echo "backup em $BK"; pause

step "1. Relatório (só leitura)"
nodus dotfiles
pause

step "2. adopt ~/.bash_logout (responda s)"
nodus dotfiles adopt .bash_logout
chk "~/.bash_logout virou symlink" test -L "$HOME/.bash_logout"
chk "aponta para a pasta-fonte" test "$(readlink "$HOME/.bash_logout")" = "$(readlink -f "$SRC")/.bash_logout"
chk "fonte idêntica ao backup" cmp "$SRC/.bash_logout" "$BK/.bash_logout"
chk "permissão da fonte igual à do original" test "$(stat -c %a "$SRC/.bash_logout")" = "$(stat -c %a "$BK/.bash_logout")"
chk "sem sobra '.bash_logout.nodus-*' no HOME" bash -c '! ls "$HOME"/.bash_logout.nodus-* 2>/dev/null | grep -q .'
chk "bash lê o arquivo pelo symlink" bash -c 'cat "$HOME/.bash_logout" >/dev/null'
pause

step "3. adopt ~/.gitconfig (responda s) e git por cima do symlink"
nodus dotfiles adopt .gitconfig
chk "~/.gitconfig virou symlink" test -L "$HOME/.gitconfig"
git config --global --list > "$T/gitconfig.depois" 2>&1
chk "git config --global --list igual ao de antes" cmp "$T/gitconfig.antes" "$T/gitconfig.depois"
echo "-- escrita pelo git (informativo: o git pode trocar o symlink por arquivo ao gravar)"
git config --global nodus.teste ok
if [ -L "$HOME/.gitconfig" ]; then ok "após 'git config --global' escrever, ainda é symlink"; else bad "git substituiu o symlink por arquivo comum (achado!)"; fi
grep -q 'teste = ok' "$SRC/.gitconfig" && ok "a escrita chegou na fonte" || echo "  [INFO] a escrita NÃO chegou na fonte"
git config --global --unset nodus.teste
echo "-- diff fonte x backup (pode sobrar seção '[nodus]' vazia: comportamento do git, não do nodus)"
diff "$BK/.gitconfig" "$SRC/.gitconfig" && echo "  (idênticos)"
pause

step "4. history e undo (um 's' por undo)"
nodus history
nodus history undo 2
chk "~/.gitconfig voltou a ser arquivo comum" bash -c '[ -f "$HOME/.gitconfig" ] && [ ! -L "$HOME/.gitconfig" ]'
chk "fonte do .gitconfig continua lá" test -f "$SRC/.gitconfig"
git config --global --list > "$T/gitconfig.undo" 2>&1
chk "git config --global --list igual ao de antes" cmp "$T/gitconfig.antes" "$T/gitconfig.undo"
chk ".gitconfig igual ao backup byte a byte" cmp "$HOME/.gitconfig" "$BK/.gitconfig"
nodus history undo 1
chk "~/.bash_logout voltou a ser arquivo comum" bash -c '[ -f "$HOME/.bash_logout" ] && [ ! -L "$HOME/.bash_logout" ]'
chk ".bash_logout igual ao backup byte a byte" cmp "$HOME/.bash_logout" "$BK/.bash_logout"
echo "-- segundo undo da mesma entrada deve recusar"
nodus history undo 1
nodus history
pause

step "5. link (máquina nova simulada): destino existente recusa; sem destino, liga (responda s)"
echo "-- 5a. destino existente (arquivo comum): deve RECUSAR"
nodus dotfiles link .bash_logout
chk "~/.bash_logout continua arquivo comum, igual ao backup" cmp "$HOME/.bash_logout" "$BK/.bash_logout"
echo "-- 5b. tirando o original do caminho (mv para .bash_logout.pre) e ligando a fonte"
mv "$HOME/.bash_logout" "$HOME/.bash_logout.pre"
nodus dotfiles link .bash_logout
chk "~/.bash_logout é symlink para a fonte" test "$(readlink "$HOME/.bash_logout")" = "$(readlink -f "$SRC")/.bash_logout"
chk "conteúdo lido pelo link = backup" cmp "$HOME/.bash_logout" "$BK/.bash_logout"
nodus history
echo "-- 5c. undo do link (responda s): remove só o symlink; a fonte fica"
LN=$(grep -c . "$T/state/nodus/history.jsonl")
nodus history undo "$LN"
chk "symlink removido (nada em ~/.bash_logout)" test ! -e "$HOME/.bash_logout" -a ! -L "$HOME/.bash_logout"
chk "fonte intacta" cmp "$SRC/.bash_logout" "$BK/.bash_logout"
mv "$HOME/.bash_logout.pre" "$HOME/.bash_logout"
chk "original restaurado, igual ao backup" cmp "$HOME/.bash_logout" "$BK/.bash_logout"
pause

step "6. Estado final"
for f in .bash_logout .gitconfig; do
  chk "~/$f é arquivo comum e igual ao backup" bash -c "[ -f \"\$HOME/$f\" ] && [ ! -L \"\$HOME/$f\" ] && cmp \"\$HOME/$f\" \"$BK/$f\""
done
chk "sem sobras '.nodus-*' no HOME" bash -c '! ls "$HOME"/.*.nodus-* 2>/dev/null | grep -q .'
echo
echo "Resultado: $PASS OK, $FAIL FALHA"
echo "Temporários (config, histórico, fonte, backup) em: $T   (apague com: rm -rf $T)"
echo
echo "Manual, não coberto pelo script: abrir um arquivo adotado no editor e salvar (o editor"
echo "troca o symlink por arquivo?); logout real executando o .bash_logout; KDE. Anote o que viu."
[ "$FAIL" -eq 0 ]
