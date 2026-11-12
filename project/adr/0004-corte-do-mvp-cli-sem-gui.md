# 0004: MVP é CLI + agente local, sem GUI

**Data:** 2026-11-12

## Contexto
A Fase 1 da SPEC §14 tem 12 itens, incluindo aplicativo desktop. O caso real (SPEC §17) é padronizar dois ambientes Arch Linux: apps, dotfiles e workspace Git.

## Decisão
O MVP (Fase 1) é uma **CLI com agente local**, só Arch Linux, cobrindo os critérios 1-10 da SPEC §16:
inventário, preset declarativo, plano antes de aplicar, reaplicação idempotente, histórico e reversão onde suportado, pacman e Flatpak, descoberta de repositórios Git (somente leitura) e verificação de atualizações.

O desktop (Tauri) passa para a **Fase 1.5**, consumindo o contrato que a CLI já expõe (comandos e saída JSON).

## Alternativas descartadas
- MVP com desktop Tauri: maior risco de escopo; a UI não valida o motor.
- MVP só de leitura: não valida preset nem idempotência, que são o núcleo do caso real.

## Consequências
- O contrato CLI/agente precisa ser estável e versionado cedo.
- Sem interface gráfica, o uso inicial é só por terminal.
- Fetch e diagnóstico de Git entram; pull/reset continuam proibidos (SPEC §15).
