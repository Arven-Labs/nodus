#!/usr/bin/env bash
# Spike P-20: dá para olhar a árvore de trabalho (`status`) e avançar (`merge --ff-only`) num
# repositório hostil sem executar código dele? (continuação do P-09, ADR 0009)
#
# Hipótese medida: DETECTAR E RECUSAR. Um filtro só executa se a config define
# `filter.<nome>.{clean,smudge,process}`. O "gate" lê TODA a config que o Git leria
# (`git config --list --includes --name-only`, que não executa nada) e recusa o repositório se
# aparecer qualquer chave de filtro. O resto (hooks, fsmonitor, gpg, ...) fica com o isolamento do
# ADR 0009 mais as opções extras do `merge`.
#
# Para cada vetor, cria um repositório com uma config ou hook "malicioso" cujo único efeito é criar
# um arquivo-canário, e roda o mesmo comando duas vezes:
#   - base: git sem isolamento e sem gate (o canário DEVE aparecer; senão o vetor não vale nada);
#   - iso:  git com o isolamento candidato e o gate (o canário NÃO deve aparecer).
# Resultado "RECUSADO" = o gate parou antes de rodar o comando (não disparou).
#
# Depois vêm os CONTROLES POSITIVOS: num repositório sem nada hostil o gate libera, o `status`
# classifica limpo e sujo, e o `merge --ff-only` avança (ou se recusa a divergir). Sem eles, a
# tabela "provaria" só que recusar tudo é seguro.
#
# Tudo acontece em `mktemp -d` (TMPDIR escolhe onde), sem rede (o remoto é local), com HOME vazio.
# O diretório é apagado no fim.
#
# Uso: project/spikes/p10-sync/medir.sh
#      SEM_GATE=1 project/spikes/p10-sync/medir.sh   # quebra de propósito: sem o gate, os vetores
#                                                    # de filtro têm de disparar sob o isolamento
# Saída: tabela VETOR | COMANDO | BASE | ISO, os controles e um resumo. Código de saída 1 se algum
# vetor disparou sob o isolamento, se algum baseline não disparou (medição inválida) ou se algum
# controle falhou.

set -u

