# Amostras de saída de pacman e Flatpak

Trechos curtos de saída **real**, medidos em 2026-11-12 num Arch Linux (pacman 7.1.0 / libalpm 16.0.1, Flatpak 1.18.4, checkupdates 1.13.1, `LANG=en_US.UTF-8`) e editados: só pacotes públicos, nada de caminho de home nem nome de usuário. Contexto e decisões: `project/adr/0008-formato-de-saida-pacman-flatpak.md`.

| Arquivo | Comando | Observação |
|---|---|---|
| `flatpak_list_runtime.tsv` | `flatpak list --runtime --columns=application,version,branch,origin` | Tabulação como separador; versão pode ser **vazia** (tab duplo); mesmo ID com ramos diferentes |
| `flatpak_remote_ls_updates.tsv` | `flatpak remote-ls --updates --columns=application,version,branch` | Sem coluna da versão instalada |
| `pacman_qu.txt` | `pacman -Qu` | `nome velha -> nova`; olha o banco local, que pode estar velho |
| `checkupdates.txt` | `checkupdates` | Mesmo formato, mas sincroniza uma cópia temporária do banco |
| `pacman_q_edge.txt` | `pacman -Q` | Epoch (`1:`), `+` e `rc` na versão; `paru` e `kwin-x11` são pacotes "foreign" (`-Qm`) |

**Lacuna:** não há amostra de `flatpak list --app` porque a máquina de medição não tinha nenhum app Flatpak (só runtimes). O formato de app foi assumido igual ao de runtime e **não foi medido**.
