#!/usr/bin/env bash
# Spike P-09: o `git` CLI isolado executa código vindo de um repositório hostil?
#
# Para cada vetor, cria um repositório com uma config ou hook "maliciosos" cujo único efeito é criar
# um arquivo-canário, e roda o mesmo comando duas vezes:
#   - base: git sem isolamento (o canário DEVE aparecer; senão o vetor não vale nada nesta versão);
#   - iso:  git com o isolamento candidato (o canário NÃO deve aparecer).
#
# Tudo acontece dentro de um diretório criado por `mktemp -d` (use TMPDIR para escolher onde). Nada
# escreve fora dele, nada usa rede (o fetch é contra um remoto local), e o HOME é um diretório vazio
# para a config do usuário não interferir. O diretório é apagado no fim.
#
# Uso: project/spikes/p09-git/medir.sh
# Saída: uma tabela VETOR | COMANDO | BASE | ISO e um resumo. Código de saída 1 se algum vetor
# disparou sob o isolamento ou se algum baseline não disparou (medição inválida).

set -u

WORK="$(mktemp -d "${TMPDIR:-/tmp}/nodus-p09.XXXXXX")" || exit 2
case "$WORK" in /*/nodus-p09.*) ;; *) echo "diretório de trabalho inesperado: $WORK" >&2; exit 2 ;; esac
trap 'rm -rf -- "$WORK"' EXIT

CANARY="$WORK/canario"
HOMEDIR="$WORK/home"
ATK="$WORK/atacante.sh"
mkdir -p "$CANARY" "$HOMEDIR"

# O "ataque": cria um arquivo com o nome do vetor. É o único efeito possível.
printf '#!/bin/sh\n: > "%s/$1"\nexit 0\n' "$CANARY" > "$ATK"
chmod +x "$ATK"

# Ambiente neutro: sem config de sistema nem do usuário, sem perguntas no terminal.
# É um array (e não uma função) para poder ser chamado por `timeout`, que só executa programas.
NEUTRO=(env HOME="$HOMEDIR" XDG_CONFIG_HOME="$HOMEDIR" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
        GIT_TERMINAL_PROMPT=0)
G() { "${NEUTRO[@]}" git -c user.name=t -c user.email=t@t -c init.defaultBranch=main "$@"; }

# Repositório modelo + remoto local com um commit novo (para o fetch ter o que buscar).
G init -q "$WORK/modelo" && echo um > "$WORK/modelo/a.txt" && G -C "$WORK/modelo" add a.txt \
  && G -C "$WORK/modelo" commit -q -m um
G clone -q --bare "$WORK/modelo" "$WORK/remoto.git"
G clone -q "$WORK/remoto.git" "$WORK/pusher" && echo dois > "$WORK/pusher/b.txt" \
  && G -C "$WORK/pusher" add b.txt && G -C "$WORK/pusher" commit -q -m dois && G -C "$WORK/pusher" push -q origin main
G -C "$WORK/modelo" remote add origin "$WORK/remoto.git"
G -C "$WORK/modelo" fetch -q origin 2>/dev/null
G -C "$WORK/modelo" fetch -q origin 2>/dev/null
# Volta o ref remoto um commit para trás: o fetch dos testes tem de novo o que atualizar.
G -C "$WORK/modelo" update-ref refs/remotes/origin/main "$(G -C "$WORK/modelo" rev-parse HEAD)"

# Isolamento candidato (a confirmar por esta medição).
iso_env=(GIT_ALLOW_PROTOCOL=file:https:ssh)
iso_c=(-c core.fsmonitor=false -c core.hooksPath=/dev/null -c core.sshCommand=ssh -c core.gitProxy=
       -c credential.helper= -c protocol.ext.allow=never)

# run_git MODO COMANDO REPO [VARIÁVEIS EXTRAS...]  -> imprime a saída em $WORK/saida
run_git() {
  local modo="$1" cmd="$2" repo="$3"; shift 3
  local extra=("$@") args
  local anula=() lotes=()
  case "$cmd:$modo" in
    fetch:base) args=(fetch origin) ;;
    # Receita do marco 6: sem submódulos e sem manutenção automática (gc/hooks) no fetch.
    fetch:iso)  args=(fetch --no-recurse-submodules --no-auto-maintenance --upload-pack=git-upload-pack origin) ;;
    status:*)   args=(status --porcelain) ;;
    # Os três comandos de leitura do `nodus repos` (marco 6), todos só de refs e config.
    leitura:*)  lotes=("for-each-ref --format=%(HEAD)%09%(refname:short)%09%(objectname:short)%09%(upstream:short)%09%(upstream:track,nobracket)"
                       "rev-parse --short HEAD" "config --local --get remote.origin.url") ;;
    refs:*)     args=(for-each-ref) ;;   # só lê refs: não olha a árvore de trabalho
    status_anula:base) args=(status --porcelain) ;;
    status_anula:iso)
      args=(status --porcelain)
      # Lê (sem executar) os nomes de filtro da config do repo e anula clean/smudge/process de cada um.
      local chave
      while IFS= read -r chave; do
        case "$chave" in filter.*.clean|filter.*.smudge|filter.*.process) anula+=(-c "$chave=") ;; esac
      done < <("${NEUTRO[@]}" git -C "$repo" config --local --name-only --list 2>/dev/null)
      ;;
  esac
  [ "${#lotes[@]}" -eq 0 ] && lotes=("${args[*]}")   # nenhum argumento tem espaço
  : > "$WORK/saida"
  local lote a
  for lote in "${lotes[@]}"; do
    read -ra a <<<"$lote"
    if [ "$modo" = iso ]; then
      timeout 20 "${NEUTRO[@]}" "${iso_env[@]}" "${extra[@]}" git "${iso_c[@]}" "${anula[@]}" -C "$repo" "${a[@]}" >>"$WORK/saida" 2>&1
    else
      timeout 20 "${NEUTRO[@]}" "${extra[@]}" git -C "$repo" "${a[@]}" >>"$WORK/saida" 2>&1
    fi
  done
}

TOTAL=0; FALHAS=0; INVALIDOS=0; TABELA=""

# medir NOME COMANDO FUNÇÃO_DE_PREPARO [VARIÁVEIS EXTRAS...]
medir() {
  local nome="$1" cmd="$2" prepara="$3"; shift 3
  local res_base res_iso modo repo="$WORK/r"
  for modo in base iso; do
    rm -rf -- "$repo" && cp -a "$WORK/modelo" "$repo"
    "$prepara" "$repo"
    rm -f -- "$CANARY"/*   # só vale o que dispara no comando medido, não no preparo
    run_git "$modo" "$cmd" "$repo" "$@"
    local r=limpo
    if [ -e "$CANARY/${CANARIO:-$nome}" ]; then r=DISPARA
    elif grep -qi 'dubious ownership' "$WORK/saida"; then r=RECUSADO; fi
    [ "$modo" = base ] && res_base=$r || res_iso=$r
  done
  TOTAL=$((TOTAL+1))
  [ "$res_iso" = DISPARA ] && FALHAS=$((FALHAS+1))
  # ownership é recusa (não dispara); alt_* são alternativas cujo baseline não precisa disparar.
  case "$nome" in ownership|alt_*) ;; *) [ "$res_base" != DISPARA ] && INVALIDOS=$((INVALIDOS+1)) ;; esac
  TABELA+="$(printf '%-18s %-7s %-9s %-9s' "$nome" "$cmd" "$res_base" "$res_iso")"$'\n'
}

cfg() { G -C "$1" config "${@:2}"; }

p_fsmonitor()  { cfg "$1" core.fsmonitor "$ATK fsmonitor"; }
p_hook_reftx() { mkdir -p "$1/.git/hooks"; printf '#!/bin/sh\n"%s" hook_reftx\n' "$ATK" > "$1/.git/hooks/reference-transaction"; chmod +x "$1/.git/hooks/reference-transaction"; }
# O gc automático só roda com "objetos soltos demais" em objects/17 (amostra do Git); plantamos dois
# arquivos com nome de objeto para o disparo não depender do hash sorteado.
p_hook_autogc(){ mkdir -p "$1/.git/hooks" "$1/.git/objects/17"; printf '#!/bin/sh\n"%s" hook_autogc\n' "$ATK" > "$1/.git/hooks/pre-auto-gc"; chmod +x "$1/.git/hooks/pre-auto-gc"; cfg "$1" gc.auto 1; cfg "$1" gc.autoDetach false
                 : > "$1/.git/objects/17/00000000000000000000000000000000000001"; : > "$1/.git/objects/17/00000000000000000000000000000000000002"; }
p_hookspath()  { mkdir -p "$WORK/hooks-atacante"; printf '#!/bin/sh\n"%s" hookspath\n' "$ATK" > "$WORK/hooks-atacante/reference-transaction"; chmod +x "$WORK/hooks-atacante/reference-transaction"; cfg "$1" core.hooksPath "$WORK/hooks-atacante"; }
p_uploadpack() { cfg "$1" remote.origin.uploadpack "$ATK uploadpack"; }
p_sshcommand() { cfg "$1" remote.origin.url ssh://localhost/x; cfg "$1" core.sshCommand "$ATK sshcommand"; }
# core.gitProxy é executado como programa, sem shell e sem argumentos próprios: precisa de um executável.
p_gitproxy()   { printf '#!/bin/sh\n: > "%s/gitproxy"\nexit 1\n' "$CANARY" > "$WORK/proxy-atacante.sh"; chmod +x "$WORK/proxy-atacante.sh"
                 cfg "$1" remote.origin.url git://localhost/x; cfg "$1" core.gitProxy "$WORK/proxy-atacante.sh"; }
p_ext()        { cfg "$1" protocol.ext.allow always; cfg "$1" remote.origin.url "ext::$ATK ext"; }
p_include()    { printf '[core]\n\tfsmonitor = %s include\n' "$ATK" > "$WORK/incluido.cfg"; cfg "$1" include.path "$WORK/incluido.cfg"; }
p_filter()     { cfg "$1" filter.evil.clean "$ATK filter_clean"; echo '* filter=evil' > "$1/.gitattributes"; touch -d '2000-01-01' "$1/a.txt"; G -C "$1" update-index --refresh >/dev/null 2>&1; touch "$1/a.txt"; }

p_filter_clean() { p_filter "$@"; }   # nome do vetor na tabela

# Submódulo hostil: a config do repo manda o fetch recursar, e o submódulo tem seu próprio uploadpack.
p_submodule()  { cfg "$1" fetch.recurseSubmodules true
                 mkdir -p "$1/sub" && G init -q "$1/sub" && G -C "$1/sub" remote add origin "$WORK/remoto.git" \
                   && G -C "$1/sub" config remote.origin.uploadpack "$ATK submodule"
                 printf '[submodule "sub"]\n\tpath = sub\n\turl = %s\n' "$WORK/remoto.git" > "$1/.gitmodules"
                 G -C "$1" update-index --add --cacheinfo "160000,$(G -C "$WORK/modelo" rev-parse HEAD),sub" 2>/dev/null; }

medir fsmonitor    status p_fsmonitor
medir hook_reftx   fetch  p_hook_reftx
medir hook_autogc  fetch  p_hook_autogc
medir hookspath    fetch  p_hookspath
medir uploadpack   fetch  p_uploadpack
medir sshcommand   fetch  p_sshcommand
medir gitproxy     fetch  p_gitproxy
medir ext          fetch  p_ext
medir include      status p_include
medir filter_clean status p_filter
medir submodule    fetch  p_submodule
# Alternativas para o vetor sem chave fixa a anular (filtro com nome escolhido pelo atacante):
CANARIO=filter_clean
medir alt_filter_anula  status_anula p_filter   # (A) anular todo filter.*.{clean,smudge,process} encontrado
medir alt_filter_refs   refs         p_filter   # (B) não usar `status`: só ler refs
# Comandos de leitura do `nodus repos` contra todos os vetores (o baseline não precisa disparar: estes
# comandos não são gatilho; o que importa é que NADA dispare sob isolamento).
for v in fsmonitor hook_reftx hookspath uploadpack sshcommand gitproxy ext include filter_clean submodule; do
  CANARIO=$v
  medir "alt_leitura_$v" leitura "p_$v"
done
unset CANARIO
# Dono diferente: simulado com a variável de teste do próprio Git (não há como criar outro dono sem root).
medir ownership    status p_fsmonitor GIT_TEST_ASSUME_DIFFERENT_OWNER=1

printf '%-18s %-7s %-9s %-9s\n' VETOR COMANDO BASE ISO
printf '%s' "$TABELA"
printf '%-18s %-7s %s\n' credential - "NAO MEDIDO (exige servidor HTTP que peça credencial)"
echo
echo "git: $(git --version)"
echo "vetores medidos: $TOTAL | dispararam sob isolamento: $FALHAS | baseline que não disparou (medição inválida): $INVALIDOS"
[ "$FALHAS" -eq 0 ] && [ "$INVALIDOS" -eq 0 ]