WORK="$(mktemp -d "${TMPDIR:-/tmp}/nodus-p20.XXXXXX")" || exit 2
case "$WORK" in /*/nodus-p20.*) ;; *) echo "diretório de trabalho inesperado: $WORK" >&2; exit 2 ;; esac
trap 'rm -rf -- "$WORK"' EXIT

CANARY="$WORK/canario"
HOMEDIR="$WORK/home"
ATK="$WORK/atacante.sh"
mkdir -p "$CANARY" "$HOMEDIR"

# O "ataque": cria um arquivo com o nome do vetor. É o único efeito possível.
printf '#!/bin/sh\n: > "%s/$1"\nexit 0\n' "$CANARY" > "$ATK"
chmod +x "$ATK"

# Ambiente neutro: sem config de sistema nem do usuário, sem perguntas no terminal.
NEUTRO=(env HOME="$HOMEDIR" XDG_CONFIG_HOME="$HOMEDIR" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
        GIT_TERMINAL_PROMPT=0)
G() { "${NEUTRO[@]}" git -c user.name=t -c user.email=t@t -c init.defaultBranch=main "$@"; }

# Repositório modelo (commit 1: a.txt e um .gitattributes que manda todo arquivo para o filtro
# "evil", inofensivo enquanto a config não o define) com o remoto à frente (commit 2 muda a.txt e
# cria b.txt). Depois do fetch, `origin/main` está um commit à frente: há o que avançar.
G init -q "$WORK/modelo" && echo um > "$WORK/modelo/a.txt" && echo '* filter=evil' > "$WORK/modelo/.gitattributes" \
  && G -C "$WORK/modelo" add a.txt .gitattributes && G -C "$WORK/modelo" commit -q -m um
G clone -q --bare "$WORK/modelo" "$WORK/remoto.git"
G clone -q "$WORK/remoto.git" "$WORK/pusher" && echo dois > "$WORK/pusher/a.txt" && echo b > "$WORK/pusher/b.txt" \
  && G -C "$WORK/pusher" add a.txt b.txt && G -C "$WORK/pusher" commit -q -m dois
# O commit 2 leva um cabeçalho `gpgsig` falso: só com ele o `merge.verifySignatures` chama o
# `gpg.program` (sem assinatura o Git recusa antes de executar qualquer coisa).
c2="$(printf 'tree %s\nparent %s\nauthor t <t@t> 1700000000 +0000\ncommitter t <t@t> 1700000000 +0000\ngpgsig -----BEGIN PGP SIGNATURE-----\n \n fake\n -----END PGP SIGNATURE-----\n\ndois\n' \
  "$(G -C "$WORK/pusher" rev-parse 'HEAD^{tree}')" "$(G -C "$WORK/pusher" rev-parse 'HEAD^')" | G -C "$WORK/pusher" hash-object -t commit -w --stdin)"
G -C "$WORK/pusher" update-ref refs/heads/main "$c2" && G -C "$WORK/pusher" push -q origin main --force
G -C "$WORK/modelo" remote add origin "$WORK/remoto.git"
G -C "$WORK/modelo" fetch -q origin 2>/dev/null
G -C "$WORK/modelo" branch -q --set-upstream-to=origin/main main
if [ "$(G -C "$WORK/modelo" rev-list --count 'HEAD..@{u}')" != 1 ]; then
  echo "preparo inválido: origin/main deveria estar 1 commit à frente" >&2; exit 2
fi

# Isolamento candidato: o do ADR 0009 (copiado de crates/nodus-core/src/repos.rs) mais o que o
# `merge` pode tocar (submódulo e verificação de assinatura).
iso_env=(GIT_ALLOW_PROTOCOL=file:https:ssh)
iso_c=(-c core.fsmonitor=false -c core.hooksPath=/dev/null -c core.sshCommand=ssh -c core.gitProxy=
       -c credential.helper= -c protocol.ext.allow=never
       -c submodule.recurse=false -c merge.verifySignatures=false)

# O gate: 0 = pode rodar; 1 = recusa. Falha fechada: se nem a config der para ler, recusa.
gate() {
  local chaves
  chaves="$("${NEUTRO[@]}" "${iso_env[@]}" git "${iso_c[@]}" -C "$1" config --list --includes --name-only 2>/dev/null)" || return 1
  ! grep -Eiq '^filter\..+\.(clean|smudge|process)$' <<<"$chaves"
}

GATE_RECUSOU=0
# run_git MODO COMANDO REPO  -> saída em $WORK/saida; GATE_RECUSOU=1 se o gate barrou.
run_git() {
  local modo="$1" cmd="$2" repo="$3" args=()
  : > "$WORK/saida"; GATE_RECUSOU=0
  case "$cmd:$modo" in
    status:*)    args=(status --porcelain) ;;
    merge:base)  args=(merge --ff-only --no-edit '@{u}') ;;
    merge:iso)   args=(merge --ff-only --no-edit --no-verify-signatures '@{u}') ;;
  esac
  if [ "$modo" = iso ]; then
    if [ -z "${SEM_GATE:-}" ] && ! gate "$repo"; then GATE_RECUSOU=1; return 0; fi
    timeout 20 "${NEUTRO[@]}" "${iso_env[@]}" git "${iso_c[@]}" -C "$repo" "${args[@]}" >>"$WORK/saida" 2>&1
  else
    timeout 20 "${NEUTRO[@]}" git -C "$repo" "${args[@]}" >>"$WORK/saida" 2>&1
  fi
}

TOTAL=0; FALHAS=0; INVALIDOS=0; TABELA=""

# medir NOME COMANDO FUNÇÃO_DE_PREPARO   (NOME é também o nome do canário)
medir() {
  local nome="$1" cmd="$2" prepara="$3"
  local res_base res_iso modo repo="$WORK/r"
  for modo in base iso; do
    rm -rf -- "$repo" && cp -a "$WORK/modelo" "$repo"
    "$prepara" "$repo" "$nome"
    rm -f -- "$CANARY"/*   # só vale o que dispara no comando medido, não no preparo
    run_git "$modo" "$cmd" "$repo"
    local r=limpo
    if [ -e "$CANARY/$nome" ]; then r=DISPARA
    elif [ "$GATE_RECUSOU" = 1 ]; then r=RECUSADO; fi
    [ "$modo" = base ] && res_base=$r || res_iso=$r
  done
  TOTAL=$((TOTAL+1))
  [ "$res_iso" = DISPARA ] && FALHAS=$((FALHAS+1))
  [ "$res_base" != DISPARA ] && INVALIDOS=$((INVALIDOS+1))
  TABELA+="$(printf '%-26s %-7s %-9s %-9s' "$nome" "$cmd" "$res_base" "$res_iso")"$'\n'
}

cfg() { G -C "$1" config "${@:2}"; }
# Executável próprio que cria o canário NOME (para comandos que o Git chama com argumentos seus).
atk_proprio() { printf '#!/bin/sh\n: > "%s/%s"\nexit 0\n' "$CANARY" "$1" > "$WORK/atk-$1.sh"; chmod +x "$WORK/atk-$1.sh"; echo "$WORK/atk-$1.sh"; }
# Faz o `status` precisar reler o conteúdo (e não só o stat) de a.txt, o que roda o filtro `clean`.
suja_stat()   { touch -d '2000-01-01' "$1/a.txt"; G -C "$1" update-index --refresh >/dev/null 2>&1; touch "$1/a.txt"; }
hook()        { mkdir -p "$1/.git/hooks"; printf '#!/bin/sh\n"%s" %s\n' "$ATK" "$2" > "$1/.git/hooks/$3"; chmod +x "$1/.git/hooks/$3"; }

# --- vetores: filtros (o gate tem de barrar) ---
p_filter_clean()   { cfg "$1" filter.evil.clean "$ATK $2"; suja_stat "$1"; }
p_filter_smudge()  { cfg "$1" filter.evil.smudge "$ATK $2"; }
p_filter_process() { cfg "$1" filter.evil.process "$ATK $2"; }
p_filter_include() { printf '[filter "evil"]\n\tclean = %s %s\n' "$ATK" "$2" > "$WORK/incluido.cfg"; cfg "$1" include.path "$WORK/incluido.cfg"; suja_stat "$1"; }
p_filter_incl_if() { printf '[filter "evil"]\n\tsmudge = %s %s\n' "$ATK" "$2" > "$WORK/incluido-if.cfg"
                     cfg "$1" "includeIf.gitdir:$WORK/.path" "$WORK/incluido-if.cfg"; }
p_filter_worktree(){ cfg "$1" core.repositoryformatversion 1; cfg "$1" extensions.worktreeConfig true
                     printf '[filter "evil"]\n\tclean = %s %s\n' "$ATK" "$2" > "$1/.git/config.worktree"; suja_stat "$1"; }
# Seção e variável em maiúsculas, escritas à mão no arquivo: o gate não pode depender da caixa.
p_filter_caixa()   { printf '[FILTER "evil"]\n\tCLEAN = %s %s\n' "$ATK" "$2" >> "$1/.git/config"; suja_stat "$1"; }

# --- vetores: o que o isolamento do ADR 0009 já cobre, agora no `merge` ---
p_fsmonitor()      { cfg "$1" core.fsmonitor "$ATK $2"; }
p_hook_postmerge() { hook "$1" "$2" post-merge; }
p_hook_reftx()     { hook "$1" "$2" reference-transaction; }
p_hookspath()      { mkdir -p "$WORK/hooks-atacante"
                     printf '#!/bin/sh\n"%s" %s\n' "$ATK" "$2" > "$WORK/hooks-atacante/post-merge"; chmod +x "$WORK/hooks-atacante/post-merge"
                     cfg "$1" core.hooksPath "$WORK/hooks-atacante"; }
p_gpg()            { cfg "$1" merge.verifySignatures true; cfg "$1" gpg.program "$(atk_proprio "$2")"; }

echo "== vetores (git $(git --version | awk '{print $3}')) =="
medir filter_clean_status   status p_filter_clean
medir filter_clean_include  status p_filter_include
medir filter_clean_worktree status p_filter_worktree
medir filter_clean_caixa    status p_filter_caixa
medir filter_smudge_merge   merge  p_filter_smudge
medir filter_process_merge  merge  p_filter_process
medir filter_smudge_includeif merge p_filter_incl_if
medir fsmonitor_status      status p_fsmonitor
medir fsmonitor_merge       merge  p_fsmonitor
medir hook_postmerge        merge  p_hook_postmerge
medir hook_reftx_merge      merge  p_hook_reftx
medir hookspath_merge       merge  p_hookspath
medir gpg_verify_merge      merge  p_gpg

printf '%-26s %-7s %-9s %-9s\n' VETOR COMANDO BASE ISO
printf '%s' "$TABELA"

# --- controles positivos: o gate e o isolamento não podem só recusar tudo ---
CONTROLES=0; CONTROLES_FALHOS=0; CTAB=""
confere() {   # confere DESCRIÇÃO CONDIÇÃO(0 = ok)
  CONTROLES=$((CONTROLES+1)); local r=ok
  if [ "$2" -ne 0 ]; then r=FALHA; CONTROLES_FALHOS=$((CONTROLES_FALHOS+1)); fi
  CTAB+="$(printf '%-62s %s' "$1" "$r")"$'\n'
}
novo() { rm -rf -- "$WORK/r" && cp -a "$WORK/modelo" "$WORK/r"; }
head_de() { G -C "$WORK/r" rev-parse "$1"; }

novo; run_git iso status "$WORK/r"
confere "gate libera repositório sem filtro" "$GATE_RECUSOU"
confere "status de árvore limpa: saída vazia" "$([ -z "$(cat "$WORK/saida")" ]; echo $?)"

novo; echo mudou >> "$WORK/r/a.txt"; run_git iso status "$WORK/r"
confere "status de árvore suja mostra ' M a.txt'" "$(grep -q '^ M a.txt$' "$WORK/saida"; echo $?)"

novo; echo novo > "$WORK/r/untracked.txt"; run_git iso status "$WORK/r"
confere "status com arquivo novo mostra '?? untracked.txt'" "$(grep -q '^?? untracked.txt$' "$WORK/saida"; echo $?)"

novo; run_git iso merge "$WORK/r"
confere "merge --ff-only avança o HEAD até origin/main" "$([ "$(head_de HEAD)" = "$(head_de origin/main)" ]; echo $?)"
confere "o avanço não criou commit de merge" "$([ "$(G -C "$WORK/r" rev-list --merges --count HEAD)" = 0 ]; echo $?)"
confere "o conteúdo do arquivo é o do remoto (dois)" "$([ "$(cat "$WORK/r/a.txt")" = dois ]; echo $?)"

novo; cfg "$WORK/r" merge.ff false; cfg "$WORK/r" pull.rebase true; run_git iso merge "$WORK/r"
confere "merge.ff=false e pull.rebase=true na config: --ff-only vence" "$([ "$(head_de HEAD)" = "$(head_de origin/main)" ] && [ "$(G -C "$WORK/r" rev-list --merges --count HEAD)" = 0 ]; echo $?)"

novo; echo local > "$WORK/r/local.txt"; G -C "$WORK/r" add local.txt; G -C "$WORK/r" commit -q -m local
antes="$(head_de HEAD)"; run_git iso merge "$WORK/r"
confere "histórico divergido: --ff-only recusa e o HEAD não muda" "$([ "$(head_de HEAD)" = "$antes" ] && grep -qi 'fast-forward' "$WORK/saida"; echo $?)"

novo; echo mudou >> "$WORK/r/a.txt"; antes="$(head_de HEAD)"; run_git iso merge "$WORK/r"
confere "árvore suja no arquivo que o ff mexeria: o Git recusa e preserva" "$([ "$(head_de HEAD)" = "$antes" ] && grep -q 'mudou' "$WORK/r/a.txt"; echo $?)"

echo
echo "== controles positivos =="
printf '%s' "$CTAB"

echo
echo "== resumo =="
echo "vetores: $TOTAL | dispararam sob o isolamento: $FALHAS | baselines que não dispararam (medição inválida): $INVALIDOS"
echo "controles: $CONTROLES | falhos: $CONTROLES_FALHOS"
[ -n "${SEM_GATE:-}" ] && echo "(SEM_GATE=1: quebra de propósito; era esperado disparar nos vetores de filtro)"
[ "$FALHAS" -eq 0 ] && [ "$INVALIDOS" -eq 0 ] && [ "$CONTROLES_FALHOS" -eq 0 ]
