# nodus: Log de sessões

Mais recente no topo.

### 2026-11-13 — Sessão 9, continuação (Fase 1, marco 6: `nodus repos`)

- **Pedido**: critério 7 do MVP. Escolhas do dono no plano: `fetch` só com `--fetch`; descoberta só em filhos diretos; ambiente por `env VAR=... git ...` (sem mexer no `CommandRunner`); mensagem de erro genérica no repo com `git` falhando (sem capturar stderr); repos ignorados (symlink, `.git` arquivo, nome com controle) em silêncio.
- **Antes**: PR #13 (P-09) mesclado, `main` do nodus com `CI` verde em `4eb352b`.
- **Etapa 0 (medição)**: `medir.sh` estendido; comandos de leitura limpos em 10 vetores; submódulo hostil bloqueado por `--no-recurse-submodules`; `--no-auto-maintenance` tira o `git maintenance` do `fetch` (3 chamadas para 0 no trace). Defeito do próprio script achado no caminho (`p_filter_clean` inexistente) e corrigido antes de valer.
- **Feito**: `nodus-core/src/repos.rs` (`parse_branches`, `isolated_git_args`, `inspect_repo`, `FetchResult`), `nodus-cli/src/discovery.rs` (`find_repos`), subcomando `Repos`, `render_repos`. Sem dependência nova.
- **CI (resultado real, PR #14, aberto como rascunho)**: commit 1 (só testes) falhou por erro de compilação (`parse_branches`, `find_repos`, `render_repos`, `inspect_repo`, `isolated_git_args`, variante `Repos` inexistentes); commit 2 passou de primeira (85 testes no core e 44 na CLI, clippy limpo); mutação (`core.hooksPath=/dev/null` → `/tmp`) falhou 2 testes (`linha_isolada_tem_a_receita_do_adr_0009` e `toda_chamada_vai_por_env_e_isolada`), 83 passaram; revertida em commit próprio.
- **Mudança de doc**: `OVERVIEW.md` ainda dizia que o marco 5 estava "em revisão no PR #11" (já mesclado); corrigido.
- **Não verificado**: execução do `nodus repos` no Arch real (falta o dono rodar e comparar com `git branch -vv`); `--fetch` com rede real (a medição usou remoto local); `--fetch` em repo HTTPS privado vai falhar (`GIT_CONFIG_GLOBAL=/dev/null` descarta credenciais; SSH com agent deve funcionar, não testado); remotos com nome diferente de `origin` (só `origin` é buscado); a mensagem do erro de "dubious ownership" é genérica (`código 128`); `credential.helper` segue sem medição (ADR 0009).
- **Fica**: validar no Arch real; preset e o resto do MVP; P-11, P-12, P-02, P-03, P-07, P-08; `pacman -Qu` = 1 numa máquina em dia; fallback sem `pacman-contrib`; fechamento formal da Fase 0.

### 2026-11-13 — Sessão 9 (spike P-09: Git isolado)

- **Pedido**: seguir pelo P-09 antes do critério 7. Escolhas do dono no plano: medir só o `git` CLI (`gix` só se o CLI falhasse), entregar ADR + script repetível que gera o repo hostil num diretório temporário.
- **Antes**: PRs #11 e #12 (marco 5 e docs) mesclados; `CI` e `CI Private Files` verdes na `main`.
- **Medido** (git 2.56.0, `project/spikes/p09-git/medir.sh`, canários, sem rede, HOME vazio): 10 vetores disparam sem isolamento; com o isolamento candidato 9 ficam bloqueados. **O filtro `filter.<nome>.clean` continua executando em `git status` mesmo isolado**: não há chave fixa para anular. Anular todo `filter.*` achado bloqueia, mas é frágil; ler só refs (`for-each-ref`) não executa nada.
- **Primeira rodada inválida**: 9 de 11 baselines vieram "limpo" (dois defeitos do próprio script: `timeout` não executa função de shell; o preparo disparava o canário antes da medição). Tratei como medição inválida, corrigi e repeti. Quebra de propósito (tirar `core.hooksPath=/dev/null` do isolamento) fez `hook_reftx` e `hookspath` dispararem de novo.
- **Decisão do dono**: o critério 7 lê **só refs** (sem `git status`, sem "sujo/limpo" no MVP); `pre-auto-gc` e `credential.helper` declarados como lacunas não provadas.
- **Feito**: ADR 0009, `medir.sh`, `SECURITY.md` §6 (receita medida no lugar do "não verificado"), `LIBS.md`, `OVERVIEW.md`, ADR 0007 (aponta para o 0009), `PENDING.md` (P-09 resolvida).
- **CI (resultado real, PR #13)**: `Rust (clippy and tests)` e `guard` verdes (só docs e um script; nenhum código Rust mudou, então o CI valida apenas que nada quebrou, não o `medir.sh`, que o CI não executa).
- **Não verificado**: `credential.helper` (exige servidor HTTP de teste); `pre-auto-gc` (o gc não subiu nem com objetos soltos plantados); dono diferente só simulado (`GIT_TEST_ASSUME_DIFFERENT_OWNER`); remotos com nome diferente de `origin`; outros vetores (`diff.external`, `textconv`, `submodule`), outras versões do Git e outros SOs; `gix` não foi medido.
- **Fica**: critério 7 em si (descoberta de repos e refs com o isolamento do ADR 0009); preset e o resto do MVP; P-11, P-12, P-02, P-03, P-07, P-08; `pacman -Qu` = 1 numa máquina realmente em dia; fallback sem `pacman-contrib`; fechamento formal da Fase 0.

### 2026-11-13 — Sessão 8 (Fase 1, marco 5: `nodus updates`)

- **Pedido**: verificação de atualizações (critério 8 do MVP). Escolhas do dono no plano: subcomando separado `nodus updates [--json]` (o `inventory` segue offline), fallback para `pacman -Qu` rotulado como possivelmente desatualizado quando falta `checkupdates`, e medir o exit code antes de implementar.
- **Medição**: `checkupdates`, `pacman -Qu` e `flatpak remote-ls --updates` saíram com 0 (havia pendências; 1 app Flatpak, igual à amostra). O exit code **sem** pendências não deu para medir.
- **Feito**: `crates/nodus-core/src/updates.rs` (parsers `parse_pacman_updates` e `parse_flatpak_updates`, `collect_updates`, `PacmanSource`), subcomando `Updates` na CLI, `render_updates` com aviso quando a fonte é `pacman -Qu`. Sem dependência nova. Flatpak ausente vira `null`; `checkupdates` com exit 2 e `pacman -Qu` com exit 1 viram lista vazia (suposição, ver ADR 0008).
- **CI (resultado real, PR #11)**: testes e implementação passaram juntos (`Rust (clippy and tests)` e `guard` verdes). A falha do commit só de testes **não foi mostrada** no CI: o CI só rodou depois de abrir o PR. Mutação (exit 2 → 3 no `checkupdates`) falhou 1 teste (`checkupdates_codigo_2_significa_sem_atualizacoes`), 59 passaram; revertida em commit próprio.
- **Incidente**: um `git revert -q` falhou sem aviso e um `--amend` renomeou o commit da mutação; detectei pela linha ainda com `3`, nada tinha sido empurrado, refiz com `reset` e um revert normal.
- **Verificado depois da mesclagem (PR #11, `3c00007`)**: CI da `main` verde (`CI` e `CI Private Files`). O dono rodou `nodus updates` e `--json` no Arch real: 147 pacotes, igual ao `checkupdates | wc -l`; 1 app Flatpak (`org.gtk.Gtk3theme.Breeze 6.7.5 3.22`); fonte `checkupdates`; epoch preservado. Exit codes sem pendências medidos com banco temporário sem repositórios (sistema intocado): `checkupdates` = 2 (alta), `pacman -Qu` = 1 (média, ver ADR 0008).
- **Não verificado**: `pacman -Qu` = 1 numa máquina realmente em dia (só simulada); o caminho de fallback numa máquina sem `pacman-contrib`; locale `pt_BR`; `IgnorePkg`.
- **Fica**: preset e o resto do MVP; P-09, P-11, P-12, P-02, P-03, P-07, P-08; fechamento formal da Fase 0.

### 2026-11-12 — Sessão 7, continuação (Fase 1, marco 4: apps Flatpak no `inventory`)

- **Pedido**: Flatpak no inventário. Escolhas do dono no plano: só apps (sem runtimes), `flatpak_apps: null` quando o Flatpak não está instalado, e a amostra de `--app` medida de verdade.
- **Antes**: spike P-10 (PR #9) mesclado; `CI Private Files` verde na `main` (o `CI` principal estava em andamento quando conferi).
- **Medição**: o dono pediu para eu instalar um app. Instalei `org.nongnu.lingot` (1,2 MB, `--user`, sem runtime novo) e adicionei o remote `flathub` ao escopo do usuário (`flatpak remote-add --user`), porque o remote existente era só do sistema. `flatpak list --app` saiu `org.nongnu.lingot<TAB>1.1.1<TAB>stable<TAB>flathub<TAB>user`. Para desfazer: `flatpak uninstall --user org.nongnu.lingot` e `flatpak remote-delete --user flathub`.
- **Feito**: `FlatpakApp` e `parse_flatpak_list` (divide por tabulação sem descartar campo vazio; 5 colunas exatas; erro tipado em coluna a mais ou a menos, id/ramo/instalação vazio ou caractere de controle); `Inventory.flatpak_apps` como `Option<Vec>`; `RunnerError::NotFound` separado de `Spawn` (só `NotFound` do `flatpak` vira `None`; `pacman` ausente continua erro); seção `Flatpak` no texto; fixture `flatpak_list_app.tsv`; ADR 0008 e `README` das fixtures atualizados.
- **CI (resultado real, PR #10)**: commit 1 (só testes) falhou por erro de compilação (`FlatpakApp`, `NotFound` e `flatpak_apps` inexistentes); commit 2 passou com 22 + 34 testes e clippy limpo; mutação (versão vazia deixa de virar `None`) falhou 1 teste (`Some("")` em vez de `None`); revertida.
- **Mudança de comportamento**: o teste do runner do marco 2 passou a esperar `NotFound` para programa inexistente.
- **Não verificado**: instalação Flatpak de sistema para apps (só `user` medido); app **sem** versão (o campo vazio vem dos runtimes, não de um app); execução do `nodus inventory` no Arch real; o bloqueio de merge pelo check obrigatório (nenhum PR falhou sob a regra até agora); locale `pt_BR`; a regra de `--installation=NAME`.
- **Fica**: verificação de atualizações (critério 8: `checkupdates` e `flatpak remote-ls --updates`, ADR 0008); preset e o resto do MVP; P-09, P-11, P-12, P-02, P-03, P-07, P-08; fechamento formal da Fase 0.

### 2026-11-12 — Sessão 7, continuação (spike P-10: formato de pacman e Flatpak)

- **Pedido**: seguir com o spike P-10 antes do Flatpak no inventário. Escolhas do dono: eu rodo as medições (só leitura), amostras curtas e editadas, entrega = ADR + amostras sem código de produção.
- **Antes**: marco 3 (PR #8) mesclado; `CI` e `CI Private Files` verdes na `main`.
- **Medido** (pacman 7.1.0, Flatpak 1.18.4, checkupdates 1.13.1, `LANG=en_US.UTF-8`): `flatpak list` sai com tabulação, sem cabeçalho fora do terminal e com campo vazio possível; `pacman -Qu` deu 92 itens e `checkupdates` 134 (banco local velho); `vercmp` confirma que epoch vence; `checkupdates` só toca um banco temporário em `/tmp` (lido no script).
- **Feito**: ADR 0008 (formato escolhido: `checkupdates` como fonte principal, `-Qu` como alternativa offline, colunas do Flatpak fixadas, nodus não compara versões no MVP, `alpm-types` fora), fixtures em `crates/nodus-core/tests/fixtures/` com README, P-10 resolvida, `LIBS.md` ajustado.
- **CI**: a conferir no PR (só docs e fixtures; nenhum código mudou).
- **Não verificado**: `flatpak list --app` (a máquina não tem app Flatpak, formato assumido igual ao de runtime); locale `pt_BR` (não está gerado na máquina); exit codes de `pacman -Qu` e `checkupdates` sem atualização; linhas `[ignored]` de `IgnorePkg`; instalações `--user`/`--installation`; outras versões e distros.
- **Fica**: Flatpak no inventário e verificação de atualizações (próximos marcos); rodar `nodus inventory` no Arch real; P-09, P-11, P-12, P-02, P-03, P-07, P-08; fechamento formal da Fase 0.

### 2026-11-12 — Sessão 7, continuação (Fase 1, marco 3: hardware no `inventory`)

- **Pedido**: seguir o critério 2 do MVP com o hardware. Escolhas do dono no plano: `sysinfo`, campos CPU/RAM/discos, campo novo `hardware` no `--json`.
- **Feito**: tipos `HardwareInfo` e `Disk` no core (`collect` recebe o hardware pronto, o core continua sem I/O); adaptador `hardware.rs` na CLI com `sysinfo` 0.40.3; seção de hardware no texto (GiB com 1 casa decimal); filtro que descarta discos de tamanho 0 (`/proc`, `/sys`). Marco 2 (PR #7) mesclado antes, com `CI` e `CI Private Files` verdes na `main`.
- **CI (resultado real, PR #8)**: commit 1 (só testes) falhou por erro de compilação (`HardwareInfo`, `Disk` e o campo `hardware` inexistentes); commit 2 passou de primeira, com 18 + 17 testes ok e clippy limpo (a API do `sysinfo` conferida na docs.rs bateu); mutação (filtro `> 0` → `== 0`) falhou 1 teste (`["/proc", "/sys"]` em vez de `["/", "/home"]`); revertido.
- **Achado**: o teste de leitura real (`leitura_real_tem_invariantes_basicas`) **não** pegou a mutação, porque o runner do CI não tinha disco de tamanho 0 e as invariantes valem para lista vazia. Quem protege o filtro é só o teste com dados fixos.
- **LIBS.md**: licença e data do `sysinfo` (MIT, 2026-11-11) e do `serde_json` (MIT OR Apache-2.0, 2026-10-11) conferidas no crates.io em 2026-11-12; só metadados, sem leitura de código.
- **Não verificado**: execução em Arch real (o CI é ubuntu): falta o dono rodar `nodus inventory` e conferir com `lscpu`, `free -b` e `lsblk`; o bloqueio de merge pelo check obrigatório ainda não foi provado na prática; `cpu_threads` são threads lógicas, não núcleos físicos; o filtro por tamanho deixa passar `tmpfs` grande.
- **Fica**: Flatpak no inventário, preset; P-09 a P-12, P-02, P-03, P-07, P-08; fechamento formal da Fase 0.

### 2026-11-12 — Sessão 7 (Fase 1, marco 2: `nodus inventory`)

- **Pedido**: seguir a Fase 1 pelo critério 2 do MVP. Escolhas do dono no plano: sistema + pacotes pacman (sem hardware), texto + `--json`, trait `CommandRunner` no core.
- **Feito**: `nodus inventory [--json]`. Core: parsers puros de `os-release` e `pacman -Q`, tipos `Inventory`/`SystemInfo`/`Package`, trait `CommandRunner`. CLI: `RealRunner` (lista de argumentos, sem shell), render em texto. Deps novas: `anyhow`, `serde`, `serde_json`, `thiserror` (`LIBS.md` atualizado; `serde_json` fora do levantamento original e sem licença conferida).
- **Ruleset `protect-main`** (aplicado no fim da sessão 6, com autorização do dono): agora exige `guard` e `Rust (clippy and tests)`, igual ao `kin`. Confirmado pela resposta da API; este PR é o primeiro a exercitá-lo.
- **CI (resultado real, PR #7)**: commit 1 (só testes) falhou por erro de compilação (itens inexistentes); commit 2 passou com 14 + 17 testes ok e clippy limpo; commit de mutação (`index + 1` → `index` no número da linha) falhou em 3 testes (`number: 0` em vez de `1`); revertido.
- **Não verificado**: execução no Arch real (CI é ubuntu; falta o dono rodar `nodus inventory` e `--json` e comparar com `pacman -Q | wc -l`); o bloqueio de merge pelo check obrigatório ainda não foi provado na prática; a licença do `serde_json`; a leitura de `/etc/os-release` e `/proc` na CLI (`main.rs`) não tem teste automatizado.
- **Fica**: hardware (`sysinfo`), Flatpak, preset; P-09 a P-12, P-02, P-03, P-07, P-08; fechamento formal da Fase 0.

### 2026-11-12 — Sessão 6 (Fase 1, marco 1: esqueleto e CI)

- **Pedido**: iniciar a Fase 1. Escolhas do dono: primeiro marco = esqueleto + CI; binário `nodus`, crates `nodus-*`.
- **Feito**: Cargo workspace (`nodus-core`, `nodus-cli`), `nodus --version` e `--help` com `clap`, `.github/workflows/ci.yml` (modelo `padrao/workflows/ci-rust.yml`), `.gitignore` com `target/`, `GUIDELINES.md`.
- **CI (resultado real, PR #6)**: commit 1 (só testes) falhou no clippy por `cannot find value VERSION` (erro de compilação, não asserção); commit 2 passou: 4 testes ok, clippy limpo, `guard` verde, Rust 1.100.0 no runner.
- **Desvio do plano**: não criei `rust-toolchain.toml`. Com `channel = "stable"` não fixaria nada, e o kin também não tem. Fixar a versão fica como decisão futura.
- **Cargo.lock**: gerado pelo dono com `cargo generate-lockfile` (Rust 1.98.1 local; 21 pacotes, só a árvore do `clap` e os dois crates). O CI passou a usar `--locked`.
- **Não verificado**: nada de lógica do MVP foi escrita; não fiz a etapa de quebrar um teste de propósito. O check `Rust (clippy and tests)` ainda não é obrigatório no ruleset. O lock foi resolvido com Rust 1.98.1 e o CI usa 1.100.0.
- **Fica**: exigir o check no ruleset depois do merge, P-09 a P-12, P-02, P-03, P-07, P-08.

### 2026-11-12 — Sessão 5 (P-06: libs e licenças)

- **Pedido**: resolver a P-06 e fechar a Fase 0.
- **Feito**: `LIBS.md` (levantamento no crates.io) e ADR 0007. Achado: `alpm` é GPL-3.0, incompatível com linkar num binário MIT; pacman e Flatpak passam a ser processos externos. Rust confirmado. P-06 resolvida; novas P-09 a P-12.
- **CI**: PR #4 da sessão 4 mesclado, `guard` verde na `main`; `guard` do PR desta sessão a conferir.
- **Não verificado**: só metadados; sem leitura de código, `cargo audit` ou teste das libs. A fronteira de licença "processo vs link" não é parecer jurídico. `alpm-types` e `gix` não foram testados.
- **Fica**: P-09 a P-12 (Fase 1), P-02, P-03, P-07, P-08. O fechamento formal da Fase 0 depende de o dono aprovar.

### 2026-11-12 — Sessão 4 (P-04: monorepo)

- **Pedido**: resolver a P-04 (monorepo vs múltiplos repos).
- **Decisão**: monorepo com Cargo workspace, como o `kin` (ADR 0006). Plano aprovado pelo dono antes de escrever.
- **Feito**: ADR 0006; P-04 resolvida; `OVERVIEW.md` com a estrutura prevista. Nenhuma pasta nem `Cargo.toml` criados.
- **CI**: PRs #2 e #3 mesclados, `guard` verde na `main` nos dois; `guard` do PR desta sessão a conferir.
- **Não verificado**: nada de código; a decisão depende do contrato CLI/agente mudar mais rápido que o ritmo de releases, o que é previsão, não medição.
- **Fica**: P-06 (libs e licenças), P-07, P-08, P-02, P-03.

### 2026-11-12 — Sessão 3 (P-05: SECURITY.md)

- **Pedido**: ligar as proteções do repo; depois escrever o modelo de ameaças (P-05).
- **Conferido (sem mudar nada)**: ruleset `protect-main` ativo e idêntico ao do `arven`; secret scanning, push protection e alertas do Dependabot ligados. Security updates (PRs automáticos) desligado, igual ao `arven`, pelo padrão de triagem manual (ADR 0006 do workspace). Não testei um push direto na `main`, só a configuração.
- **Decisão assumida**: o modelo cobre só o MVP local; Core remoto, plugins e IA ficam como lacunas. A pergunta de escopo não teve resposta explícita.
- **Feito**: `SECURITY.md` (ativos, fronteiras de confiança, 9 ameaças, 3 classes de permissão, segredos, Git, supply chain, lacunas); P-05 resolvida; novas pendências P-07 e P-08.
- **CI**: PR #2 da sessão 2 mesclado; `guard` do PR desta sessão a conferir.
- **Não verificado**: as opções exatas para isolar o `git` (`SECURITY.md` §6) e a cobertura real das ameaças; sem revisão externa.
- **Fica**: P-04 (monorepo), P-06 (libs e licenças), P-07, P-08, P-02, P-03.

### 2026-11-12 — Sessão 2 (P-01: Fase 0 e MVP)

- **Pedido**: resolver P-01 (cortar um primeiro marco pequeno).
- **Decisões do dono**: Fase 0 só docs e ADRs; MVP = CLI + agente local, sem GUI; registro em ADRs + `MVP.md` + `ROADMAP.md`.
- **Feito**: ADRs 0003, 0004 e 0005 (stack provisória), `MVP.md`, `ROADMAP.md`; P-01 resolvida; novas pendências P-04 a P-06.
- **CI**: a conferir no PR desta sessão (`guard`).
- **Não verificado**: Rust como stack é hipótese da SPEC, sem validação; sem código nesta fase.
- **Fica**: P-04 (monorepo), P-05 (`SECURITY.md`), P-06 (libs e licenças), P-02, P-03.

### 2026-11-12 — Sessão 1 (criação do projeto)

- **Pedido**: criar o projeto a partir da spec "Universal", com o nome nodus, dentro da Arven Labs.
- **Decisão do dono**: repo público, MIT, só governança e docs; spec movida da raiz do workspace para `project/SPEC.md`.
- **Feito**: repo `Arven-Labs/nodus`, commit inicial na `main`, PR #1 com template, `guard` e `project/`.
- **CI**: a conferir no PR #1 (`guard`).
- **Não verificado**: ruleset e segurança ainda não ligados (dependem do `guard` verde na `main`).
- **Fica**: ligar ruleset, secret scanning, push protection e alertas do Dependabot depois do merge. *(Corrigido na sessão 3: já estavam ligados.)*
