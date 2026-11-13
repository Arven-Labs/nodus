# nodus: Roadmap

Resumo da SPEC §14, com o corte dos ADRs 0003 e 0004.

| Fase | Foco | Estado |
|---|---|---|
| 0 | Docs e ADRs: arquitetura, modelo de ameaças, repo, libs e licenças (sem código) | entregas feitas (ADRs 0003 a 0007, `SECURITY.md`, `LIBS.md`); falta o dono aprovar o fechamento |
| 1 | MVP: CLI + agente local no Arch (ver `MVP.md`) | em andamento: marco 1 (esqueleto e CI) feito; marco 2 (`inventory`: sistema e pacman) feito; marco 3 (hardware no `inventory`) em revisão |
| 1.5 | Desktop Tauri sobre o contrato da CLI | não iniciada |
| 2 | Windows (WinGet), Core central, multi-device, tarefas remotas com aprovação | não iniciada |
| 3 | Debian/Ubuntu/Fedora, servidores, Docker, Proxmox, métricas | não iniciada |
| 4 | Project Health: uptime, OpenTelemetry, deploys | não iniciada |
| 5 | Empresa: multi-tenancy, RBAC, SSO/SCIM, auditoria | não iniciada |
| 6 | macOS, periféricos, plugins, cloud gerenciada, TruthID e IA | não iniciada |

Cada fase só começa com a anterior validada (SPEC §15).
