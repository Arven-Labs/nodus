# nodus: MVP (Fase 1)

CLI + agente local, só Arch Linux (ADR 0004). Base: SPEC §16.

## Critérios de pronto
1. Instalar a CLI em um Arch Linux.
2. `inventory`: hardware, sistema e aplicativos instalados. (Marcos 2 a 4: sistema, hardware (CPU, RAM, discos), pacotes pacman e apps Flatpak, texto e `--json`. Critério 2 completo no código; falta o dono validar no Arch real.)
3. Criar um preset declarativo (apps e configurações). (Fatia 1: `nodus preset init` imprime um preset TOML com os pacotes explícitos, `pacman -Qe`, e os apps Flatpak; só pacotes, sem configurações.)
4. Reaplicar o preset sem duplicar nem quebrar configurações (idempotência). (Fatia 1: `nodus preset plan <arquivo>` mostra o que falta, comparando com `pacman -Q`; `init` seguido de `plan` dá plano vazio. Fatia 2: `nodus preset apply <arquivo>` instala o que falta do pacman com `sudo pacman -S --needed`; plano vazio não executa nada. Flatpak ainda não.)
5. Detectar app recém-instalado e perguntar se entra no preset.
6. Identificar a pasta padrão de workspace.
7. Descobrir repositórios Git e exibir o estado (somente leitura, mais `fetch`). (Marco 6: `nodus repos <pasta> [--fetch] [--json]` lista os filhos diretos com branch, commit, upstream, ahead/behind e URL do `origin`, só por refs com o isolamento do ADR 0009; sem "sujo/limpo", sem `status`. `--fetch` só do `origin`.)
8. Verificar atualizações de pacotes (pacman) e apps (Flatpak).
9. Exibir um plano antes de qualquer mudança; nada executa sem confirmação. (`preset apply`: plano, comando exato, `Executar? [s/N]` e o `[Y/n]` do pacman; sem terminal interativo, recusa.)
10. Registrar o que mudou e permitir reversão quando suportada.

## Fora do MVP
Desktop (Fase 1.5), Windows, Core e multi-device (Fase 2), outras distros, periféricos, overclock, pull/reset de Git, sincronização de segredos, IA, métricas e empresa.

## Regras herdadas da SPEC §15
Sem scripts genéricos de otimização; segredos nunca entram em presets; mudanças sempre via plano.
