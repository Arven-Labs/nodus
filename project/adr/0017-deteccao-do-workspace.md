# 0017: Detecção da pasta de workspace por candidatos conhecidos, com escolha manual sem estado

**Data:** 2026-11-15

## Contexto
O critério 6 do MVP pede "identificar a pasta padrão de workspace" (SPEC §6.6: "detectar ou solicitar o diretório padrão de projetos"). Até aqui o `nodusctl repos` exigia a pasta como argumento toda vez, e o nodus não sabia qual é o workspace.

Medido em 2026-11-15, nesta máquina, só leitura:
- O workspace real é `~/Documents/workspace`, com **9 repositórios** como filhos diretos (a própria pasta também é um repositório).
- `~/Projects` existe (é o `XDG_PROJECTS_DIR`), com **0 repositórios**.
- Uma varredura do `$HOME` agrupando os `.git` por pasta-pai ranquearia o **`$HOME` em primeiro** (3 repositórios soltos no nível 1) e erraria: os 9 do workspace ficam a 4 níveis de profundidade.
- `find_repos` (`discovery.rs`) já conta os filhos diretos que são pasta real com `.git` real e ignora symlink e `.git` arquivo (T2).

## Decisão
1. **Detecção por candidatos conhecidos, não por varredura.** Lista fixa sob o `$HOME`, nesta ordem: `Documents/workspace`, `workspace`, `Projects`, `projects`, `dev`, `src`, `code`, `repos`, `git`. Para cada candidato que é pasta, conta os repositórios filhos com `find_repos`. Vence o de mais repositórios (precisa ter pelo menos 1); empate pela ordem da lista; candidatos que resolvem ao mesmo caminho real aparecem uma vez; candidato ilegível ou que não é pasta é pulado.
2. **Escolha manual sem estado:** `nodusctl workspace [pasta]` e a variável `NODUS_WORKSPACE`. Precedência: argumento, variável, detecção. A pasta informada precisa existir e ser pasta; vale o caminho real; aceita 0 repositórios com um aviso. A variável vazia é ignorada; **relativa, inexistente ou que não é pasta é erro**, não cai em silêncio na detecção.
3. **Só relatório:** o comando não escreve nada e não guarda estado; `--json` traz `chosen` (com `path`, `source`, `repo_count`) e `candidates`. Sem workspace detectado, exit 0, com a lista dos candidatos vazios e a instrução para informar a pasta.
4. **O `repos` não muda:** continua exigindo a pasta.

## Alternativas
- **Varredura do `$HOME`:** descartada; lenta, entra em pastas privadas e erra nesta máquina (escolheria o `$HOME`).
- **Salvar a escolha (`workspace set`, arquivo de configuração):** descartada nesta fatia por escolha do dono; é o "solicitar e lembrar" da SPEC, mas exige um arquivo novo (onde fica, permissão, symlink, formato) e deixa de ser só relatório. A escolha manual não fica gravada: vale por chamada, ou fixa na variável do shell.
- **`repos` sem argumento usar o workspace detectado:** descartada nesta fatia; mexe no contrato do `repos` e nos testes dele. É o passo natural seguinte.
- **Ler `~/.config/user-dirs.dirs` para o `XDG_PROJECTS_DIR`:** descartada; a chave não é do padrão do xdg-user-dirs e `~/Projects` já está na lista.
- **Cair na detecção quando a variável é inválida:** descartada; esconderia o engano de quem a definiu.

## Consequências
- Nome de pasta fora da lista não é detectado; o argumento e a variável cobrem.
- Um workspace com 0 repositórios (recém-criado) não é escolhido pela detecção; informá-lo à mão funciona, com aviso.
- Candidato que é symlink é resolvido ao caminho real e mostrado assim (só lê, não escreve).
- O desempate pela ordem da lista é arbitrário (`workspace` ganha de `Projects` com o mesmo número de repositórios); o dono corrige informando a pasta.
- Só Arch Linux/Unix: o SPEC pede um caminho equivalente configurável no Windows, fora do MVP.
- Reabrir se: o dono quiser lembrar a escolha (ADR de configuração) ou o `repos` passar a usar o workspace detectado.

## Não verificado
- O comando no Arch real com binário novo (os testes usam pastas temporárias e um `HOME` falso).
- Candidato ilegível por permissão (pulado, mas não testado com permissão real) e symlink circular.
- Só um teste cobre o ranking por contagem: a mutação que o ignora foi pega por apenas um teste (`candidatos_saem_ordenados_por_repositorios_decrescente`).
- Este ADR foi escrito por IA e revisado só pelo dono; nenhuma revisão externa.
