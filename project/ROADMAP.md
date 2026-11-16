# nodus: Roadmap

Resumo da SPEC §14, com o corte dos ADRs 0003 e 0004.

| Fase | Foco | Estado |
|---|---|---|
| 0 | Docs e ADRs: arquitetura, modelo de ameaças, repo, libs e licenças (sem código) | **fechada** pelo dono em 2026-11-15 (ADRs 0003 a 0007, `SECURITY.md`, `LIBS.md`) |
| 1 | MVP: CLI + agente local no Arch (ver `MVP.md`) | em andamento: marcos 1 a 6 feitos (esqueleto e CI, `inventory` com sistema, pacman, hardware e Flatpak, `updates`, `repos`); spikes P-09 e P-10 feitos; preset com `init`, `plan`, `apply` (pacman e Flatpak), `suggest` (só relatório), `history` e `history undo`; `workspace` e `repos` sem pasta usando o workspace. Critérios 5 e 6 completos; `dotfiles` (critério 11) só como relatório, falta verificar no Arch e decidir se vincular entra no MVP (ver `MVP.md` e `SESSIONS.md`) |
| 1.5 | Desktop Tauri sobre o contrato da CLI | não iniciada |
| 2 | Windows (WinGet), Core central, multi-device, tarefas remotas com aprovação | não iniciada |
| 3 | Debian/Ubuntu/Fedora, servidores, Docker, Proxmox, métricas | não iniciada |
| 4 | Project Health: uptime, OpenTelemetry, deploys | não iniciada |
| 5 | Empresa: multi-tenancy, RBAC, SSO/SCIM, auditoria | não iniciada |
| 6 | macOS, periféricos, plugins, cloud gerenciada, TruthID e IA | não iniciada |

Cada fase só começa com a anterior validada (SPEC §15).
