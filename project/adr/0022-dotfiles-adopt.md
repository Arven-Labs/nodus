# 0022: `dotfiles adopt`: pasta-fonte espelhando o HOME, cópia conferida e troca atômica por symlink

**Data:** 2026-11-16

## Contexto
O ADR 0021 entregou só o relatório de dotfiles. O dono decidiu em 2026-11-16 que **vincular dotfile entra no MVP** (critério 11). Medido no `$HOME` dele: os cinco dotfiles da lista padrão (`.bashrc`, `.bash_profile`, `.bash_logout`, `.profile`, `.gitconfig`) são **arquivos reais** e não existe pasta-fonte; um `link` que só cria symlink onde o alvo não existe não teria o que fazer. A primeira escrita útil é **adotar**: levar o arquivo real para uma pasta-fonte e deixar um symlink no lugar.

É escrita em arquivo real do usuário (AGENTS §4; `SECURITY.md` §4, classe "escrita de usuário": plano e confirmação). O histórico atual (v3) é feito para pacotes e não comporta o registro.

## Decisão
1. **Modelo (escolha do dono):** uma **pasta-fonte que espelha o HOME** (`<dir>/.bashrc` para `~/.bashrc`), configurada por `dotfiles_dir` no `config.toml` (ADR 0018, versão 1). Estilo stow. O formato do preset não muda. A pasta é um repositório Git **privado**, fora do workspace (escolha do dono); o nodus **não** configura nem roda Git nela.
2. **`nodusctl dotfiles adopt <caminho>`** (caminho relativo ao HOME, regras do `validate_path` do ADR 0021, inclusive a denylist). Um arquivo por vez; só arquivo comum (pasta inteira e symlink ficam fora). Limite de **1 MiB**.
3. **Sequência**, cada passo só depois de o anterior dar certo:
   1. plano mostrado (origem, destino, tamanho, aviso de segredo) e `Executar? [s/N]` (só `s`/`sim`; sem terminal interativo, recusa);
   2. cópia para `<dir>/<caminho>` com `create_new` (**nunca sobrescreve**; destino existente é recusa), preservando só os 9 bits de permissão (sem setuid/setgid/sticky), com `fsync`;
   3. **conferência byte a byte** da cópia contra o original;
   4. conferir de novo que o original **não mudou** (tamanho, `mtime`, `dev` e `ino`) desde a leitura;
   5. criar o symlink temporário `.<nome>.nodus-<pid>` no mesmo diretório do original e **`rename` por cima** dele (atômico: quem lê vê o arquivo antigo ou o link, nunca nada).
   Falha em qualquer passo antes do 5 deixa o original intacto e remove **só** a cópia que o nodus criou.
4. **A cópia conferida é o backup.** Depois da troca o conteúdo original só existe na pasta-fonte; por isso a ordem acima. Sem `.bak` no HOME.
5. **Symlink absoluto**, apontando para o caminho real (`canonicalize`) da pasta-fonte, mostrado assim no plano.
6. **Recusas** (nada é alterado): origem que é symlink, pasta ou outro tipo; origem acima de 1 MiB; algum componente do **pai** da origem (dentro do HOME) é symlink (T2); `dotfiles_dir` ausente, não configurada, relativa, inexistente ou dentro de um componente da denylist; pasta-fonte que contém o próprio arquivo; destino existente.
7. **Histórico v4, só para dotfile** (a leitura continua aceitando v1 a v3): `manager = "dotfiles"`, `action = "adopt"`, `dir` (caminho absoluto da pasta-fonte, para o `undo` futuro não depender do config de hoje), nomes validados por `validate_path`. Pacman e Flatpak continuam gravando v3. A leitura estrita vale: um nodus antigo recusa a linha (`UnsupportedVersion`), como no ADR 0014.
8. **`history undo` de entrada v4 recusa** com mensagem clara (a reversão é a fatia seguinte). O arquivo continua na pasta-fonte e o dono pode desfazer à mão. *(Emenda, ADR 0023: a reversão existe; o `undo` de adoção copia o conteúdo atual da fonte de volta e grava uma linha v4 `remove` com `undoes`. O `parse_line` v4 passou a aceitar essa combinação.)*
9. **Sem dependência nova.** `O_NOFOLLOW` exigiria o crate `libc`; em vez disso: `symlink_metadata`, abrir, e comparar `dev`/`ino` do arquivo aberto com o observado.

