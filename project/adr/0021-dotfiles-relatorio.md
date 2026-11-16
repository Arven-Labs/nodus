# 0021: Dotfiles, fatia 1: relatório de lista curada, sem ler conteúdo

**Data:** 2026-11-16

## Contexto
O ROADMAP listava "dotfiles" como o que faltava da Fase 1, mas o `MVP.md` não tinha critério para eles (só a SPEC §16 e o `SECURITY.md`, "vincular dotfile"). No `$HOME` do dono, medido em 2026-11-16 (só leitura): não há stow, chezmoi, yadm nem repositório de dotfiles; são arquivos soltos (`.bashrc`, `.bash_profile`, `.profile`, `.bash_logout`, `.gitconfig`), mais `.config/*` cheio de arquivos que apps e o KDE reescrevem. Tocar em dotfile é escrever em arquivo real do usuário (AGENTS §4), então a primeira fatia não escreve.

## Decisão
1. **Critério 11 do MVP** (escolha do dono): relatar o estado dos dotfiles. Vincular, copiar ou comparar conteúdo ficam para fatias posteriores, cada uma com ADR próprio. *(Emenda: adotar é o ADR 0022, desfazer a adoção o 0023 e ligar na máquina nova o 0024; comparar conteúdo segue sem fatia.)*
2. **`nodusctl dotfiles [--json]`**, só leitura: para cada item da lista, se existe, se é arquivo, diretório ou symlink (com o alvo) e o tamanho. **Nunca abre o arquivo** (só `symlink_metadata` e `read_link`). **Escolha do dono: relatório de lista curada.**
3. **Lista curada, não varredura do `$HOME`** (escolha do dono). Padrão no código: `.bashrc`, `.bash_profile`, `.bash_logout`, `.profile`, `.gitconfig`, `.config/git/config`, `.zshrc`, `.vimrc`, `.tmux.conf`. O usuário ajusta no `config.toml` do ADR 0018 (versão 1) com `dotfiles_extra` e `dotfiles_skip`. **Escolha do dono.**
4. **Denylist fixa de segredos**, não configurável: `.ssh`, `.gnupg`, `.config/gh`, `.aws`, `.docker`, `.pki`, `.mozilla`, `.npmrc`, `.claude*` e nomes `*_history`. Um item de `dotfiles_extra` que bata, ou que seja absoluto ou tenha `..`, faz o config ser recusado com o caminho (SECURITY T3; SPEC §10.2: o nodus não lê nem registra segredos).
5. **Symlink não é seguido** (SECURITY T2): o relatório mostra o alvo e marca se ele sai do `$HOME`, mas não olha o que há nele.
6. **Sem hash nesta fatia.** Hash exige abrir o arquivo; comparar máquinas é da Fase 2.

## Alternativas
- **Varrer o `$HOME` ou `.config`:** descartada; traria `.cache`, KDE, `.claude.json` (token) e ruído de apps.
- **Só declarar no preset:** descartada pelo dono; o relatório ficaria vazio até ele escrever a lista.
- **Seção `[dotfiles]` no preset com `plan`:** adiada; exige decidir onde ficam os arquivos-fonte.
- **Vincular com symlink já (tipo stow):** adiada; escrita em arquivo real, precisa de backup e reversão no histórico.
- **Denylist configurável:** descartada; o ponto dela é não depender de o dono lembrar.

## Consequências
- Fecha uma lacuna de documentação (critério que não existia) e entrega a fatia mais barata e sem risco de escrita.
- Um `config.toml` com `dotfiles_extra` inválido derruba também `workspace` e `repos`, porque o arquivo é lido por inteiro (mesma consequência do ADR 0020); aceito.
- Nesta fatia, ajustar a lista é editar o `config.toml` à mão; subcomandos de adicionar e pular ficam para depois se doer.
- Reabrir se: a denylist esconder algo que o dono quer gerenciar, ou se o relatório precisar de conteúdo (hash, diff).

## Lacunas
- A denylist é por caminho relativo ao `$HOME`; um dotfile que contenha segredo mas esteja fora dela (por exemplo `.gitconfig` com token na URL) é listado sem que o nodus saiba, porque ele não lê o conteúdo.
- Nada implementado ainda: a fatia de código vem em PR seguinte.
