# nodus: Visão geral

## O que é
Plataforma open source e extensível para conhecer, configurar, padronizar, monitorar e administrar computadores, servidores, VPS, ambientes de desenvolvimento e aplicações. Quatro frentes: gestão de dispositivos, de ambientes, de infraestrutura e saúde de projetos, e de frotas e organizações. Princípios: local-first, self-hostable, multiplataforma, modular, API-first, seguro por padrão, vendor-neutral. A IA é integração opcional, nunca requisito. Detalhes no `SPEC.md` (que ainda usa o nome provisório "Universal").

## Stack
Provisória (ADR 0005): Rust para a CLI e o agente local do MVP; estado local embutido a confirmar. Tauri, Core e PostgreSQL adiados. MVP = CLI + agente no Arch, sem GUI (ADR 0004; ver `MVP.md`, `ROADMAP.md`).

## Estrutura do repo
Monorepo com Cargo workspace (`crates/*`), como o `kin` (ADR 0006). Nada criado ainda.

## Licença
MIT (ADR 0002).

## Status
Fase 0 em andamento (só docs e ADRs, sem código; ADR 0003). Fase 1 ainda não iniciada.