## Alternativas
- **Seção `[[dotfiles]]` no preset:** descartada pelo dono; muda o parser do preset e mistura "instalar" com "editar arquivo real".
- **Copiar para o HOME em vez de linkar:** descartada pelo dono; editar no HOME não atualiza a fonte e a divergência vira o problema.
- **`link` primeiro (só onde o alvo não existe):** descartada pelo dono; não faria nada na máquina dele até existir `adopt`.
- **`mv` do original para a pasta:** descartada; entre sistemas de arquivos é copiar e apagar, com janela sem arquivo no HOME.
- **Apagar o original e criar o link:** descartada; mesma janela.
- **`.bak` ao lado do original:** descartada; lixo no HOME e a cópia conferida já é o backup.
- **`--force` para sobrescrever destino:** descartada; é o caminho para perder dado.
- **Reaproveitar o v3 com `manager = "pacman"`:** descartada; validaria o nome pela regra errada. **Arquivo de histórico separado:** descartada; quebra a numeração do `undo`.
- **Varrer o conteúdo à procura de segredo:** descartada; o nodus não lê para classificar (ADR 0021). O plano só avisa.

## Consequências
- Habilita levar os dotfiles para uma pasta versionável com um comando e confirmação, sem janela de perda.
- O conteúdo do arquivo **é lido e copiado** para uma pasta que vira repo: vazamento de segredo é risco real e fica com o dono. O plano avisa em destaque (por exemplo token na URL do `.gitconfig`).
- Programas que reescrevem o arquivo (editor com salvamento atômico, ferramentas que regravam a configuração, o desktop) podem trocar o symlink por um arquivo comum e divergir da pasta-fonte. Confiança **baixa** sobre quais fazem isso; `nodusctl dotfiles` já mostra `arquivo` no lugar de `symlink`, então a divergência aparece. A verificar no Arch real.
- Dois processos do nodus adotando o mesmo arquivo ao mesmo tempo: o `create_new` faz o segundo recusar; sem trava além disso.
- Reabrir se: o limite de 1 MiB atrapalhar, ou se `link`/`undo` pedirem outro formato de registro.

## Lacunas
- **Resíduo de corrida (declarado):** um processo do mesmo usuário ainda pode trocar o original entre a conferência e o `rename`; as checagens de `mtime`/`ino` estreitam a janela, não a fecham. O usuário é a fronteira de confiança.
- **Implementado no PR #66** (testes primeiro, CI vermelho nas rodadas de core, config e `adopt_file`; mutação de propósito: 7 quebras derrubaram 9 testes). Verificado no Arch em 2026-11-16: num ensaio com `HOME=/tmp/hx` (adotar, listar, histórico, `undo` recusado, adotar de novo recusado) e numa adoção real do `.bash_logout` (21 bytes, `diff` igual, histórico real com v3 e v4 juntos). O comportamento de `git config --global`, editores e KDE com o symlink segue **não verificado**.
- **A checagem de "o original mudou" não pega tudo:** compara tamanho, `mtime`, `dev` e `ino`. Uma regravação do **mesmo tamanho dentro do mesmo tick do relógio do kernel** (alguns milissegundos) não muda o `mtime` e passa. Os testes mudam o tamanho de propósito.
- **Pastas vazias podem sobrar:** se a falha vem depois de criar `.config/git/` na pasta-fonte, só o arquivo copiado é removido; as pastas ficam (não perdem dado).
- **`dotfiles_dir` na leitura do config só confere a forma** (absoluta, sem `..`, `.`, `//` nem caractere de controle); HOME e denylist são conferidos no `adopt`, que conhece o HOME. O erro aparece ao adotar, não ao ler o config.
- **Ganchos de teste no código de produção:** `adopt_file` recebe dois closures (`after_copy`, `before_swap`), vazios no uso real, para testar as recusas no meio da operação de forma determinística.
- Dois testes (`sem_permissao_na_pasta_fonte...` e `falha_ao_gravar_o_historico...`) saem sem checar nada se rodarem como root, porque a escrita passaria; no CI (usuário comum) rodam. Não verificado se o runner é root.
- Não verificado: o comportamento de `git config --global`, editores e KDE com o symlink; só se sabe no Arch real.
- Este ADR foi escrito por IA e revisado só pelo dono; nenhuma revisão externa.
