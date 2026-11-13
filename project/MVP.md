# nodus: MVP (Fase 1)

CLI + agente local, só Arch Linux (ADR 0004). Base: SPEC §16.

## Critérios de pronto
1. Instalar a CLI em um Arch Linux.
2. `inventory`: hardware, sistema e aplicativos instalados. (Marcos 2 a 4: sistema, hardware (CPU, RAM, discos), pacotes pacman e apps Flatpak, texto e `--json`. Critério 2 completo no código; falta o dono validar no Arch real.)
3. Criar um preset declarativo (apps e configurações).
4. Reaplicar o preset sem duplicar nem quebrar configurações (idempotência).
5. Detectar app recém-instalado e perguntar se entra no preset.
6. Identificar a pasta padrão de workspace.
7. Descobrir repositórios Git e exibir o estado (somente leitura, mais `fetch`).
8. Verificar atualizações de pacotes (pacman) e apps (Flatpak).
9. Exibir um plano antes de qualquer mudança; nada executa sem confirmação.
10. Registrar o que mudou e permitir reversão quando suportada.

## Fora do MVP
Desktop (Fase 1.5), Windows, Core e multi-device (Fase 2), outras distros, periféricos, overclock, pull/reset de Git, sincronização de segredos, IA, métricas e empresa.

## Regras herdadas da SPEC §15
Sem scripts genéricos de otimização; segredos nunca entram em presets; mudanças sempre via plano.
