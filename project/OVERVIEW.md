# nodus: Visão geral

## O que é
Plataforma open source e extensível para conhecer, configurar, padronizar, monitorar e administrar computadores, servidores, VPS, ambientes de desenvolvimento e aplicações. Quatro frentes: gestão de dispositivos, de ambientes, de infraestrutura e saúde de projetos, e de frotas e organizações. Princípios: local-first, self-hostable, multiplataforma, modular, API-first, seguro por padrão, vendor-neutral. A IA é integração opcional, nunca requisito. Detalhes no `SPEC.md` (que ainda usa o nome provisório "Universal").

## Stack
Rust confirmado para o MVP (ADRs 0005 e 0007): CLI e agente local, com `clap`, `serde`/`toml`, `rusqlite`, `sysinfo`. pacman e Flatpak por processo externo (`alpm` é GPL-3.0). Git em aberto (P-09). Detalhes em `LIBS.md`. Tauri, Core e PostgreSQL adiados. MVP = CLI + agente no Arch, sem GUI (ADR 0004; ver `MVP.md`, `ROADMAP.md`).

## Estrutura do repo
Monorepo com Cargo workspace (`crates/*`), como o `kin` (ADR 0006): `nodus-core` (domínio) e `nodus-cli` (binário `nodus`). Regras de código em `GUIDELINES.md`.

## Licença
MIT (ADR 0002).

## Status
Fase 0 com todas as entregas feitas (ADR 0003), aguardando o fechamento formal pelo dono. Fase 1 em andamento: esqueleto do workspace e CI Rust; `nodus inventory` (sistema, hardware via `sysinfo`, pacotes pacman, `--json`) mesclado. Spike P-10 (ADR 0008) mesclado: formato de saída de Flatpak e de atualizações medido. Apps Flatpak no inventário (`flatpak_apps`) mesclados (marco 4). `nodus updates` (marco 5, critério 8: `checkupdates` com fallback `pacman -Qu`, e Flatpak) em revisão no PR #11. Preset e o resto do MVP ainda não existem.
