# 0005: Stack inicial (provisória)

**Data:** 2026-11-12
**Status:** provisória; revisar ao fim da Fase 0.

## Contexto
A SPEC §8 propõe Rust, Tauri, React, PostgreSQL e OpenTelemetry como hipótese a validar. O MVP (ADR 0004) é uma CLI com agente local, sem Core nem GUI.

## Decisão
Para o MVP:
- **Rust** para a CLI e o agente local.
- Estado local em arquivo/SQLite embutido (a confirmar na Fase 0); sem servidor.

Adiados, sem decisão agora: Tauri/React (Fase 1.5), Core e PostgreSQL (Fase 2), métricas e OpenTelemetry (Fase 4).

## Alternativas descartadas
- Go: bom para agentes, mas a SPEC já aponta Rust e reaproveita a mesma linguagem no Tauri.
- Python: prototipagem rápida, mas distribuição de agente e binário único são piores.
- Decidir toda a stack agora: contraria a §18 (decisões em aberto).

## Consequências
- Compilação em Rust roda só no CI (ADR 0009 do workspace).
- Se a validação da Fase 0 contradisser Rust, este ADR é substituído, não editado.
