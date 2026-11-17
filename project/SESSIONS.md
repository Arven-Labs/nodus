# nodus: Log de sessões

Mais recente no topo.

### 2026-11-17 — Sessão 21 (continuação 4), MVP declarado fechado (só docs)

- **Pedido**: o dono escolheu declarar o MVP fechado, em vez de seguir construindo. Recomendação minha, registrada: usar o nodus de verdade por uma ou duas semanas antes de decidir a Fase 1.5 ou o PKGBUILD.
- **Feito**: `MVP.md` (status fechado), `ROADMAP.md` (Fase 1 fechada; o texto da linha estava desatualizado, dizia `adopt` sem verificar e `undo` e `link` faltando) e `OVERVIEW.md`. Nenhum código.
- **Fica**: P-02 (busca de marca) **antes de divulgar ou publicar pacote**; P-14; P-17; remoto privado da `~/dotfiles`; reiniciar para carregar o kernel 7.3.4 (depois tirar o `--network host` do script da tag); uso real do nodus (a dor vira tarefa: `dotfiles dir set`, reversão de Flatpak, `link` em lote); decidir Fase 1.5 só depois.

### 2026-11-17 — Sessão 21 (continuação 3), primeira tag assinada `v0.1.0` (critério 1)

- **Pedido**: fechar o critério 1, em plan mode (plano aprovado). Decisões do dono: tag **antes** da busca de marca (P-02); verificação numa máquina limpa descartável.
- **Feito**: PR #77 (testes primeiro, CI **vermelho** com 2 falhas `0.0.1` vs `0.1.0`; depois bump em `Cargo.toml` e `Cargo.lock`, CI verde), mesclado em `6b2e4ed`, CI da `main` verde. O dono criou `git tag -s v0.1.0` e empurrou. Conferi antes do push: `git tag -v` com a chave do GitHub exit 0, chave errada exit 1. GitHub: `verified: true`, `reason: valid` (e-mail `noreply` casou). Roteiro `project/spikes/tag-limpa/verificar.sh` (container `archlinux`, `--rm`): **7 OK, 0 FALHA** (chave igual na API e no README, `tag -v` 0, errada 1, `Cargo.toml` da tag, instalação `--locked --tag`, `nodusctl --version = 0.1.0`). README ganhou "Instalar".
- **Achado**: o Docker falhou no `veth` (kernel em execução 7.2.14 sem módulos; só havia os do 7.3.4, atualização sem reinício). O dono não quis reiniciar; o script passou a usar `--network host`.
- **Não verificado**: o container usa o kernel e o hardware do host (a fumaça do `inventory` mostrou o i5 do dono e o kernel 7.2.14), então não é outra máquina; só Arch com `rust` do pacman, rustc da data; o ADR 0010 pede também conferir o PKGBUILD (P-14, adiada).
- **Fica**: o dono declarar o MVP fechado; P-02 (busca de marca) antes de divulgar ou publicar pacote; P-14; P-17; remoto da `~/dotfiles`; reiniciar para carregar o kernel novo; a branch `docs/verificacao-tag-limpa` mesclar.

### 2026-11-17 — Sessão 21 (continuação 2), revisão de fechamento do MVP (só docs)

- **Pedido**: depois dos dotfiles reais, fechar o MVP formalmente, em plan mode (plano aprovado). Revisão só de leitura, critério por critério.
- **Feito**: seção "Fechamento do MVP" no `MVP.md` (tabela dos 11 critérios). Só o **critério 1** está não pronto: falta a primeira tag assinada (`git tag -v` numa máquina limpa, selo "Verified" numa tag real e `cargo install --locked --git --tag` medido). Critério 10 pronto com lacuna (sem reversão de Flatpak) e 11 pronto com lacunas de verificação. Texto desatualizado dos critérios 2 e 3 corrigido. **Decidido pelo dono:** "configurações" do preset (critério 3) são os dotfiles do critério 11; o preset fica só de pacotes e apps.
- **Não verificado**: nada foi rodado; a tabela herda o que o `SESSIONS.md` registra. Discos do `inventory` sem conferência com `lsblk`.
- **Fica**: roteiro da primeira tag `v0.1.0` (você executa; usa a chave de assinatura, P-13); decidir se a P-02 entra antes da tag; remoto da `~/dotfiles`; P-14, P-17.

### 2026-11-17 — Sessão 21 (continuação), `~/dotfiles` real, só local (só docs)

- **Pedido**: depois da verificação, montar o `~/dotfiles` definitivo. Decisões do dono: adotar só `.bash_logout` e `.gitconfig` (os de shell de login ficam para depois); **só local, sem remoto** por enquanto.
- **Feito**: `~/dotfiles` criado com `git init -b main`; `~/.config/nodus/config.toml` real criado **à mão** (0600, pasta 0700, `dotfiles_dir = "/home/masterlxz/dotfiles"`), porque `dotfiles dir set` não existe. O dono rodou `dotfiles adopt` nos dois arquivos (histórico real, entradas 3 e 4); os dois são symlinks para `~/dotfiles`. Commit `d23bc62` na pasta (2 arquivos, 14 linhas, sem remoto), feito pelo dono; o `git commit` funcionou lendo o `.gitconfig` pelo symlink. Busca por `token`, `password`, `secret` e URL com credencial nos dotfiles deu 0 (o conteúdo não foi lido).
- **Dor de uso**: criar o `config.toml` à mão (com permissões certas) foi necessário; é o argumento concreto a favor de `dotfiles dir set`.
- **Não verificado**: `undo` e `link` com a pasta definitiva (só foram exercitados na rodada com `/tmp`); logout real; reinício de sessão com o `.gitconfig` como symlink.
- **Fica**: remoto privado da `~/dotfiles` (cópia fora da máquina; hoje o backup é só esta máquina); adotar `.bashrc` e `.bash_profile` (risco no login); `dotfiles dir set`; `link` em lote; nano e vim; P-02, P-14, P-17.

### 2026-11-17 — Sessão 21, verificação em dotfile real (só docs e script)

- **Pedido**: fechar a dívida "verificar `adopt`, `undo` e `link` num dotfile real", em vez de fatia nova.
- **Feito**: roteiro `project/spikes/dotfiles-real/verificar.sh` (o dono rodou no terminal real, `main` em `f703c12`, binário de `target/release` de 2026-11-16 20:51). Toca só `~/.bash_logout` e `~/.gitconfig` reais, com backup antes; config, histórico e pasta-fonte em `/tmp` (`XDG_*`), então o `config.toml` e o `history.jsonl` do dono não foram tocados. **26 OK, 0 FALHA**: `adopt` dos dois, `git config --global --list` igual antes e depois, `git config --global` escrevendo **pelo symlink** sem trocá-lo por arquivo comum (a escrita chegou na fonte; depois do `--unset` o diff com o backup ficou vazio), `undo 2` e `undo 1` (arquivos comuns, idênticos ao backup byte a byte), segundo `undo 1` recusado, `link` recusado com destino existente, `link` após mover o original, `undo` do link (só o symlink sai, a fonte fica) e estado final igual ao backup, sem sobras `.nodus-*`.
- **Correção minha**: avisei que o binário (de 20:51, anterior ao commit de 21:12) podia não ter o `undo` de ligação; tinha. A comparação por data de arquivo era palpite, e `nodusctl --version` (`0.0.1`) não diz o commit.
- **Editores (depois do roteiro, `adopt .bash_logout` com config e fonte em `/tmp`)**: Kate e VS Code (`code --wait`) salvaram **pelo symlink**: `ls -l` continuou symlink e a edição (uma linha em branco) apareceu na fonte. Nano e vim não testados (vim não está instalado). Depois, `history undo 1` e restauração do original; `cmp` com o backup deu igual.
- **Não verificado**: outros editores (nano, vim com `backupcopy` padrão, JetBrains) salvando por cima do symlink; logout real executando o `.bash_logout`; KDE; permissão negada; adoção com a pasta-fonte definitiva (a desta rodada foi em `/tmp`); versão do git não anotada.
- **Fica**: teste do editor (`adopt` num arquivo, abrir no editor, salvar, `ls -l`); o `~/dotfiles` do dono (commit, remoto privado); `link` em lote; `dotfiles dir set`; decidir sobre testes de corrida com gancho; considerar o hash do commit em `--version`; P-02, P-14, P-17.

### 2026-11-17 — Sessão 20, `history undo` de ligação (critério 11, fatia 5)

