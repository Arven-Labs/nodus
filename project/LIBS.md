# nodus: Levantamento de bibliotecas do MVP

**Data do levantamento:** 2026-11-12. Fonte: API do crates.io (versão, licença, data da última publicação, downloads recentes). Versões e popularidade mudam; refaça antes de adotar.

> **Limite deste levantamento:** só olhei metadados. **Não li o código, não rodei `cargo audit` ou `cargo deny`, não testei nenhuma lib.** "Atualizada recentemente" e "muito usada" não significam "segura". Cada dependência ainda precisa de aprovação individual quando entrar (ADR 0007).

Critério: (1) licença compatível com MIT (ADR 0002); (2) publicada nos últimos meses; (3) uso comprovado; (4) risco de segurança e de manutenção para o requisito do `MVP.md`.

## Tabela

| Necessidade (MVP.md) | Candidata | Versão | Licença | Última publicação | Veredito |
|---|---|---|---|---|---|
| CLI | `clap` | 4.6.7 | MIT OR Apache-2.0 | 2026-09 | Adotar |
| Preset e serialização | `serde`, `toml` | 1.0.229 / 1.2.1 | MIT OR Apache-2.0 | 2026-07 / 2026-11 | `serde` adotado no marco 2; `toml` adotado na fatia 1 do preset (1.2.1+spec-1.1.0, só no `nodus-core`, features padrão; o lock ganhou 9 pacotes da árvore do `toml`, todos MIT ou Apache-2.0, aceitos pelo `cargo-deny`) |
| Saída `--json` (contrato da CLI, marco 2) | `serde_json` | 1.0.154 (`Cargo.lock`) | MIT OR Apache-2.0 | 2026-10 | Adotada no marco 2, fora do levantamento original; licença e data conferidas no crates.io em 2026-11-12 (só metadados) |
| Erros | `thiserror`, `anyhow` | 2.0.21 / 1.0.104 | MIT OR Apache-2.0 | 2026-09 / 2026-07 | Adotar |
| Estado e histórico locais (critério 10) | `rusqlite` | 0.42.0 | MIT | 2026-11 | Adotar, a confirmar no spike |
| Inventário de hardware e sistema (critério 2) | `sysinfo` | 0.40.3 | MIT | 2026-11 | Adotada no marco 3 (0.40.3 no `Cargo.lock`; licença MIT e data reconferidas no crates.io em 2026-11-12, só metadados); cobre CPU, RAM, discos; **não** cobre GPU nem periféricos (fora do MVP) |
| Pacotes pacman (critérios 2, 8) | `alpm` (libalpm) | 5.0.2 | **GPL-3.0** | 2026-01 | **Descartar** (ver abaixo) |
| Tipos e versões de pacote Arch | `alpm-types` | 0.14.0 | Apache-2.0 OR MIT | 2026-11 | Não entra no MVP (ADR 0008): `pacman -Qu` e `checkupdates` já entregam o par `velha -> nova`. Docs.rs 0.14.0 mostra `Version` com epoch/pkgver/pkgrel e `vercmp`; reavaliar se o nodus precisar comparar versões |
| Apps Flatpak (critérios 2, 8) | `libflatpak` | 0.7.0 | MIT | 2025-11 | Descartar para o MVP: bindings pouco usados (~5 mil downloads recentes), sem publicação há ~1 ano |
| Git: descoberta e `fetch` (critério 7) | `git` CLI isolado (processo externo) | n/a | n/a | n/a | Decidido no ADR 0009; `gix` 0.90.0 (MIT OR Apache-2.0) fora do MVP |
| Git, alternativa | `git2` (libgit2, C) | 0.21.0 | MIT OR Apache-2.0 | 2026-10 | Descartar: liga código C nativo e aumenta a superfície |
| Testes de arquivo | `tempfile` | 3.27.0 | MIT OR Apache-2.0 | 2026-03 | Adotar (só dev) |
| Auditoria de licenças e avisos (CI) | `cargo-deny` | 0.20.2 | MIT OR Apache-2.0 | 2026-07 | Adotado (P-11): job `cargo-deny` no CI com `EmbarkStudios/cargo-deny-action@v2` (v2.1.1, cargo-deny 0.20.2, imagem Docker com Rust 1.85), regras em `deny.toml`; não obrigatório no ruleset |

## Achado principal: `alpm` é GPL-3.0

O crate `alpm` embrulha a libalpm e é licenciado GPL-3.0. Linká-lo no binário do nodus (MIT, ADR 0002) obrigaria o que distribuímos a sair sob GPL, o que contraria a escolha da MIT para adoção. Não é defeito do crate; é incompatibilidade com a nossa decisão.

Alternativa: executar `pacman` e `flatpak` como **processos externos**, com lista de argumentos (SECURITY.md T7), sem linkar nada. É prática comum separar assim, mas **não sou advogado**: confirme antes de distribuir.

## Não verificado
- Código, segurança e testes de cada crate.
- Se `alpm-types` cobre a comparação de versões que o plano de atualização precisa (`vercmp`): ficou sem necessidade no MVP (ADR 0008).
- ~~Se `gix` não executa hooks nem `core.fsmonitor` num `fetch`~~: o `git` CLI isolado foi medido no P-09 (ADR 0009); `gix` não foi medido porque não foi necessário.
- Formatos de saída estáveis de `pacman` e `flatpak` para parsing: medidos no spike P-10 (ADR 0008), com lacunas lá registradas (nenhum app Flatpak na máquina, locale `pt_BR` ausente).
- Dotfiles: não há lib candidata; a vinculação por symlink usa `std::os::unix::fs` e é código nosso, sujeito ao SECURITY.md T2.
