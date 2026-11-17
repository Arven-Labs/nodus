# nodus: Roadmap

Resumo da SPEC §14, com o corte dos ADRs 0003 e 0004.

| Fase | Foco | Estado |
|---|---|---|
| 0 | Docs e ADRs: arquitetura, modelo de ameaças, repo, libs e licenças (sem código) | **fechada** pelo dono em 2026-11-15 (ADRs 0003 a 0007, `SECURITY.md`, `LIBS.md`) |
| 1 | MVP: CLI + agente local no Arch (ver `MVP.md`) | **fechada pelo dono em 2026-11-17**: os 11 critérios do `MVP.md` estão prontos ou prontos com lacuna (10: sem reversão de Flatpak; 11: lacunas de verificação), primeira versão `v0.1.0` (tag assinada, só código-fonte). Ver "Fechamento do MVP" no `MVP.md` e `SESSIONS.md`. Pendências que não bloqueiam: P-02 antes de divulgar, P-14, P-17 |
| 1.5 | Desktop Tauri sobre o contrato da CLI | decisão de arquitetura no ADR 0026 (Tauri 2 + React chamando `nodusctl --json`, primeira versão só de leitura); fatia 1 (tela de inventário, só leitura, `desktop/`) no PR #82, CI verde; falta o dono rodar `npm run tauri dev` |
| 2 | Windows (WinGet), Core central, multi-device, tarefas remotas com aprovação | não iniciada |
| 3 | Debian/Ubuntu/Fedora, servidores, Docker, Proxmox, métricas | não iniciada |
| 4 | Project Health: uptime, OpenTelemetry, deploys | não iniciada |
| 5 | Empresa: multi-tenancy, RBAC, SSO/SCIM, auditoria | não iniciada |
| 6 | macOS, periféricos, plugins, cloud gerenciada, TruthID e IA | não iniciada |

Cada fase só começa com a anterior validada (SPEC §15).