- **Pedido**: seguir com a fatia 5, em plan mode. Decisão do dono: as pastas-pai criadas pelo `link` **ficam**; ADR 0025 criado a pedido (PR #72, mesclado).
- **Feito (PR #73)**: testes primeiro, CI **vermelho** por item inexistente (`plan_undo_link`, `UndoLinkError`, `UndoLinkPlan`, `select_dotfile_entry`, `DotfileKind`, `NotADotfileEntry`). Os testes de `select_adoption` foram reescritos para `select_dotfile_entry` (devolve também adopt ou link). Implementação: core (`plan_undo_link` com alvo por texto, seleção generalizada), CLI (`dotfiles_undo.rs` com um `Plan` de duas variantes, `remove_link` que reconfere o alvo e remove só o symlink), render ("ligação desfeita" consultando a entrada de `undoes`). Sem mudança de formato do histórico. Passou de primeira no CI.
- **Mutação**: 4 quebras (aceitar qualquer symlink; ignorar "já desfeita"; `remove_file` no alvo em vez do symlink; inverter o render) derrubaram **11 testes** (8 no CLI, 3 no core); revertidas com `git revert`. **A reconferência do `read_link` no `remove_link` não tem teste.**
- **Verificado no Arch real, em HOMEs temporários (dono, terminal real, `cargo build --release` na branch do PR #73)**: ver ADR 0025. A 1ª rodada foi com o `!` (sem terminal) e só provou a recusa sem terminal.
- **Achado na verificação**: a mensagem de "já desfeita" dizia "adoção" para uma ligação; corrigida e testada.
- **Não verificado**: num dotfile real; permissão negada; `git config --global`, editores e KDE; o runner do CI ser root.
- **Fica**: `link` em lote; decidir se testa as reconferências de corrida (undo da adoção, undo da ligação, link) com gancho; verificar `adopt`, `undo` e `link` num dotfile real; o `~/dotfiles` do dono (commit, remoto privado); `dotfiles dir set`; P-02, P-14, P-17.

### 2026-11-16 — Sessão 20, verificação do `apply` confirmado com `s` (só docs)

- **Pedido**: fechar a dívida de verificação do `preset apply` (P-18, ADR 0016), em vez de código novo.
- **Feito**: roteiro num script (o dono rodou no terminal real, `main` em `261124a`, histórico em `XDG_STATE_HOME` temporário). Preset `sl`, `brave-bin`, nome inexistente: `plan` e `apply` com o mesmo plano; `s` rodou só `sudo pacman -S --needed -- sl`, instalou `sl 5.05-6` (explícito), exit 0; "Depois:" correto; histórico com 1 linha; `plan` seguinte com 2 presentes; `history undo 1` removeu `sl` (`sudo pacman -Rs -- sl`, exit 0) e o histórico ficou com 2 linhas.
- **Achado**: o pacman faz a própria pergunta `Proceed? [Y/n]` depois do `Executar? [s/N]` do nodus (duas confirmações); o nodus não passa `--noconfirm`, de propósito (ADR 0011).
- **Não verificado**: item indisponível que o pacman só recusa na instalação; custo do `plan` com muitos itens faltando; o `apply` do passo Flatpak junto com indisponíveis.
- **Fica**: `undo` da ligação de dotfile (fatia 5) e `link` em lote; verificar `adopt`, `undo` e `link` num dotfile real; o `~/dotfiles` do dono; reconferência de corrida com gancho (decisão); P-02, P-14, P-17.

### 2026-11-16 — Sessão 20, `dotfiles link` (critério 11, fatia 4)

- **Pedido**: seguir com a fatia 4, em plan mode. Decisões do dono: um caminho por vez; destino existente sempre recusa; criar pastas-pai; ADR 0024 criado a pedido (PR #69, mesclado).
- **Feito (PR #70)**: testes primeiro, CI **vermelho** por item inexistente (`plan_link`, `LinkError`, `LinkPlan`, `entry_from_link`, `Action::Link`, `DotfilesAction::Link`, `link`). Implementação: core (`plan_link` com alvo comparado por texto, `Action::Link`, `parse_line` v4 aceita `link`), CLI (`dotfiles_link.rs`: `missing_home_dirs` tolerante a pasta ausente, plano com aviso de execução de código, `ensure_dirs` reaproveitado, **`symlink(2)` direto** em vez de temporário + `rename`, histórico), `dotfiles_source_dir()` extraída do `adopt` (sem mudar comportamento), render, e o `history undo` passa a desviar toda entrada de dotfile antes de consultar o pacman. Uma rodada vermelha extra foi só de clippy (`type_complexity` num teste), corrigida com alias.
- **Mutação**: aceitar destino arquivo e comparar o alvo por `Path` derrubaram **3 testes** (2 no core, 1 no CLI); revertida com `git revert`. A quebra do braço "pai é symlink" não derrubou nada e não é lacuna (o braço seguinte recusa do mesmo jeito; ver ADR 0024).
- **Verificado no Arch real, em HOMEs temporários (dono, terminal real, `cargo build --release` na branch do PR #70)**: ver ADR 0024. O primeiro bloco colado (com `exec bash -l`) não rodou; foi refeito com um script.
- **Não verificado**: num dotfile real; permissão negada na criação das pastas; `git config --global`, editores e KDE com o symlink; o runner do CI ser root.
- **Fica**: `undo` da ligação (remover o symlink; fatia 5); `link` em lote; decidir se testa a reconferência da fonte e a do symlink do `undo` com um gancho; o `~/dotfiles` do dono (commit, remoto privado) e adotar os outros; `dotfiles dir set`; `apply` confirmado com `s`; P-02, P-14, P-17.

### 2026-11-16 — Sessão 20, `history undo` de dotfile (critério 11, fatia 3a)

- **Pedido**: seguir com a fatia 3, em plan mode. Decisões do dono: só o undo (o `link` vira a fatia 4); copiar de volta e **manter** a fonte; se o symlink divergiu, **recusar e explicar**; ADR 0023 criado a pedido (PR #67, mesclado).
- **Feito (PR #68)**: testes primeiro, CI **vermelho** por item inexistente (`select_adoption`, `plan_undo_adopt`, `entry_for_dotfile_undo`, `undo_dotfile`, variantes novas). Implementação: core (`plan_undo_adopt` com revalidação de `path` e `dir`, `select_adoption`, `entry_for_dotfile_undo`, `parse_line` v4 aceita `remove` com `undoes` e `dotfiles`), CLI (`dotfiles_undo.rs`: cópia para temporário ao lado do symlink, conferência byte a byte, reconferência do alvo, `rename`; desvio no `main.rs` antes de consultar o pacman; render). Helpers `snapshot` e `check_parents` do `adopt` ficaram `pub(crate)`.
- **Mutação**: a 1ª tentativa não compilou no clippy (variável sem uso) e não provou nada; a 2ª (tirar a conferência do alvo do symlink, a de "já desfeita" e a do pai symlink) derrubou **5 testes** (3 no CLI, 2 no core); revertidas com `git revert`. **A reconferência do symlink antes do `rename` não tem teste** (quebrada, nada falhou).
- **Verificado no Arch real, em HOME temporário (dono, terminal real, `cargo build --release` na branch do PR #68, `HOME=/tmp/hx`)**: adotar `.bashrc`, `history undo 1` com plano e `s`, arquivo comum de 10 bytes, `diff` igual à fonte, `history` com `desfez [1]`, segundo `undo 1` recusado.
- **Não verificado**: num dotfile real; recusas no binário real; `git config --global`, editores e KDE com a cópia de volta; se o runner do CI é root (o teste de falha do histórico sai cedo como root).
- **Fica**: `dotfiles link` (fatia 4, ADR próprio); decidir se testa a reconferência do symlink com um gancho; o `~/dotfiles` do dono (commit, remoto privado) e adotar os outros quatro; `dotfiles dir set`; `apply` confirmado com `s`; P-02, P-14, P-17.

### 2026-11-16 — Sessão 20, `dotfiles adopt` (critério 11, fatia 2)

- **Pedido**: o dono decidiu que vincular dotfile entra no MVP; em plan mode. Decisões do dono: pasta-fonte espelhando o HOME (`dotfiles_dir`), `adopt` primeiro e `undo` depois, repo privado fora do workspace; ADR 0022 criado a pedido (PR #65, mesclado).
- **Feito (PR #66)**: testes primeiro, CI **vermelho** por item inexistente em quatro rodadas (core: `plan_adopt`, `validate_dir`, histórico v4; config: `dotfiles_dir`; I/O: `adopt_file`). Implementação: core (`plan_adopt`, `validate_dir`, `check_dir_shape`, histórico v4 com `Action::Adopt`, `Manager::Dotfiles` e `dir`, `undo` recusa dotfile com mensagem clara), `config.rs` (`dotfiles_dir`), `dotfiles_adopt.rs` (`adopt_file`: pais sem symlink, pastas da fonte criadas uma a uma, cópia `create_new`, conferência byte a byte, conferência de que o original não mudou, `rename` atômico do symlink; fluxo com plano e `Executar? [s/N]`), subcomando `dotfiles adopt <caminho>`, render do histórico. `validate_path` passou a recusar caractere de controle.
- **Mutação**: 7 quebras (`create_new` virou `create`+`truncate`; sem conferência byte a byte; sem conferir o original; sem checar pai symlink; sem checar symlink na pasta-fonte; limite `>=`; máscara com setuid) derrubaram **9 testes** (2 no core, 7 no CLI); revertidas com `git revert`.
- **Diferenças do plano**: o teste do `undo` com dotfile e o do render do histórico foram escritos junto da implementação, sem rodada vermelha própria; o `dotfiles_dir` no config só confere a forma (a denylist da pasta-fonte vale no `adopt`); duas asserções minhas estavam erradas na primeira rodada (nota de rodapé do histórico; `TmpDir` apagado na hora) e foram corrigidas.
- **Verificado no Arch real, em HOME temporário (dono, terminal real, `cargo build --release` na branch do PR #66, 2026-11-16, `HOME=/tmp/hx`)**: `dotfiles adopt .bashrc` mostrou o plano (10 bytes, permissão 644, aviso de segredo) e, com `s`, adotou: `/tmp/hx/.bashrc` virou symlink para `/tmp/hx/dot/.bashrc` (cópia `-rw-r--r--`, 10 bytes); `dotfiles` mostrou `symlink -> /tmp/hx/dot/.bashrc` e `Presentes: 1 de 9`; `history` listou `dotfile adotado: .bashrc (pasta-fonte /tmp/hx/dot)`; `history undo 1` recusou com a mensagem de dotfile e nada mudou; adotar de novo recusou (`já é um symlink`).
- **Verificado no Arch real, adoção de verdade (dono, terminal real, mesmo binário, 2026-11-16)**: com `dotfiles_dir = "/home/masterlxz/dotfiles"` (repo privado novo, `git init` vazio) num `config.toml` 0600 com `version = 1`, `dotfiles adopt .bash_logout` mostrou o plano (21 bytes, permissão 644) e, com `s`, adotou: `~/.bash_logout -> /home/masterlxz/dotfiles/.bash_logout`, cópia `-rw-r--r--` de 21 bytes, `diff` igual; `dotfiles` mostrou `symlink -> ...` e `Presentes: 5 de 9`; o `history` real listou a entrada Flatpak v3 de 2026-11-15 e a adoção v4 juntas. Antes disso, duas tentativas foram **recusadas pelo nodus sem alterar nada**: o `config.toml` criado por `>>` ficou 0644 e sem `version`, e depois o terminal ainda tinha `HOME=/tmp/hx` do ensaio (as escritas falharam em caminho inexistente). Ambas foram erro do roteiro que eu passei, não do nodus.
- **Não verificado**: recusas no binário real (destino existente, pai symlink, arquivo grande, sem terminal); se o runner do CI é root (dois testes saem cedo como root); regravação do mesmo tamanho no mesmo tick do relógio (não detectada); `git config --global`, editores e KDE com o symlink; nenhum teste de CLI de ponta a ponta do binário (o fluxo é testado com entrada e saída injetadas).
- **Fica**: o dono decidir o que fazer com o `~/dotfiles` (commit, remoto privado; o nodus não mexe) e se adota os outros quatro (`.bashrc`, `.bash_profile`, `.profile`, `.gitconfig`, este com risco de token); criar o config exige `version = 1` e `chmod 600` à mão (candidato a `dotfiles dir set`); `history undo` de dotfile e `dotfiles link` (fatia 3, ADR próprio); `apply` confirmado com `s`; P-02, P-14, P-17.

### 2026-11-16 — Sessão 20, dotfiles (critério 11, fatia 1: relatório)

- **Pedido**: seguir com a última frente do MVP, em plan mode. Corrigi minha própria recomendação: o `MVP.md` não tinha critério de dotfiles (só ROADMAP, SPEC §16 e SECURITY). Decisões do dono: criar o critério 11; primeira fatia = relatório de lista curada; lista fixa no código mais `config.toml`; o ADR 0021 criado a pedido.
- **Feito**: PR #63 (só docs: critério 11 e ADR 0021, mesclado). PR #64: testes primeiro, CI **vermelho** nas três rodadas (módulo do core, campos do config, e `home_dir`/`observe`/`report`/`Command::Dotfiles`/`render_dotfiles`, sempre por item inexistente). Implementação: `nodus-core/src/dotfiles.rs` (puro: `validate_path`, denylist por componente, `effective_list`, `classify`, normalização léxica do alvo do symlink), `nodus-cli/src/dotfiles.rs` (só `symlink_metadata` e `read_link`), `config.rs` (`dotfiles_extra` e `dotfiles_skip`, validados na leitura e preservados por toda escrita do config), `render_dotfiles`, subcomando `dotfiles [--json]`. CI verde.
- **Mutação**: tirar `.ssh` da denylist e deixar `..` passar: **4 testes** falharam (2 no core, 2 no CLI: `recusa_a_denylist_de_segredos`, `recusa_caminho_vazio_absoluto_ou_com_componente_estranho`, `report_recusa_caminho_da_denylist...`, `read_recusa_dotfiles_invalidos...`); revertida com `git revert`.
- **Diferença do plano**: o texto de erro das listas antigas do config mudou de `nome inválido no item N` para `` `chave`: item N: nome inválido `` (refator `name_list_with`); os testes antigos só conferiam o nome do arquivo.
- **Verificado no Arch real (dono, terminal real, `cargo build --release` na branch do PR #64, 2026-11-16)**: `nodusctl dotfiles` listou os 9 da lista padrão (`.bashrc` 1670, `.bash_profile` 78, `.bash_logout` 21, `.profile` 21 e `.gitconfig` 289 bytes como arquivo; `.config/git/config`, `.zshrc`, `.vimrc` e `.tmux.conf` ausentes) e fechou com `Presentes: 5 de 9.`; com `config.toml` em `XDG_CONFIG_HOME` temporário, `dotfiles_extra = [".ssh/config"]` deu erro citando `.ssh/config` e nenhuma listagem. Rodei depois, no mesmo binário: `--json` é JSON válido (`python3 -m json.tool`; o `jq` não existe nessa máquina), e `dotfiles_skip` com `dotfiles_extra` (`.config/starship.toml`, 1533 bytes) deu `Presentes: 6 de 8.`
- **Não verificado**: symlink real e dotfile de pasta no Arch (só testes de unidade); nenhum teste de CLI de ponta a ponta; caminho cujo pai intermediário é symlink (o `symlink_metadata` segue o pai, só metadados); nome de arquivo fora de UTF-8 (sai com substituição); `.gitconfig` com segredo dentro (o nodus não lê o conteúdo, ADR 0021, lacunas).
- **Fica**: decidir se vincular dotfile entra no MVP (hoje não está nos critérios); `apply` confirmado com `s`; P-02, P-14, P-17.

### 2026-11-15 — Sessão 19, ignorar para sempre (critério 5, fatia 3)

- **Pedido**: o PR B do critério 5, em plan mode. Decisões do dono: lista no `config.toml`; `suggest` **esconde e conta** (`--all` mostra); desfazer por `preset ignored` e `preset unignore`.
- **Feito (PR #61)**: testes primeiro, CI **vermelho** (`hide_ignored`, `Config`, `add_ignored`, `remove_ignored`, `suggestions_json`, `render_ignored` e os campos novos inexistentes). Implementação: `hide_ignored` no núcleo (pura) e os validadores de nome públicos; `config.rs` com `Config`, leitura estrita das listas e **escrita sempre por leitura-modificação-escrita** (o `write_workspace` antigo apagaria os ignorados); `adopt` com `i`/`ignorar` (`[s/N/i/q]`); `suggest` com `--all`, rodapé de contagem e `ignored` no `--json`; `preset ignored` e `preset unignore`. ADR 0020; emendas aos ADRs 0016, 0018 e 0019. CI **verde** de primeira (258 testes no núcleo, 225 na CLI).
- **Mutação**: a primeira (`write_workspace` sem as listas) **não compilou no clippy** (`field_reassign_with_default`), então não provou nada; reescrita em estilo aceito, junto de `hide_ignored` sem esconder: **2 testes** do `config` e **3** do núcleo vermelhos; revertidas com `git revert`.
- **Verificado no Arch real depois do merge (PR #61, `e9e0ab6`, dono, terminal real, cópia do preset em `/tmp` e `XDG_CONFIG_HOME` temporário)**: `adopt` respondendo `s`, `i`, `n`, `i` adotou `alsa-firmware`, ignorou `alsa-utils` e `ark` e pulou `android-tools`; `suggest` mostrou só `android-tools` com `2 ignorados (use --all para ver).`; `suggest --all` listou os 3 com `Ignorados entre os listados (2): alsa-utils, ark`; `preset ignored` listou os 2; `workspace set` depois de ignorar **não apagou** a lista; `preset unignore ark` tirou só ele (`suggest` voltou a mostrar `ark` e `android-tools`, `1 ignorado`); `unignore nao-existe` deu erro e não gravou. Duas execuções não provaram nada: a primeira deixou `<um dos nomes listados>` literal no comando (erro de sintaxe do bash, meu), e a segunda rodou num shell sem o `XDG_CONFIG_HOME` temporário (leu o config real, sem ignorados; nada foi gravado nele).
- **Não verificado**: o caminho do Flatpak e o `q` junto com `i`; `unignore` de nome nas duas listas; nenhum teste de CLI de ponta a ponta; falha do `rename`; `unignore` de nome presente nas duas listas; `suggest` passou a exigir `HOME` ou `XDG_CONFIG_HOME` absoluto.
- **Fica**: o dono conferir no Arch; `apply` confirmado com `s`; P-02, P-14, P-17; dotfiles (a última frente do MVP).

### 2026-11-15 — Sessão 19, `preset adopt` (critério 5, fatia 2)

- **Pedido**: seguir com o critério 5, em plan mode. Decisões do dono: **dois PRs** (adotar agora, ignorar depois); escrita por **inserção de texto com `toml::Spanned`** (sem `toml_edit`, sem crate novo: `serde_spanned` já está no lockfile); os ignorados do PR B ficam no `config.toml` do ADR 0018.
- **Feito (PR #59)**: testes primeiro, CI **vermelho** (`add_to_preset`, `ask`, `Adoption`, `write_preset` e `PresetAction::Adopt` inexistentes). Implementação: `add_to_preset` (núcleo, puro, com reparse do resultado), `adopt.rs` (pergunta com entrada e saída injetadas), `preset_file::write_preset` (segue symlink, preserva permissões, compara o conteúdo com o lido, atômico), `config::commit` como `pub(crate)`, `preset adopt` no CLI. ADR 0019, emenda ao 0016, SECURITY, MVP, OVERVIEW, README. CI **verde** de primeira (232 testes no núcleo, 220 na CLI): confirmou que o span do `toml::Spanned` inclui os colchetes. **Mutação**: a checagem de "arquivo mudou" desligada (1 teste vermelho) e a vírgula do último elemento nunca inserida (4 testes vermelhos); revertida com `git revert`.
- **Diferenças do plano**: `add_to_preset` recebe duas listas em vez de um tipo `Additions` (sem tipo novo); a checagem de mudança compara o **conteúdo** lido, não tamanho e `mtime` (determinística e testável).
- **Verificado no Arch real depois do merge (PR #59, `7ecbb9d`, dono, terminal real, cópia em `/tmp`)**: preset gerado por `init` com `alsa-firmware` e `alsa-utils` apagados; `preset adopt` perguntou as duas na ordem (`Adotar alsa-firmware (pacman)? [s/N/q]`), respondidas `s` e `n`; o `diff` mostrou só `alsa-firmware` de volta, no fim da lista de pacman (linha 157), e `alsa-utils` fora; `preset suggest` listou só `alsa-utils`. Uma primeira tentativa, tudo num comando via `!`, não testou nada (sem terminal e sem as linhas apagadas: "Nada fora do preset"). Detalhe cosmético: a mensagem final imprime `flatpak 0 ()` sem nada de Flatpak.
- **Não verificado**: a resposta `q` (só havia 2 itens); o caminho do Flatpak; preset com comentários à mão, vírgula faltando ou lista inline (só os testes de unidade); symlink; `adopt` sem terminal; nenhum teste de CLI de ponta a ponta; falha do `rename`; preset com strings multilinha ou tabelas inline.
- **Fica**: o dono conferir no Arch; PR B (`i`, ignorar para sempre, no `config.toml`); `apply` confirmado com `s`; P-02, P-14, P-17; dotfiles.

### 2026-11-15 — Sessão 19, `workspace set`/`unset` (critério 6, fatia 3)

- **Pedido**: lembrar a escolha do workspace, em plan mode. Decisões do dono: `config.toml` em TOML no XDG, e precedência argumento, variável, gravada, detecção. Medi antes (só leitura) o padrão do histórico (`history_path`, `check_file`, 0700/0600) para reusar.
- **Feito (PR #57)**: testes primeiro, CI **vermelho** (`config_path` e o campo `action` inexistentes). Implementação: `config.rs` (caminho, leitura estrita com limite de 64 KiB, gravação atômica com temporário e `rename`, `clear`), `Source::Stored`, `resolve` e `pick_dir` com o parâmetro `stored` (closure: o config só é lido se nem argumento nem variável decidem), `workspace::validate`, `workspace set <pasta>` e `unset` no `cli.rs` e no `main.rs`; `history::check_file` passou a `pub(crate)` e `toml` entrou nas dependências da CLI (já aprovado; `Cargo.lock` editado à mão e aceito pelo `--locked` do CI). ADR 0018; emendas ao ADR 0017; SECURITY, README, MVP, OVERVIEW. CI **verde** (214 testes do CLI). **Mutação**: `0o600` virou `0o644` no temporário e a variável deixou de vencer; **11 testes** vermelhos (5 do `config`, 6 do `workspace`); revertida com `git revert`.
- **Verificado no Arch real depois do merge (PR #57, `6e2a630`, dono, binário novo)**: `workspace set ~/Documents/workspace` gravou o caminho real e avisou `repositórios: 9`; o arquivo ficou `-rw-------` e o diretório `~/.config/nodus` `drwx------`; `workspace` e `repos` deram `(gravado)` (cabeçalho do `repos` em stderr); `NODUS_WORKSPACE=/tmp` venceu a gravada (`ambiente`, 0 repositórios, com aviso); `unset` removeu a escolha e a detecção voltou (`detectado`, 9). O `unset` apaga só o arquivo: o diretório `~/.config/nodus` sobra vazio (conhecido, de propósito). O conteúdo do `config.toml` não foi lido no Arch (o `unset` já o tinha apagado); só o teste de ida e volta do CI o cobre.
- **Não verificado**: nenhum teste de CLI de ponta a ponta de `set`/`unset`; falha do `rename` e temporário já existente; `set` numa pasta com 0 repositórios (grava e avisa em stderr).
- **Fica**: o dono conferir no Arch; critério 5 interativo; `apply` confirmado com `s`; P-02, P-14, P-17.

### 2026-11-15 — Sessão 19, P-19 (`Broken pipe`)

- **Pedido**: depois do fechamento da Fase 0 (PR #54), corrigir a P-19 em plan mode. Medi antes (só leitura): `unsafe_code = "forbid"` impede resetar o `SIGPIPE`; 17 pontos de saída em `main.rs`; `std::io::pipe()` permite um pipe com a leitura já fechada (teste sem corrida). Escolhi, sem perguntar ao dono, o exit 141 (plano aprovado com isso dito).
- **Feito (PR #55)**: teste de ponta a ponta primeiro (`tests/broken_pipe.rs`, o primeiro teste de CLI do projeto), CI **vermelho** (exit 101, `panicked` em `stdio.rs`; o teste de controle com stdout aberto passou). Depois `emit` e a troca dos 17 `print!`/`println!` em `main.rs`; GUIDELINES com a regra. CI **verde** (2 testes novos). **Mutação**: `exit(0)` no lugar de 141; pega por **1 teste** (`left: Some(0)`, `right: Some(141)`), CI vermelho; revertida com `git revert`.
- **Verificado no Arch real depois do merge (PR #55, `304ec9e`, dono, binário novo)**: `repos --json | jq .` com o `jq` ausente: sem pânico, só `bash: jq: command not found`, e `${PIPESTATUS[0]}` = **141**. `repos | head -1` imprimiu a linha e deu `0`: correto, o `repos` escreve tudo de uma vez e o `head` fecha depois, então não há `EPIPE` (a expectativa de 141 que eu escrevi para esse caso estava errada).
- **Não verificado**: só `workspace` é exercitado pelo teste; o `--help` do clap com pipe fechado; o `apply` com EPIPE no "Depois:".
- **Fica**: o dono conferir no Arch; `apply` confirmado com `s`; P-02, P-14, P-17; lembrar a escolha do workspace; critério 5 interativo.

### 2026-11-15 — Sessão 19, fechamento formal da Fase 0

- **Pedido**: o dono aprovou o fechamento da Fase 0 (resposta a uma pergunta direta, depois de eu listar o critério de saída do ADR 0003 e conferir que as entregas existem: `MVP.md`, `ROADMAP.md`, ADRs 0004 a 0007, `SECURITY.md`, `LIBS.md`, monorepo no ADR 0006, e os marcos da Fase 1 já entregues).
- **Feito (só docs)**: `ROADMAP.md` (Fase 0 fechada; a linha da Fase 1 estava velha, "fatia 2 em revisão", e foi reescrita com o estado real) e `OVERVIEW.md`. Não conferi o conteúdo de cada ADR e de cada doc de novo nesta sessão, só a existência e o que as sessões anteriores registraram.
- **Fica**: P-19 (`Broken pipe`, em plan mode); P-02, P-14, P-17; lembrar a escolha do workspace; critério 5 interativo; `apply` confirmado com `s`.

### 2026-11-15 — Sessão 19, `repos` usa o workspace (critério 6, fatia 2) e `apply` verificado até o prompt

- **Pedido**: seguir a recomendação: verificar o `preset apply` com item indisponível e depois o critério 6. Em plan mode.
- **Verificado no Arch real (dono, terminal)**: preset `version = 1` + `pacman = ["sl", "brave-bin", "pacote-que-nao-existe-xyz"]` (o formato é chave de topo, **sem** `[pacman]`; meu primeiro TOML estava errado e o binário não estava no `PATH`, usei `target/release/nodusctl`). `plan` e `apply` deram o mesmo plano (`sl` a instalar, `brave-bin` presente, o inexistente em "Indisponíveis"); o comando foi `sudo pacman -S --needed -- sl`; respondido `n`, "Nada foi executado".
- **Feito (PR #52)**: testes primeiro, CI **vermelho** (`pick_dir` inexistente e `dir` ainda não `Option`). Implementação: `workspace::pick_dir` (argumento vale como está; sem argumento reusa `resolve`; sem workspace, erro com a saída), `Repos { dir: Option<PathBuf> }`, cabeçalho `Workspace: <pasta> (<origem>)` em stderr, `Source::label` (o `render_workspace` passou a usá-lo). ADR 0017: emenda ao item 4. CI **verde** (180 testes do CLI). **Mutação**: `pick_dir` ignora o argumento; pega por **1 teste** (`pick_dir_com_argumento_devolve_o_argumento_sem_origem`), CI vermelho; revertida com `git revert`.
- **Verificado no Arch real depois do merge (PR #52, `92b9c5a`, dono, binário novo)**: `nodusctl repos` sem argumento listou os 9 repositórios de `~/Documents/workspace` com `Workspace: ... (detectado)` em stderr; `repos /tmp` deu 0 repositórios, sem cabeçalho (como antes); `repos --json 2>/dev/null | python3 -m json.tool` deu JSON válido (stdout limpo). Achado: com o leitor do pipe fechado (`| jq` sem `jq` instalado) o `println!` entra em pânico com `Broken pipe`; não é do PR #52, vale para todos os comandos (P-19).
- **Não verificado**: `apply` confirmado com `s` e um indisponível; se o histórico ficou sem linha após o `n`; nenhum teste de CLI de ponta a ponta (o `main.rs` só liga as peças); workspace com 0 repositórios sem aviso próprio no `repos`.
- **Fica**: o dono reconstruir e rodar `nodusctl repos`; P-02, P-14, P-17; lembrar a escolha do workspace (config, ADR novo); critério 5 interativo; fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, verificação real do `suggest`, da P-18 e do `workspace`

- **Pedido**: conferir na máquina real os três PRs mesclados sem teste real (#48, #49, #50). O dono reconstruiu o binário (`cargo build --release -p nodus-cli`, 3,6 s) e rodou `workspace` e `preset init > p.toml && preset suggest p.toml`; eu rodei o resto por leitura.
- **Verificado**: `workspace` detectou `~/Documents/workspace` (9 repositórios) e listou `~/Projects` (0); `workspace /tmp` deu "informado, 0 repositórios" com aviso; `NODUS_WORKSPACE` válido venceu a detecção, inexistente e relativa deram erro (exit 1); `--json` trouxe `chosen` e `candidates`. `init` seguido de `suggest` deu "Nada fora do preset". `preset plan` com `sl`, `brave-bin`, um nome inexistente e `java-runtime`: `pacman_missing = [sl, java-runtime]`, `pacman_present = 1` (o `brave-bin`, AUR já instalado, conta como presente), `pacman_unavailable = [nome inexistente]`, em texto e em `--json`. O `java-runtime` (só por *provides*) em "instalar" confirma na prática a escolha da simulação em vez de uma lista de pacotes.
- **Não verificado**: o `apply` real com um item indisponível (precisa de terminal); a seção "Fora dos repositórios" do `suggest` com um preset sem os pacotes do AUR; um item do AUR ausente da máquina; permissão real e symlink circular no `workspace`.
- **Fica**: o dono rodar `nodusctl preset apply <preset>` com o preset de teste (`sl` ausente, `brave-bin`, inexistente), respondendo `n`: o comando mostrado deve levar só o instalável; P-02, P-14, P-17; critério 5 (pergunta interativa) e critério 6 (lembrar a escolha, `repos` usar o workspace); fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, critério 6 do MVP (`nodusctl workspace`)

- **Pedido**: seguir o MVP. Em plan mode, **medi antes** (só leitura): o workspace real é `~/Documents/workspace` com 9 repositórios filhos; `~/Projects` existe vazio; uma varredura do `$HOME` por pasta-pai erraria (escolheria o `$HOME`, com 3 repositórios soltos). Decisões do dono: candidatos conhecidos ranqueados, só relatório, e escolha manual por argumento e variável de ambiente (sem estado; "salvar a escolha" ficou de fora).
- **Feito (PR #50)**: testes primeiro, CI **vermelho** (módulo vazio). Implementação: `workspace.rs` (`resolve`, `detect`, `report_json`, reusa `find_repos`), `render_workspace`, `Command::Workspace`, leitura de `NODUS_WORKSPACE` e `HOME` no `main.rs`; ADR 0017; README, MVP, OVERVIEW. O primeiro CI da implementação falhou por **clippy** (`unnecessary_sort_by`), antes de rodar os testes; corrigido com `sort_by_key` e `Reverse` (estável, o desempate segue a ordem da lista). Depois, CI verde.
- **Mutação**: o ranking passou a ignorar a contagem de repositórios (`Reverse(count.min(1))`); compilou e foi pega por **1 teste** por asserção (`candidatos_saem_ordenados_por_repositorios_decrescente`); revertida com `git revert`. Cobertura fina: só esse teste protege o ranking por contagem.
- **Não verificado**: o comando no Arch real; candidato ilegível por permissão real; symlink circular; nenhum teste de CLI de ponta a ponta.
- **Fica**: o dono rodar com binário novo (`nodusctl workspace` deve dar `~/Documents/workspace (detectado, repositórios: 9)`, com `~/Projects` vazio; `nodusctl workspace /tmp` dá `informado`, 0 repositórios, com aviso; `NODUS_WORKSPACE=/nao/existe nodusctl workspace` dá erro); P-02, P-14, P-17; fatia seguinte do critério 5; lembrar a escolha do workspace e o `repos` usá-lo; fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, P-18 (itens que o pacman não encontra)

- **Pedido**: atacar a P-18. Em plan mode, **medi antes de decidir** (só leitura, `pacman -S --print`): um item fora dos repositórios aborta a transação inteira com `target not found`, exit 1, mesmo já instalado, e o nativo do mesmo pedido também não instala; `java-runtime` (só por *provides*) simula com exit 0; o nome do culpado só sai no stderr, que o nodus não captura. Decisões do dono: corrigir no `plan`/`apply` por simulação (não no `init`, não formato v2) e o `apply` pula e avisa.
- **ADR**: o dono pediu o ADR 0016 (`adr/0016-itens-fora-dos-repositorios.md`), que cobre a P-18 e a decisão do `suggest` (só relatório, AUR à parte), com as medições e as alternativas descartadas.
- **Feito (PR #49)**: testes primeiro, CI **vermelho** (erro de compilação: `Plan.pacman_unavailable`). Implementação: `split_unavailable` e `simulate_install` (`preset.rs`), seção "Indisponíveis" e mensagem de plano só com indisponíveis (`render.rs`); `apply.rs` e `history.rs` não mudaram (já usam `pacman_missing`). CI verde.
- **Mutação**: a primeira (lote que falha vira tudo instalável) **não compilou** (`unreachable statement`), então não provou nada e foi revertida; a segunda (destinos trocados no laço por item) compilou e deixou **3 testes vermelhos por asserção**; revertida com `git revert`.
- **Não verificado**: o `apply` real com um item do AUR (só a simulação foi medida); custo em máquina nova com muitos itens faltando; "não encontrado" vs outro exit 1 por item (indistinguíveis sem o stderr); nenhum teste de CLI de ponta a ponta.
- **Fica**: o dono verificar com binário novo (preset com um nativo ausente + `brave-bin` + um nome inexistente: `plan` mostra os indisponíveis; `apply` mostra o comando só com o instalável; pode responder `n`); P-02, P-14, P-17; critério 6; fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, critério 5 do MVP (`preset suggest`, só relatório)

- **Pedido**: seguir o MVP. Antes, validei o `inventory` por leitura contra `/etc/os-release`, `uname`, `/proc` e `pacman -Q` (sistema, kernel, CPU, memória, 1244 pacotes e Flatpak 0 batem). Depois o critério 5, em plan mode. Decisões do dono: só relatório (sem escrever no preset, sem estado de ignorados) e subcomando novo.
- **Feito (PR #48)**: testes primeiro, CI **vermelho** (erro de compilação: `suggest`, `collect_suggestions`, `render_suggestions`, `Suggestions`). Depois a implementação: `preset.rs` (`Suggestions`, `suggest`, `collect_suggestions`), `render.rs`, `cli.rs`, `main.rs`; README, MVP, OVERVIEW. CI verde. **Mutação**: filtro do pacman invertido, CI **vermelho** com 10 testes do core falhando; revertida com `git revert`.
- **Achado**: dos 154 pacotes explícitos desta máquina, 13 são do AUR ou locais (`pacman -Qem`). O `suggest` os lista à parte e fora do trecho para colar. O mesmo problema existe no `preset init` e no `apply` (P-18, não corrigido).
- **Não verificado**: o `suggest` na máquina real (com 154 explícitos e um preset pequeno ele lista dezenas de itens, porque não há "ignorados"); nenhum teste de CLI de ponta a ponta (o `main.rs` só liga as peças); a P-18 não foi medida com `apply` e um pacote do AUR.
- **Fica**: o dono rodar `nodusctl preset init > p.toml && nodusctl preset suggest p.toml` (deve dar "Nada fora do preset"); ADR 0016 (a criar, se o dono quiser); fatia seguinte do critério 5 (pergunta interativa, escrita no preset, "ignorar"); P-18; P-02, P-14, P-17; critério 6 (pasta de workspace); fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, PR B verificado no Arch real

- **Pedido**: testar o `preset apply` com Flatpak na máquina real. O PR #46 já estava mesclado (`c490cf4`). Eu não tenho TTY nem build local (ADR 0009), então o dono construiu o binário (`cargo build --release -p nodus-cli`) e rodou o `apply` num terminal; eu conferi só por leitura.
- **Verificado** (Flatpak 1.18.4, `--user`, `flathub` adicionado pelo dono): preset com `org.gnome.Calculator`. O apply mostrou `flatpak install --user -- flathub org.gnome.Calculator`, pediu `Executar? [s/N]`, o Flatpak perguntou pelo runtime `org.gnome.Platform//51` e instalou 8 itens (cerca de 570 MB, somando os downloads). "Depois:" deu "nada a fazer". `history.jsonl` (0600, 1 linha) com `version 3`, `manager flatpak`, `installed` com o ID, `result` ok. `nodusctl history` a listou como `flatpak`. `nodusctl preset plan` de novo: nada a fazer (idempotente). `nodusctl history undo 1`: recusou, "a reversão de Flatpak ainda não existe".
- **Antes disso**: sem o `flathub` no `--user`, o apply pararia antes de qualquer pergunta (só lido em `flatpak remotes --user`, que estava vazio; não rodei o apply nesse estado).
- **Não verificado**: recusa do prompt do Flatpak no fluxo real; falha de rede no meio do download; apply com pacman e Flatpak juntos; outras versões do Flatpak. O app ficou instalado na máquina do dono (remover: `flatpak uninstall --user org.gnome.Calculator`; o histórico mantém a linha).
- **Fica**: P-02 (marca e domínio), P-14, P-17 (bloqueada); fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, Flatpak no `preset apply`, PR B (instalação)

- **Pedido**: seguir com o PR B. Plano aprovado em plan mode (uma confirmação por passo; recusa independente; falha do pacman interrompe; uma entrada de histórico por passo).
- **Feito (PR #46)**: testes primeiro; o CI do commit só de testes ficou **vermelho** (erro de compilação: `entry_from_flatpak`, `flatpak_install_command`, `flathub_configured` e a nova assinatura de `apply` não existiam). Depois a implementação: `flatpak_install_command` e `flathub_configured` (`preset.rs`), `entry_from_flatpak` (`history.rs`), `apply` em dois passos com checagem do remote antes de qualquer pergunta (`apply.rs`), `main.rs`, ADR 0015, README, MVP, OVERVIEW, SECURITY.
- **Verificado**: CI verde na implementação (clippy e testes, `cargo-deny`, `guard`). **Mutação**: `--user` trocado por `--system` no comando Flatpak, empurrado, CI **vermelho** com 6 testes falhando (4 de `apply`, 2 de `preset`); revertida com `git revert`.
- **Não verificado**: uma instalação Flatpak que termina com sucesso (baixaria runtime); o fluxo real no Arch (os testes usam executor falso); o `history.jsonl` real do dono; só Flatpak 1.18.4.
- **Fica**: o dono rodar `nodusctl preset apply` com um app Flatpak pequeno faltando e conferir `nodusctl history`; P-02 (marca e domínio), P-14, P-17 (bloqueada); fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, Flatpak no `preset apply`, etapa 0 do PR B (medição)

- **Pedido**: o dono autorizou a etapa 0 (medir `flatpak install --user`). Fiz só a parte que não instala: `HOME` e `XDG_*` isolados num diretório temporário, `flathub` adicionado só ali, recusa com `n` no prompt. Nenhum app instalado; o diretório foi apagado (7,7 MB de metadados) e o `flatpak list` do dono segue só com `org.nongnu.lingot` (`user`).
- **Medido** (Flatpak 1.18.4): `flatpak remotes --user --columns=name` sem remote dá saída vazia, exit 0; com `flathub`, imprime `flathub`. `flatpak install --user -- flathub org.no.Exist` dá `Nothing matches org.no.Exist in remote flathub`, exit 1, igual ao comando sem `--`: o `--` é aceito antes do remote. `flatpak install --user -- flathub org.gnome.Calculator` com `n` no prompt: o Flatpak pergunta primeiro sobre o runtime (`Do you want to install it? [Y/n]`) e termina com `requires the runtime ... which is not installed`, exit 1.
- **Decide**: comando previsto `flatpak install --user -- flathub <ids...>`; nunca `-y` nem `--noninteractive` (o prompt do Flatpak mostra runtimes e dependências, e pode perguntar mais de uma vez); recusa e erro dão o mesmo exit 1, então o nodus confere o estado depois com `flatpak list`, como no pacman; sem `flathub` na saída de `remotes --user`, parar antes de pedir confirmação.
- **Não verificado**: uma instalação que termina com sucesso (exigiria baixar o runtime); a saída com app já instalado; outras versões do Flatpak. O PR B (código) não começou.
- **Fica**: PR B (comando, passos em sequência no `apply`, checagem do remote, histórico `manager = flatpak`, ADR 0015, docs); o dono conferir `nodusctl history` com as entradas v2 antigas; P-02 (marca e domínio), P-14 (PKGBUILD real depois do MVP; GitHub; manter ou tirar a conferência do `HEAD`), P-17 (bloqueada); fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, Flatpak no `preset apply`, PR A (histórico v3)

- **Pedido**: seguir na Fase 1 (o dono preferiu isso a fechar a Fase 0 formalmente). Plano aprovado em plan mode, em 2 PRs: A = formato do histórico, B = instalação Flatpak. Decisões do dono: `--user`, remote `flathub` fixo, histórico v3 com `manager`.
- **Feito (PR A, #44)**: testes primeiro e CI vermelho (12 testes falham: v3 exige `manager`, ID Flatpak com maiúscula, `undo` recusa Flatpak, ordem dos campos). Depois a implementação: `Manager`, leitura de v1, v2 e v3, validação por gerenciador, `ENTRY_VERSION = 3`, `NotPacman` no `undo`, rótulo `flatpak` no `nodusctl history`, ADR 0014, `SECURITY.md` (T5).
- **Não verificado**: o `history.jsonl` real do dono (testado com linhas literais); nada instala Flatpak ainda (PR B); build e teste só no CI.
- **Fica**: PR B (instalar Flatpak: etapa 0 mede `flatpak install --user` na máquina, com aprovação do dono), P-02 (marca e domínio), P-14 (PKGBUILD real depois do MVP; GitHub; manter ou tirar a conferência do `HEAD`), P-17 (bloqueada); fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, P-03 (spec renomeada para nodus)

- **Pedido**: seguir a P-03. Decisões do dono (as duas recomendadas): componentes viram `nodus Core/Agent/Desktop/Web`, e o cabeçalho ganha uma nota sobre o nome provisório.
- **Feito**: só docs. `SPEC.md` (29 ocorrências de "Universal" trocadas por `Edit` com `replace_all`; `grep` confirma que só resta "Universal" na nota do cabeçalho), `INDEX.md` (tirou o "nome provisório"), ADR 0001 (consequência atualizada), `PENDING.md` (P-03 em Resolvidas).
- **Não verificado**: concordância e tom da spec inteira (conferi só as linhas trocadas por `grep`, não li as 1.228 linhas); links externos para a spec antiga, se houver.
- **Fica**: P-02 (marca e domínio), P-14 (PKGBUILD real depois do MVP; GitHub; manter ou tirar a conferência do `HEAD`), P-17 (bloqueada); Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, P-02 (binário `nodusctl`, ADR 0013)

- **Pedido**: seguir a P-02. Li o `Cargo.toml` do crate `nodus` 0.17.0 (tarball, sem executar): instala o binário `nodus`. O dono escolheu a opção B (manter o projeto, mudar o binário) e o nome `nodusctl`, e manter `~/.local/state/nodus`. Plano aprovado em plan mode.
- **Feito**: testes primeiro (nome do comando, `--version`, cabeçalho do preset). CI do PR #42 no commit só de testes: `Rust (clippy and tests)` **vermelho** (`left: "nodus"`, `right: "nodusctl"` e `--version`), `cargo-deny` e `guard` verdes. Depois a troca (`[[bin]]`, clap, cabeçalho do preset, doc-comments), docs de estado atual (`README.md`, `OVERVIEW.md`, `MVP.md`, `SECURITY.md`) e ADR 0013.
- **Percalço**: o CI não roda em push de branch, só em PR, `main` e disparo manual; a primeira consulta de runs veio vazia e abri o PR #42 como rascunho para medir o vermelho. Uma consulta ao GitHub deu timeout de rede.
- **Não verificado**: marca de `nodusctl`; o `[[bin]]` real (build só no CI); a mutação (trocar de volta o nome e ver o teste falhar) não foi feita separadamente, o vermelho do commit de testes cumpre esse papel; não conferi se o teste do preset também aparece vermelho (a lista que li foi cortada).
- **Fica**: P-02 (marca e domínio), P-14 (PKGBUILD real depois do MVP; GitHub; manter ou tirar a conferência do `HEAD`), P-17 (bloqueada), P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, P-02 (nome mantido, opção A)

- **Pedido**: seguir a P-02. Medi a disponibilidade do nome e apresentei A (manter `nodus`), B (manter com sufixo nos pacotes) e C (renomear já). O dono escolheu a A, a recomendada.
- **Medido** (crates.io, AUR e repositórios do Arch, npm, PyPI, GitHub, DNS de 10 TLDs; só a palavra `nodus` nas consultas): resultado completo na P-02 do `PENDING.md`. O que pesa: o crates.io `nodus` está ocupado por uma CLI "local-first" de outro autor; no AUR está livre.
- **Feito**: só docs. `PENDING.md` (P-02 com decisão, medição e o que resta) e ADR 0001 (atualização das consequências). A P-02 **continua aberta**.
- **Não verificado**: marca (INPI, EUIPO, USPTO) não foi buscada; se o `nodus` do crates.io instala um binário de mesmo nome; dono dos domínios (sem `whois`); a busca web por produto homônimo foi fraca; DNS resolvendo não prova que o domínio está registrado por terceiros.
- **Fica**: P-02 (marca, binário, domínio), P-14 (PKGBUILD real só depois do MVP; GitHub; manter ou tirar a conferência do `HEAD`), P-17 (bloqueada), P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, P-12 (risco aceito, opção A)

- **Pedido**: seguir a P-12. Opções apresentadas: A (aceitar o risco e fechar), B (manter aberta até distribuir), C (parecer jurídico). O dono escolheu a A, a recomendada.
- **Feito**: só docs. `PENDING.md` (P-12 movida para Resolvidas, com a base, os gatilhos de reabertura e as lacunas) e ADR 0007 (atualização na consequência da fronteira). A decisão técnica do ADR não mudou.
- **Base**: GPL FAQ da FSF (entrada sobre "aggregate", lida em 2026-11-15, só os primeiros 100 mil de 156 mil caracteres da página), `process.rs` (chamadas por `Command::new` com argumentos), `deny.toml` (só MIT, Apache-2.0 e Unicode-3.0).
- **Não verificado**: **não é parecer jurídico**; a FSF interpreta, não decide; bibliotecas de sistema linkadas dinamicamente (sem `ldd`); licença instalada de `git`, `pacman` e `flatpak`; resto da página da FSF.
- **Fica**: P-14 (PKGBUILD real só depois do MVP; GitHub; manter ou tirar a conferência do `HEAD`), P-17 (bloqueada), P-02, P-03; Flatpak no `preset apply`; fechamento formal da Fase 0. A P-14 deixa de depender da P-12.

### 2026-11-15 — Sessão 19, P-14 (casos de desvio de commit)

- **Pedido**: seguir com a opção 2 (caso de desvio de commit), depois do #39. Li `extract_git` em `/usr/share/makepkg/source/git.sh` antes de montar os casos.
- **Medido** (`makepkg` 7.1.0, git 2.56.0, `git+file://`, chave descartável sem passphrase, `HOME` e `SRCDEST` isolados, script no scratchpad; diretório temporário apagado): controle passa; tag forjada (objeto assinado de `v1` publicado como `v2`): `git verify-tag` sozinho passa, mas o `makepkg` aborta antes do `prepare()` (`the git tag has been forged`); tag leve: falha no `prepare()`, exit 4; tag movida e reassinada: build limpo pega o commit novo, rebuild no mesmo `srcdir` mantém o commit antigo e passa sem aviso.
- **Achado**: a conferência do `HEAD` (item 3 do ADR 0012) nunca disparou e não achei como montar um desvio; com `#tag=` o `makepkg` faz checkout da própria tag. Parece redundante nesta versão, mas a decisão de manter ou tirar é do dono. A proteção contra tag forjada é do `makepkg`, não do nosso `prepare()`.
- **Feito**: só docs, no mesmo PR #39 (mesma intenção e mesmos arquivos). `PENDING.md` (P-14) e ADR 0012 (item "Não verificado").
- **Não verificado**: só clone local (GitHub não); só `makepkg` 7.1.0; `-S` a partir do AUR; o rebuild com `srcdir` velho foi medido com `makepkg` direto, não com helper.
- **Fica**: P-14 (PKGBUILD real só depois do MVP; GitHub), P-17 (bloqueada), P-12, P-02, P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-15 — Sessão 19, P-14 (opção 1 medida com `yay` e `paru`)

- **Pedido**: seguir a P-14. Opções apresentadas: 1 (AUR helper), 2 (caso de desvio de commit), 3 (GitHub real, com tag irreversível; não recomendada). O dono escolheu a 1 e autorizou rodar `yay` e `paru` em `/tmp`.
- **Medido** (`yay` 13.1.0, `paru` 2.2.0, `-B <pasta>` sem instalar; PKGBUILD de teste com o `prepare()` da opção 1; chaves descartáveis sem passphrase; `HOME` isolado; script no scratchpad da sessão, diretório temporário apagado por ele): tag da chave confiada, pacote gerado e exit 0 nos dois e no `makepkg` de controle; tag de outra chave, `prepare()` falha (exit 4) e nenhum pacote, nos dois; com `--mflags=--noprepare`, os dois geram o pacote da tag de outra chave sem verificar.
- **Achado**: a fraqueza (a) do ADR 0012 vale também para quem usa helper. O ADR e o README do pacote devem avisar; a decisão não mudou.
- **Percalço**: o script falhou três vezes antes de medir (flag `--nodiffmenu` que não existe no `yay`; `yay -B` exige a pasta como repo git e com remoto/upstream, então montei um remoto bare local). Nenhum desses defeitos afeta o resultado; as execuções que contam foram as da última rodada.
- **Feito**: só docs. `PENDING.md` (P-14) e ADR 0012 (item "Não verificado").
- **Não verificado**: fluxo `-S` a partir do AUR (clona e mostra o diff do PKGBUILD); o aviso `Skipping verification of source file PGP signatures` do `yay` não foi investigado; config real do dono; GitHub; outras versões; caso de desvio de commit; PKGBUILD real.
- **Fica**: P-14 (PKGBUILD real só depois do MVP; caso de desvio e GitHub), P-17 (bloqueada), P-12, P-02, P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, ADR 0012 (verificação da tag no AUR)

- **Pedido**: o dono aceitou o ADR sugerido para a decisão da P-14 e disse para seguir na P-14 depois.
- **Feito**: só docs. `adr/0012-verificacao-da-tag-no-aur.md` (decisão com 6 itens, alternativas, consequências, não verificado); o ADR 0010 ganhou um ponteiro para o 0012 no item 2, sem mudar a decisão dele; `PENDING.md` (P-14 aponta para o ADR).
- **Fica**: o PKGBUILD real (só depois do MVP, depende da P-02 e da P-12); repetir a medição contra o GitHub e com um AUR helper antes de publicar; caso de desvio de commit sem teste.
- **Não verificado**: tudo o que a P-14 já listava; o ADR foi escrito por IA e não teve revisão externa.
- **Fica**: P-14, P-17 (bloqueada), P-12, P-02, P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, P-14 decidida (opção 1)

- **Pedido**: a decisão que faltava na P-14. Opções apresentadas: 1 (`prepare()` com `git verify-tag`), 2 (tag GPG, refaz a P-13), 3 (sem verificação no `makepkg`). O dono escolheu a 1, a recomendada.
- **Feito**: só docs, `PENDING.md` (P-14 com a decisão e o que resta). O ADR 0010 não foi alterado.
- **Fica na P-14**: o PKGBUILD real, só depois do MVP; nota no README do pacote sobre `--noprepare`; repetir a medição contra o GitHub e com um AUR helper antes de publicar; depende da P-02 e da P-12.
- **Não verificado**: o mesmo da medição (GitHub, AUR helper, outras versões do `makepkg`, caso de desvio de commit).
- **Sugestão em aberto**: registrar a decisão como ADR (muda o mecanismo do item 2 do ADR 0010); não criei, pergunto antes (AGENTS.md §11).
- **Fica**: P-14 (implementação), P-17 (bloqueada), P-12, P-02, P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, P-14 opção 1 medida (`prepare()` com `git verify-tag`)

- **Pedido**: seguir a P-14; o dono disse "p14". Medi a opção 1, a recomendada e ainda sem medição.
- **Medido** (PKGBUILD de teste em `/tmp`, `git+file://`, chaves descartáveis, `makepkg --nobuild --nodeps`, script no scratchpad; makepkg 7.1.0, git 2.56.0): tag da chave confiada, `prepare()` passa e o `HEAD` confere, exit 0; tag de outra chave, `No principal matched`, exit 4; tag sem assinatura, `no signature found`, exit 4. Diretório temporário apagado pelo script.
- **Achado**: a opção 1 verifica de verdade, mas `makepkg --noprepare` pula o `prepare()` (confirmado em `makepkg --help`) e `--verifysource` sozinho não verifica; o `?signed` nativo não tem esse furo, mas não aceita a chave SSH. A raiz de confiança fica no PKGBUILD do AUR.
- **Percalço**: a 1ª versão do script tinha dois defeitos (o `PIPESTATUS` dentro do subshell não devolvia o exit do `makepkg`, e um caso de "desvio de commit" que eu não montava de fato); corrigi antes de rodar. O caso de desvio ficou sem teste, pois o `makepkg` faz checkout da própria tag.
- **Feito**: só docs, `PENDING.md` (P-14). A decisão do ADR 0010 não mudou; a escolha entre as 3 opções segue com o dono.
- **Não verificado**: contra o GitHub (clone local), AUR helper, outras versões do `makepkg`; o PKGBUILD real; a opção 2 (GPG) não foi medida (o controle GPG da medição anterior mostra que o `?signed` nativo funciona com ela).
- **Fica**: P-14 (decidir; PKGBUILD real só depois do MVP), P-17 (bloqueada), P-12, P-02, P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, P-14 medida (`makepkg` e tag SSH)

- **Pedido**: seguir a P-14 depois do achado por leitura de código; fiz a etapa 0 (medir). O dono disse só "p14"; assumi que era o aval para medir, já que eu tinha avisado que pediria aprovação, e o prompt de permissão do `Bash` cobriu a execução.
- **Medido**: PKGBUILD de teste em `/tmp` (`git+file://`, chaves descartáveis sem passphrase, `GNUPGHOME` temporário, script no scratchpad da sessão; `makepkg` 7.1.0, git 2.56.0, gpg 2.4.9). Controle com tag GPG: `makepkg --verifysource` deu `Passed`, exit 0. Tag SSH (o `git verify-tag` direto dá exit 0): `SIGNATURE NOT FOUND` e `ERROR: One or more PGP signatures could not be verified!`, exit 1. Confirma a leitura do código; o diretório temporário foi apagado pelo script.
- **Feito**: só docs. `PENDING.md` (P-14 com o medido) e ADR 0010 (item "Não verificado"). A decisão do ADR não mudou.
- **Não verificado**: a opção 1 (`prepare()` com `git verify-tag` e `allowed_signers` como `source`) não foi medida; contra o GitHub (só clone local), AUR helper (`yay`, `paru`) e outras versões do `makepkg`; o AUR continua sem revisão.
- **Fica**: P-14 (escolher entre as 3 opções e, se for a 1, medir; o PKGBUILD real só depois do MVP), P-17 (bloqueada), P-12, P-02, P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, P-14 (achado: `makepkg` não verifica tag SSH)

- **Pedido**: começar a P-14. O MVP ainda não fechou, então só o item de leitura da P-14 ("conferir como o `makepkg` verifica a assinatura da tag") foi feito.
- **Achado (leitura de `/usr/share/makepkg/integrity/verify_signature.sh`, pacman 7.1.0, git 2.56.0)**: `git+...#tag=X?signed` roda `git verify-tag --raw X` e filtra a saída com `grep -E "(.*SIG| TRUST_.*)"`; só linhas de status do GnuPG passam, e o resultado é comparado com `validpgpkeys` (GPG). A saída da assinatura SSH (`Good "git" signature ...`) não tem esse formato, então a tag da P-13 provavelmente não passa. O ADR 0010 já marcava esse ponto como não verificado.
- **Feito**: só docs. `PENDING.md` (P-14 com o achado, 3 opções e a recomendação) e ADR 0010 (item "Não verificado" atualizado; a decisão do ADR não mudou). O dono respondeu "sim" ao registro do achado; assumi que não pediu a medição agora.
- **Não verificado**: tudo acima é leitura de código, **não rodei o `makepkg`**; o comportamento real com tag SSH precisa da medição (etapa 0 da P-14, PKGBUILD de teste com chave descartável em `/tmp`).
- **Fica**: P-14 (decisão sobre as 3 opções e a medição), P-17 (bloqueada), P-12, P-02, P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, status do README

- **Pedido**: atualizar o `README.md`, que ainda dizia "pré-projeto" (lacuna anotada nas entradas da P-13 e P-17).
- **Feito**: só o parágrafo de status. Lista os 5 comandos que existem (`inventory`, `updates`, `repos`, `preset init|plan|apply`, `history` e `history undo`), diz que não há tag nem canal de instalação, e aponta o ADR 0010. Conferi a lista contra o `enum Command` em `nodus-cli/src/main.rs` e o `OVERVIEW.md`.
- **Não verificado**: não rodei os comandos (build e teste só no CI); as descrições vêm do `OVERVIEW.md`, não de execução; que o `preset plan` leia apps Flatpak (conferido só no `OVERVIEW.md`, não no código).
- **Fica**: P-17 (bloqueada), P-14, P-12, P-02, P-03; Flatpak no `preset apply`; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, P-17 (decisão: esperar o provider Git)

- **Pedido**: a decisão que ficou em aberto na P-17, storage permanente ou deletável para a segunda cópia da chave. Opções apresentadas: esperar o provider Git, Arweave agora, ou pendrive offline sem truthid. O dono escolheu a 1 (esperar o provider Git).
- **Feito**: só docs, `PENDING.md` (P-17 bloqueada até o `GitStorageProvider` do truthid existir, critério 3 reescrito).
- **Motivo**: um blob publicado no Arweave não se apaga, e a chave é a raiz de confiança das releases sem rotação desenhada; uma passphrase fraca ou vazada não teria remédio. O blob estaria cifrado duas vezes (passphrase da chave e chave do Vault), o que reduz o risco, mas não medi quanto nem auditei o Vault.
- **Não verificado**: o estado atual do provider Git no truthid (só li os docs); o Vault aceitar o arquivo da chave.
- **Fica**: P-17 (bloqueada), P-14, P-12, P-02, P-03; `README.md` ainda diz "pré-projeto". A chave segue com uma cópia só, no Bitwarden.

### 2026-11-14 — Sessão 18, P-17 reescrita (segunda cópia, não migração)

- **Pedido**: seguir a P-17 (migrar o backup da chave de assinatura para o truthid). Escolha do dono: opção A, reescrever como segunda cópia.
- **Achado (leitura do `truthid/project`)**: o Vault já existe (Fase 13, concluída, validada em hardware; guarda senhas, campos de cartão e documentos), então "quando existir" não valia mais. O storage estava em transição (IPFS para Arweave; `GitStorageProvider` em avaliação no P90) e a rotação de chave do Vault (P89) tinha correção no código mas "não validada em runtime real".
- **Decisão**: não migrar. Migrar trocaria uma cópia única por outra, criaria dependência circular (a raiz de confiança das releases dependeria de um sistema ainda em mudança) e o Arweave é permanente (não se apaga um blob cifrado publicado). O Bitwarden fica e o Vault vira segunda cópia, com critério de pronto na P-17.
- **Aberto**: o dono não respondeu se aceita storage permanente (Arweave) ou espera o provider Git; ficou registrado como decisão em aberto na P-17.
- **Não verificado**: se o Vault aceita guardar o arquivo da chave como documento; restauração a partir dele; o estado atual do truthid (li só os docs, com `PENDING.md` de 2026-09-19; não abri o app nem o código).
- **Fica**: P-17 (agora com critério), P-14, P-12, P-02, P-03; `README.md` ainda diz "pré-projeto".

### 2026-11-14 — Sessão 18, backup da chave de assinatura (P-13)

- **Pedido**: o backup da chave privada, lacuna deixada na P-13. Escolha do dono: Bitwarden agora (usa o gerenciador), migrar para o truthid depois.
- **Feito**: o dono guardou a chave no Bitwarden e restaurou num terminal. Só docs: `PENDING.md` (P-13 atualizada, P-17 nova). A chave privada nunca passou pela conversa.
- **Medido (rodado pelo dono)**: a 1ª cópia só tinha o corpo da chave, sem as linhas `BEGIN` e `END` (6 linhas, 394 bytes; o original tem 8 linhas, 464 bytes), e o `ssh-keygen -l` deu `not a public key file`. Com as bordas, `-l` deu o fingerprint esperado e `ssh-keygen -y` (pede a passphrase) devolveu a chave pública do README.
- **Achado**: o `-l` lê só a parte pública, sem cifra; ele sozinho não prova que a privada chegou inteira. O `-y` com a passphrase é o teste que vale.
- **Percalço**: `cat > arquivo` com `!` não tem entrada interativa e ficou preso; o dono saiu com Ctrl+C. Meu diagnóstico com `tail -1` imprimiu um trecho do corpo cifrado da chave (sem as bordas a última linha não era texto fixo); sem risco prático, mas não repetir.
- **Não verificado**: se o item do Bitwarden foi corrigido para guardar as bordas (o dono disse "tudo certo"); recuperação da conta do Bitwarden; qualquer cópia fora dele.
- **Fica**: P-17, P-14, P-12, P-02, P-03; `README.md` ainda diz "pré-projeto"; primeira tag só depois do MVP.

### 2026-11-14 — Sessão 18, P-13 (chave de assinatura de tags)

- **Pedido**: resolver a P-13 antes da primeira tag (ADR 0010). Escolhas do dono: chave nova só para assinar (não reusar a do GitHub); o dono roda os comandos de chave e eu guio.
- **Feito**: chave `~/.ssh/id_ed25519_nodus_signing` (fingerprint `SHA256:KNurUVWF0KhmHv42A3OYw6spZenOE0DrQCZ0jjNFnxI`), cadastrada como signing key (`gh api user/ssh_signing_keys` listou, igual à `.pub`); config local do repo (`gpg.format=ssh`, `user.signingkey`, `tag.gpgsign=true`, global vazia); `README.md` com a chave, o segundo canal (API pública do GitHub) e como verificar. Só docs e config, sem código.
- **Percalço**: o `!` não tem TTY, então o `ssh-keygen` gerou a chave **sem passphrase** (`ssh_askpass: No such file`); confirmei com `ssh-keygen -y -P ""`. O dono adicionou a passphrase num terminal (`ssh-keygen -p`, mesmo fingerprint) antes de qualquer cadastro; reconferido.
- **Medido (repo descartável, `HOME` limpo, rodado pelo dono)**: chave publicada, `Good "git" signature`, exit 0; `allowed_signers` com outra chave e vazio, `Good signature ... No principal matched`, exit 1; tag com mensagem trocada, `incorrect signature`, exit 1. Achado: o git imprime `Good` mesmo recusando, então o README manda olhar o exit code.
- **Não verificado**: máquina realmente limpa; nenhuma tag real; o trecho de verificação do README rodou só na parte que gera o `allowed_signers` (formato e fingerprint conferidos), não de ponta a ponta contra uma tag real; verificação pelo `makepkg`/`cargo install --tag` (P-14); rotação e revogação; backup da chave privada.
- **Fica**: P-14, P-12, P-02, P-03; o `README.md` ainda diz "pré-projeto" (desatualizado, fora do escopo); fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, nota no `nodus history` (PR #28)

- **Pedido**: o aviso de que as dependências não aparecem no histórico (seguimento da P-16). Assumi: uma linha no fim, só no texto (não no `--json`), só com histórico não vazio.
- **Feito**: `HISTORY_NOTE` em `render_history` (`nodus-cli/src/render.rs`). Sem dependência nova, sem mudança de formato.
- **CI (resultado real, PR #28)**: commit 1 (só testes, `b016571`) falhou 7 testes (2 novos e 5 ajustados) e passaram 117, sem erro de compilação; commit 2 (`044263d`) passou de primeira. Mutação (nota dentro do laço, `2ff8246`): falhou 3 testes (121 passaram), o de "uma vez só" entre eles; os de uma entrada só passaram, então só os de várias entradas pegam essa mutação. Revertida em commit próprio (`d7c4e21`), código idêntico ao de `044263d`.
- **Percalço**: na reversão, o `git revert -q` não rodou e o `--amend` seguinte reescreveu o commit da mutação só localmente; o push foi recusado, então o remoto não foi afetado. Voltei a local ao remoto e refiz o revert.
- **Não verificado**: a saída real de `nodus history` no Arch (só o CI rodou os testes).

### 2026-11-14 — Sessão 18, P-16 (histórico é log de intenção)

- **Pedido**: decidir a P-16. Opções apresentadas: A aceitar e documentar, B `also_removed` só na remoção (v3), C efeito real nos dois lados (v3). O dono respondeu "sim" à recomendação (A); assumi isso e ele pode corrigir.
- **Feito**: só docs. ADR 0011 ganhou o item 8 (log de intenção, B e C descartadas); `SECURITY.md` deixou de dizer que o histórico registra "o que mudou"; P-16 movida para resolvidas. Sem código nem teste, então sem mutação.
- **Não verificado**: nenhuma opção de log de efeito foi medida ou implementada; a saída de `nodus history` não avisa que as dependências não aparecem (ficou fora do escopo, é mudança de código).
- **Fica**: banco do pacman travado; `--json` com `undo`; Flatpak; P-13, P-14, P-12, P-02, P-03; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, verificação real do `history undo`

- **Pedido**: registrar a verificação que o dono fez no Arch (o terminal dele caiu depois; o log foi colado na conversa).
- **Medido (Arch real, log do dono)**: `preset apply` instalou `abi-dumper` (arrastou `ctags` e `vtable-dumper`); `history undo 1` mostrou o plano de 3 pacotes, confirmou, rodou `sudo pacman -Rs -- abi-dumper` e removeu exatamente os 3. `nodus history` listou `[2] desfez [1] ... removidos: abi-dumper`. O `pacman -Qq | grep` seguinte não listou nada (pacotes ausentes).
- **Achado**: o histórico registra 1 removido, mas 3 saíram (dependências órfãs). Virou P-16, sem decisão. Há também duas confirmações seguidas (a do nodus e a do pacman); mantida, pois o ADR 0011 proíbe `--noconfirm`.
- **Feito**: só docs (`PENDING.md` P-15 e P-16, `OVERVIEW.md`, `MVP.md`). Sem código, sem teste novo, então sem mutação.
- **Não verificado**: banco do pacman travado; `--json` com `undo`; Flatpak; o log é colado pelo dono, não rodei nada.
- **Fica**: P-16; Flatpak no `preset apply`; decisão sobre branches extras no `repos`; P-13, P-14, P-12, P-02, P-03; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18, continuação (fatia 4, PR 2 de 2: `nodus history undo <n>`)

- **Pedido**: o comando privilegiado do ADR 0011. Escolhas do dono: stderr por função nova em `process.rs`; 5 linhas de saída quando a simulação falha; `--json` ignorado no `undo`. PR #24 (formato v2) já mesclado.
- **Etapa 0 (medição, dono rodou `preset apply` e eu li o resto)**: `abi-dumper` arrasta `ctags` e `vtable-dumper`; `-Rs --print` lista os 3 (ADR 0011 item 5 confirmado). Achado: com dependência quebrada as 979 linhas saem no **stdout**, não no stderr; o desenho do ADR (stdout capturado, stderr herdado) escondia o motivo, então a CLI mostra 5 linhas do stdout e `(+N linhas)`. Versão com época (`1:6.2.1-1`).
- **Feito**: `nodus-core/src/undo.rs` (candidatos, comandos, parser do plano, `entry_for_undo`), `nodus-cli/src/undo.rs` (orquestração), `process::run_capture`, subcomando `history undo <n>`. Sem dependência nova.
- **CI (resultado real, PR #25)**: commit 1 (só testes) falhou por erro de compilação (módulos inexistentes); commit 2 passou de primeira. Mutação 1 (tirar o `--` dos comandos) falhou 5 testes (2 + 3); mutação 2 (ignorar `-Qe`) falhou 2 (1 + 1); ambas revertidas em commits próprios.
- **Não verificado**: a **remoção real** pelo `history undo` no Arch (precisa de terminal interativo; o `!` do prompt não serve); banco do pacman travado; `--json` junto de `undo` é aceito e ignorado; o `-Rs` real remove o que o `--print` listou (assumido).
- **Fica**: verificação real do `undo` com `abi-dumper` (instalado agora, é o pacote de teste); Flatpak no `preset apply`; decisão sobre listar branches extras no `repos`; P-13, P-14, P-12, P-02, P-03; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 18 (fatia 4, PR 1 de 2: histórico v2 e número de linha)

- **Pedido**: começar o `history undo` (ADR 0011). Escolhas do dono no plano: 2 PRs (formato primeiro, comando privilegiado depois); stderr da simulação por função nova em `process.rs` (vale para o PR 2). Aceitou gravar instalações também em v2.
- **Antes**: sessão começou com a `main` local defasada (sem `git fetch`); PR #23 (docs redundantes) aberto por engano e fechado sem mesclar.
- **Feito**: `Action` (`install`/`remove`) e `undoes` em `HistoryEntry`; `ENTRY_VERSION = 2`; `parse_line` lê v1 (como instalação) e v2, recusando combinação inválida; `nodus history` numera as linhas (`[n]`) e mostra `desfez [n]` nas remoções. Sem dependência nova.
- **CI (resultado real, PR #24)**: commit 1 (só testes) falhou por erro de compilação (`Action`, `action`, `undoes` inexistentes); commit 2 passou de primeira. Mutação `undoes >= 1` → `>= 0` barrada pelo clippy (não vale como prova); `>= 2` **sobreviveu** (lacuna: nenhum teste lia `undoes = 1`); com o teste do limite adicionado, a mutação falhou 1 teste (139 passaram) e foi revertida em commit próprio.
- **Não verificado**: `nodus history` no Arch real com um histórico v1 existente; o PR 2 (`history undo`) não existe; a medição de `pacman -Rs` com dependência (etapa 0 do PR 2) não foi feita.
- **Fica**: PR 2 (`history undo <n>`, privilegiado, com etapa 0 de medição); `git fetch` no início da sessão; Flatpak no `preset apply`; decisão sobre listar branches extras no `repos`; P-13, P-14, P-12, P-02, P-03; fechamento formal da Fase 0.

### 2026-11-14 — Sessão 17 (P-15: semântica da reversão, só decisão)

- **Pedido**: resolver a P-15. Escolhas do dono no plano: só a decisão em ADR agora, com o código (`history undo`) no PR seguinte; validação por confirmar + simular + só o que o nodus instalou (sem cruzar com o `pacman.log`).
- **Antes**: PR #21 (fatia 3) mesclado em `c24d184`; CI da `main` verde.
- **Medido (pacman 7.1.0, só leitura)**: `pacman -Rs --print --print-format '%n %v' -- alsa-firmware` roda sem root e dá exit 0 com `alsa-firmware 1.2.4-4`; `glibc` falha com exit ≠ 0 (`breaks dependency … required by 7zip`); pacote inexistente dá `target not found`, exit 1; `pacman -Qi` mostra `Install Reason : Explicitly installed`.
- **Feito**: ADR 0011 (`history undo <n>`; candidatos = `installed` ∩ ainda instalado ∩ explícito; plano = `--print`; execução `sudo pacman -Rs --`, nunca `-n`/`-c`/`-d`/`--noconfirm`; entrada nova no histórico, formato v2). `SECURITY.md` (§4 e T5), `PENDING.md` (P-15 resolvida), `MVP.md` e `OVERVIEW.md`. Sem código.
- **CI**: PR #22, no commit `db93f42` (conferido pelo `headSha`): `guard`, `Rust (clippy and tests)` e `cargo-deny` verdes (só docs; sem teste novo, então sem mutação).
- **Não verificado**: a semântica de `-s` com um pacote que arrasta dependências órfãs (a man page não está instalada; só medi pacote sem dependências); a simulação com vários alvos e com o banco do pacman travado; revisão externa (nenhuma).
- **Resíduo aceito**: o histórico não é assinado, então uma linha plantada pedindo a remoção de um pacote explícito passa na validação; só a leitura do plano pelo dono a barra.
- **Fica**: fatia 4 (implementar `history undo`, medindo `-s` com dependências antes); verificação da fatia 3 no Arch pelo dono; instalar Flatpak; dotfiles; critérios 5 e 6; P-13, P-14, P-12, P-02, P-03; fechamento formal da Fase 0; conferir a aba Security (P-07).

### 2026-11-14 — Sessão 16 (Fase 1: histórico, fatia 3, `nodus history`)

- **Pedido**: critério 10 do MVP, parte de registrar. Escolhas do dono no plano: só registrar e listar (reversão vira a fatia 4, P-15); registrar só os pacotes pedidos (sem diff completo nem dependências); linha inválida no meio do histórico é erro; permissão mais larga que 0600 é recusada.
- **Antes**: PR #20 (P-08) mesclado em `31db1e8`; CI da `main` verde.
- **Feito**: `nodus-core/src/history.rs` (`HistoryEntry` com `version = 1`, `entry_from`, linha JSON estrita, `format_utc` sem crate de datas); na CLI, `history.rs` (caminho XDG, `append` 0700/0600 sem symlink, `read` com limite de 8 MiB), `apply_with_history` (grava quando o comando rodou, inclusive na falha), `nodus history [--json]`, `render_history` e `testutil.rs`. `preset_apply` resolve o caminho antes de executar. `serde_json` passou de dev-dependency para dependency do core (já era do workspace; **nenhum crate novo**, `Cargo.lock` intacto).
- **CI (resultado real, PR #21)**: commit 1 (só testes) falhou por erro de compilação (`entry_from`, `to_line`, `parse_line`, `format_utc`, `ExecResult`, `HistoryEntry`, `history_path`, `append`, `read`, `Applied`, `render_history`, `MAX_HISTORY_BYTES` e `Command::History` inexistentes); `cargo-deny` e `guard` verdes. Commit 2 passou de primeira (130 no core, 100 na CLI, clippy limpo). Mutação dupla: (A) arquivo nascendo `0644` e (B) `apply` gravando sem executar derrubaram 10 testes juntos, mas a A mascarava a B (arquivo 0644 faz toda leitura falhar); revertida só a A, a B sozinha derrubou exatamente 4 (`nao_grava_na_recusa_nem_consulta_o_depois`, `nao_grava_com_plano_vazio`, `nao_grava_quando_so_falta_flatpak`, `nao_grava_sem_terminal`) e 96 passaram. Revertidas em commit próprio; o código final é idêntico ao do commit 2 (`git diff` vazio) e o CI voltou ao verde.
- **Percalço**: um `git push` encadeado num comando que estourou o tempo não chegou ao remoto, e li o run do commit anterior como se fosse o novo (lista idêntica de falhas). Percebi pela mesma lista e conferi o `headSha` do run.
- **Não verificado**: a execução real no Arch (permissão `0600` de verdade, `sudo pacman` instalando e `nodus history` mostrando); o caminho com `XDG_STATE_HOME` real (os testes passam o ambiente como argumento); `nodus history --json` (sem teste da saída); a data gravada é a do início do `apply`, antes da confirmação.
- **Fica**: verificação no Arch pelo dono; reversão (P-15); instalar Flatpak (formato v2); dotfiles; critérios 5 e 6; P-13, P-14, P-12, P-02, P-03; fechamento formal da Fase 0; conferir a aba Security (P-07).

### 2026-11-14 — Sessão 15 (P-08: distribuição e integridade, só decisão)

- **Pedido**: resolver a P-08. Escolha do dono no plano: primeiro canal por **código-fonte** (AUR/cargo); binário descartado por ora.
- **Antes**: PR #19 (P-07) mesclado em `28579c0`; os 3 checks do PR passaram. Na hora de abrir o PR, o CI da `main` já tinha terminado verde.
- **Levantamento**: não existe workflow de release, tag nem binário; o git local não assina nada (`commit.gpgsign` e `tag.gpgsign` vazios, último commit sem assinatura).
- **Feito**: ADR 0010 (tag Git assinada por SSH, `sha256sums` no PKGBUILD, `Cargo.lock` com `--locked`, sem `self-update`, binário só com ADR novo). `SECURITY.md` T9 e §7, `PENDING.md` (P-08 resolvida; P-13 chave de assinatura e P-14 PKGBUILD abertas), `MVP.md` critério 1 e `OVERVIEW.md`. Sem código.
- **CI**: PR #20, no commit de código: `guard`, `Rust (clippy and tests)` e `cargo-deny` verdes (só docs; sem teste novo, então sem mutação). O CI da `main` depois do merge do #19 também fechou verde (`CI` e `CI Private Files`).
- **Não verificado**: como `makepkg` e `cargo install --tag` verificam a assinatura da tag; se o GitHub aceita a chave SSH do dono como signing key; revisão externa (nenhuma).
- **Fica**: P-13, P-14, P-12, P-02, P-03; instalar Flatpak (formato v2); histórico e reversão (fatia 3); dotfiles; critérios 5 e 6; fechamento formal da Fase 0; conferir a aba Security (P-07).

### 2026-11-14 — Sessão 14 (P-07: política de reporte de vulnerabilidade)

- **Pedido**: resolver a P-07. Escolhas do dono no plano: private vulnerability reporting (PVR) do GitHub como canal (e-mail descartado); `SECURITY.md` em inglês curto; melhor esforço, sem SLA.
- **Antes**: PR #18 (preset, fatia 2) mesclado em `efd3090`.
- **Feito**: PVR ligado com `gh api -X PUT repos/Arven-Labs/nodus/private-vulnerability-reporting` (antes `enabled: false`; o GET depois deu `enabled: true`). `.github/SECURITY.md` novo; `project/SECURITY.md` §8 aponta para ele; P-07 resolvida no `PENDING.md`; `OVERVIEW.md` atualizado (PR #18 mesclado). Sem código.
- **CI**: PR #19, no commit de código: `guard`, `Rust (clippy and tests)` e `cargo-deny` verdes (só docs; nenhum teste novo, então não há mutação a fazer).
- **Não verificado**: a aba Security mostrar a política e o botão "Report a vulnerability" (UI; só o dono vê); as notificações de advisory do dono estarem ativas; se o GitHub reconhece o arquivo só depois do merge na `main`.
- **Fica**: P-08, P-12, P-02, P-03; instalar Flatpak (formato v2); histórico e reversão (fatia 3); dotfiles; critérios 5 e 6; fechamento formal da Fase 0.

### 2026-11-13 — Sessão 13 (Fase 1: preset, fatia 2, `preset apply` do pacman)

- **Pedido**: executar o plano do preset (critérios 4 e 9), a primeira operação privilegiada do nodus. Escolhas do dono no plano: só pacman (Flatpak fica para o formato v2, com remote e escopo); confirmação do nodus **e** a do pacman; histórico e reversão (critério 10) numa fatia própria.
- **Antes**: PR #17 (fatia 1) mesclado em `ecb315b`, CI verde na `main` e verificado no Arch (ver a sessão 12).
- **Feito**: `pacman_install_command` no core (`sudo pacman -S --needed -- <faltantes>`; nunca `--noconfirm` nem `-y`); `apply.rs` na CLI (`confirm`, que só aceita `s`/`sim` e trata vazio e fim da entrada como não; `apply`, com o executor como closure, sem trait); `run_interactive` em `process.rs` (terminal herdado, só o status); `preset apply <arquivo>`, que recusa sem terminal interativo e recalcula o plano depois de executar. Sem dependência nova. `SECURITY.md` §4 registra o que foi implementado.
- **CI (resultado real, PR #18)**: commit 1 (só testes) falhou por erro de compilação (`Outcome`, `confirm`, `apply`, `run_interactive`, `pacman_install_command` e a variante `Apply` inexistentes); commit 2 passou de primeira (114 no core, 74 na CLI). Mutação tripla, as três pegas por testes diferentes: resposta vazia confirmando (`qualquer_outra_resposta_e_nao`, `recusado_nao_executa`, `fim_da_entrada_e_recusa`); comando sem `--` (`comando_de_instalacao_e_exato`, `confirmado_executa_o_comando_exato_uma_vez`); plano vazio gerando comando (`sem_pacote_faltando_nao_ha_comando`, `plano_vazio_nao_pergunta_nem_executa`, `so_flatpak_faltando_avisa_e_nao_executa`). Revertida em commit próprio.
- **Não verificado no CI**: a execução real (`sudo` e `pacman` não existem lá; os testes cobrem o comando montado, a confirmação e o fluxo com um executor falso). Ver a verificação no Arch abaixo.
- **Verificado no Arch antes do merge (branch `preset-fatia2`, 2026-11-14)**: rodando com `!` no Claude Code (stdin sem TTY), as três chamadas pararam em "precisa de terminal interativo" depois de mostrar o comando, sem perguntar nem chamar o `sudo`: o caminho "sem terminal, recusa" provado numa máquina real. Num terminal normal o dono rodou a sequência `n`, `s`, de novo e `sudo pacman -Rns sl`, e relatou que deu certo (as saídas de `n`, do "Depois:" e da terceira rodada **não foram coladas**; o que eu conferi foi o `/var/log/pacman.log`: `installed sl (5.05-6)` às 00:13:05 e `removed sl (5.05-6)` às 00:13:26, e `pacman -Q sl` não acha o pacote depois). Continuam sem prova: recusa no `[Y/n]` do pacman e banco do pacman travado.
- **Fica**: instalar Flatpak (formato v2); histórico e reversão (fatia 3, critério 10); dotfiles; critérios 5 e 6; P-12, P-02, P-03, P-07, P-08; fechamento formal da Fase 0.

### 2026-11-13 — Sessão 12 (Fase 1: preset, fatia 1, `init` e `plan` só leitura)

- **Pedido**: começar o preset (critérios 3, 4 e 9). Escolhas do dono no plano: fatia 1 só com pacotes e só leitura (execução na fatia 2); formato TOML; `init` com os explícitos (`pacman -Qe`) e os apps Flatpak; `init` avisa na saída de erro quando falta o Flatpak.
- **Antes**: PR #16 (P-11) mesclado em `6402f21`; `CI` (com `cargo-deny`) e `CI Private Files` verdes na `main`.
- **Medição (etapa 0)**: `pacman -Qe` sai no mesmo formato `nome versão` do `-Q` (154 explícitos contra 1244 no total, exit 0), e todos os nomes reais batem com a regra de validação; nenhum app Flatpak instalado na máquina.
- **Feito**: `nodus-core/src/preset.rs` (`Preset`, `parse_preset` com `deny_unknown_fields`, `version = 1` obrigatório e validação de nomes, `to_toml` com um nome por linha, `preset_from_machine`, `plan` puro, `collect_plan`); `read_flatpak_apps` extraído de `inventory::collect` sem mudar comportamento; CLI `preset init` e `preset plan <arquivo> [--json]`, `preset_file.rs` (limite de 1 MiB lido com `take`), `render_plan` e `init_warning`. Dependência nova: `toml` 1.2.1 (só no core; o dono atualizou o lock com `cargo metadata`, que só adicionou os 9 pacotes da árvore do `toml`).
- **CI (resultado real, PR #17)**: commit 1 (só testes) falhou por erro de compilação (`parse_preset`, `plan`, `read_preset`, `render_plan`, `PresetAction` e outros inexistentes), com o `cargo-deny` já verde com o `toml`; commit 2 passou de primeira (111 no core, 58 na CLI, clippy limpo). Mutação dupla: `plan` com `-Qe` falhou 2 testes (`plan_da_maquina_considera_todos_os_pacotes_instalados`, `falha_do_pacman_vira_erro`); **aceitar `-` no começo do nome não foi pego**, porque `-Rns` e `--overwrite=*` já eram recusados por maiúscula e `=`. Acrescentei `--noconfirm` e `-rdd` ao teste ainda com a mutação aplicada, e o CI falhou com `"--noconfirm" deveria ser recusado`; depois as mutações foram revertidas e o CI voltou ao verde.
- **Achado de passagem**: o `cargo metadata` baixou `ntapi` e `objc2*`, e as licenças deles, que tinham ficado sem conferência na P-11, cabem no `allow` (anotado no `PENDING.md`).
- **Não verificado**: `init` e `plan` no Arch real (falta o dono rodar `init > /tmp/p.toml`, `plan /tmp/p.toml`, que deve dar plano vazio, e o mesmo com um pacote inexistente acrescentado); a regra exata de ID do Flatpak (aceito 2 segmentos ou mais, e o Flatpak pode exigir 3); app Flatpak real no `init` (não há nenhum instalado); repositório `[ignored]`/`IgnorePkg` e grupos de pacotes no preset.
- **Fica**: fatia 2 (executar o plano: `sudo pacman -S` e `flatpak install`, classe privilegiada, confirmação por operação); dotfiles; critérios 5, 6 e 10; P-12, P-02, P-03, P-07, P-08; fechamento formal da Fase 0.
- **Verificado depois da mesclagem (PR #17, `ecb315b`)**: CI da `main` verde. O dono compilou (`CARGO_TARGET_DIR` em `/tmp`, apagado depois) e rodou no Arch real: `preset init > /tmp/p.toml` saiu com 154 pacotes (igual a `pacman -Qe | wc -l`), cabeçalho e um nome por linha, e `flatpak = []` **sem aviso**, o que está correto (o Flatpak está instalado, só não tem apps; minha previsão de que avisaria estava errada); `preset plan` deu `instalar 0, já presentes 154` e "Nada a fazer"; com `pacote-que-nao-existe` acrescentado, `instalar 1, já presentes 154`, só ele listado. Continuam **sem prova**: app Flatpak real, o caminho "Flatpak não instalado" numa máquina real, a regra exata de ID do Flatpak, `IgnorePkg` e grupos.

### 2026-11-13 — Sessão 11 (P-11: `cargo-deny` no CI)

- **Pedido**: resolver a P-11 (ADR 0007, item 5). Escolhas do dono no plano: `EmbarkStudios/cargo-deny-action@v2` (descartados `taiki-e/install-action` e `cargo install`); job separado e **não obrigatório** no ruleset, porque os avisos do RustSec podem ficar vermelhos sem o código mudar.
- **Antes**: PR #15 mesclado em `9743cd0`; `CI` e `CI Private Files` verdes na `main`.
- **Feito**: job `cargo-deny` no `ci.yml`; `deny.toml` com `allow = ["MIT", "Apache-2.0", "Unicode-3.0"]` (tirado das licenças do `Cargo.lock`: o `unicode-ident` exige Unicode-3.0; Zlib e Unlicense só aparecem em OR com MIT) e o resto nos defaults do cargo-deny 0.20 (`unmaintained = "all"`, `yanked = "warn"`, `multiple-versions = "warn"`, só crates.io). `LIBS.md` e `PENDING.md` (P-11 resolvida). Ruleset intocado.
- **CI (resultado real, PR #16)**: commit 1 verde no `cargo-deny`, só com o aviso de `syn` duplicado (2.0.119 e 3.0.10, esperado); mutação (tirar `Unicode-3.0`) deixou o `cargo-deny` vermelho com `error[rejected]` só no `unicode-ident` 1.0.27; revertida em commit próprio. Clippy e testes verdes em todas as rodadas.
- **Avaliação da ferramenta** (`padrao/dependencias-novas.md`): cargo-deny da Embark, MIT OR Apache-2.0, 0.20.2 (jul/2026); a action é Docker (`rust:1.85.0-alpine`), recebe só o repo e o ambiente padrão de qualquer action Docker, sem segredo do repo; fixada por tag `@v2`, sem SHA, como as outras do CI.
- **Não verificado**: a licença dos crates de Windows e macOS (`ntapi`, `objc2*`) foi aceita pelo CI, mas não conferi à mão; o comportamento num aviso real do RustSec (nenhum apareceu); `--log-level warn` não imprime o resumo "ok" por checagem, então "verde" é o que prova.
- **Fica**: tornar o check obrigatório (decisão futura do dono); preset e o resto do MVP; P-12, P-02, P-03, P-07, P-08; `pacman -Qu` = 1 numa máquina em dia; fallback sem `pacman-contrib`; fechamento formal da Fase 0.

### 2026-11-13 — Sessão 10 (`[fetch ok]` no texto do `nodus repos`)

- **Pedido**: o fetch que dava certo não imprimia nada no texto (só a falha aparecia); sem olhar o `FETCH_HEAD`, o usuário não sabia se tinha rodado. Escolha do dono: imprimir `[fetch ok]`; o `--json` já trazia `"fetch": "done"` e não muda.
- **Antes**: PR #14 mesclado em `cce849e`; `CI` e `CI Private Files` verdes na `main`.
- **Feito**: `render_repos` com `match` nos três casos de `fetch` (`Done`, `Failed`, `None`); teste `repos_texto_fetch_ok` com a linha exata. Sem dependência nova.
- **CI (resultado real, PR #15)**: commit 1 (só o teste) falhou só no `repos_texto_fetch_ok` (a linha veio sem `[fetch ok]`), 44 passaram na CLI e 85 no core; commit 2 passou (45 + 85, clippy limpo); mutação (`[fetch ok]` também sem `--fetch`) falhou 2 testes (`repos_texto_completo` e `repos_texto_head_solto_sem_commits_e_sem_upstream`); revertida em commit próprio.
- **Não verificado**: a saída nova no Arch real (falta o dono rodar `nodus repos --fetch` com o binário deste PR).
- **Fica**: as lacunas do `--fetch` listadas na sessão 9 (continuação); preset e o resto do MVP; P-11, P-12, P-02, P-03, P-07, P-08; `pacman -Qu` = 1 numa máquina em dia; fallback sem `pacman-contrib`; fechamento formal da Fase 0.
- **Verificado depois da mesclagem (PR #15, `9743cd0`)**: CI da `main` verde. O dono compilou (`CARGO_TARGET_DIR` em `/tmp`, apagado depois) e rodou no Arch real, nos repos de verdade do workspace: sem `--fetch`, 9 repos e nenhuma marca; com `--fetch`, os 9 com `[fetch ok]`. **SSH provado** (anchor, arca, kin, polis, truthid, warden: `git@github.com`, chave pelo `ssh-agent`, sem pedir senha, mesmo com `GIT_CONFIG_GLOBAL=/dev/null`); HTTPS público provado (arven, nodus, trellis). O commit do kin mudou entre as rodadas (`76fcef8` → `28870dd`): o reflog mostrou um commit de outra sessão, não efeito do fetch. Continuam **sem prova**: fetch que traga commits novos (nenhum repo estava atrás), HTTPS privado, remoto que não seja `origin`, HEAD solto, repo de outro dono.

### 2026-11-13 — Sessão 9, continuação (Fase 1, marco 6: `nodus repos`)

- **Pedido**: critério 7 do MVP. Escolhas do dono no plano: `fetch` só com `--fetch`; descoberta só em filhos diretos; ambiente por `env VAR=... git ...` (sem mexer no `CommandRunner`); mensagem de erro genérica no repo com `git` falhando (sem capturar stderr); repos ignorados (symlink, `.git` arquivo, nome com controle) em silêncio.
- **Antes**: PR #13 (P-09) mesclado, `main` do nodus com `CI` verde em `4eb352b`.
- **Etapa 0 (medição)**: `medir.sh` estendido; comandos de leitura limpos em 10 vetores; submódulo hostil bloqueado por `--no-recurse-submodules`; `--no-auto-maintenance` tira o `git maintenance` do `fetch` (3 chamadas para 0 no trace). Defeito do próprio script achado no caminho (`p_filter_clean` inexistente) e corrigido antes de valer.
- **Feito**: `nodus-core/src/repos.rs` (`parse_branches`, `isolated_git_args`, `inspect_repo`, `FetchResult`), `nodus-cli/src/discovery.rs` (`find_repos`), subcomando `Repos`, `render_repos`. Sem dependência nova.
- **CI (resultado real, PR #14, aberto como rascunho)**: commit 1 (só testes) falhou por erro de compilação (`parse_branches`, `find_repos`, `render_repos`, `inspect_repo`, `isolated_git_args`, variante `Repos` inexistentes); commit 2 passou de primeira (85 testes no core e 44 na CLI, clippy limpo); mutação (`core.hooksPath=/dev/null` → `/tmp`) falhou 2 testes (`linha_isolada_tem_a_receita_do_adr_0009` e `toda_chamada_vai_por_env_e_isolada`), 83 passaram; revertida em commit próprio.
- **Mudança de doc**: `OVERVIEW.md` ainda dizia que o marco 5 estava "em revisão no PR #11" (já mesclado); corrigido.
- **Não verificado**: execução do `nodus repos` no Arch real (falta o dono rodar e comparar com `git branch -vv`); `--fetch` com rede real (a medição usou remoto local); `--fetch` em repo HTTPS privado vai falhar (`GIT_CONFIG_GLOBAL=/dev/null` descarta credenciais; SSH com agent deve funcionar, não testado); remotos com nome diferente de `origin` (só `origin` é buscado); a mensagem do erro de "dubious ownership" é genérica (`código 128`); `credential.helper` segue sem medição (ADR 0009).
- **Fica**: validar no Arch real; preset e o resto do MVP; P-11, P-12, P-02, P-03, P-07, P-08; `pacman -Qu` = 1 numa máquina em dia; fallback sem `pacman-contrib`; fechamento formal da Fase 0.
- **Verificado depois da mesclagem (PR #14, `cce849e`)**: CI da `main` verde (`CI` e `CI Private Files`). O dono rodou `nodus repos` no Arch real: 9 repos do workspace (os que têm `.git` como pasta real; `lume` não existe nesta máquina), iguais ao `git branch -vv` em anchor, kin e nodus. O `--fetch` com rede real foi provado numa cópia em `/tmp` (apagada depois): o `FETCH_HEAD` foi reescrito na hora, vindo de `https://github.com/Arven-Labs/nodus`. Achado: o fetch bem-sucedido não aparecia no texto (corrigido na sessão 10). Continuam **sem prova**: fetch que traga commits novos, repo HTTPS privado, SSH, remoto que não seja `origin`, HEAD solto e repo de outro dono em máquina real.

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
