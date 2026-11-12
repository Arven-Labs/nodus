# nodus: Regras de código

Valem para o código Rust do workspace (ADR 0006). Regras gerais do fluxo estão no `AGENTS.md` do workspace; segurança em `SECURITY.md`.

## Build e teste
- Build, clippy e testes rodam **só no CI** (ADR 0009 do workspace). Local só `cargo fmt --check`.
- Clippy com `-D warnings`; `unsafe_code = "forbid"` no workspace.
- Testes primeiro: o teste falha antes (o CI mostra) e passa depois.

## Estrutura
- `crates/nodus-core`: domínio (presets, planos, adaptadores). Sem I/O de terminal.
- `crates/nodus-cli`: binário `nodus`; só interpreta argumentos e chama o core.
- Dependência nova entra em `[workspace.dependencies]`, com aprovação do dono (ADR 0007), e é herdada pelos crates com `.workspace = true`.

## Segurança (de `SECURITY.md`)
- Preset é **dado**: ações tipadas, nunca shell livre (T1).
- Processo externo (`pacman`, `flatpak`, `git`) com **lista de argumentos**, nunca string de shell (T7).
- Caminhos: resolver o caminho real e aceitar só dentro de raízes declaradas; recusar symlink que escape (T2).
- Nada de segredo em preset, log ou histórico (T3). Logs registram o que mudou, não o conteúdo.
- Toda escrita passa por plano e confirmação; a classe privilegiada pede confirmação por operação.

## Erros e testes
- Biblioteca (`nodus-core`): erros tipados com `thiserror`. Binário: `anyhow` na borda.
- Sem `unwrap`/`expect` em caminho de produção; `panic` só para invariante violada.
- Testes de arquivo usam diretório temporário; nunca tocam o `$HOME` real.
- Código crítico (parsing de entrada externa, caminhos, preset): casos de borda e entradas maliciosas, e depois quebrar de propósito para ver o teste falhar (AGENTS.md §D).

## Estilo
- Comentários só para o *porquê* não óbvio. Nomes comunicam intenção.
- Sem abstração para o futuro (YAGNI); sem refatoração misturada com a tarefa.
