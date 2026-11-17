# nodus desktop

Interface do nodus (Fase 1.5, [ADR 0026](../project/adr/0026-gui-desktop.md)): Tauri 2 + React/TypeScript. **Só leitura**: a janela roda `nodusctl <comando> --json` e mostra o resultado. Hoje há uma tela, o inventário.

## Como funciona
- O frontend manda ao backend só o **nome** de um comando (`read_nodusctl`, `{ command: "inventory" }`).
- O Rust (`src-tauri/src/nodusctl.rs`) recusa o que não está na lista, monta os argumentos (`inventory --json`) sem shell, confere que `nodusctl --version` tem o mesmo `major.minor` desta interface e só então roda. A saída tem limite de 16 MiB.
- O frontend lê o JSON com parsing estrito (`src/lib/inventory.ts`): falta de campo ou tipo errado vira uma mensagem de erro, e nomes vindos da máquina são mostrados como texto, nunca como HTML.
- A GUI não roda `sudo`, não escreve no HOME e não chama nenhum comando que escreve.

## Rodar (exceção ao ADR 0009 para o `desktop/`, emenda do ADR 0026)
Pré-requisitos no Arch: `webkit2gtk-4.1`, `base-devel`, `librsvg`, `libappindicator`, `xdotool`, Node e Rust ([pré-requisitos do Tauri 2](https://v2.tauri.app/start/prerequisites/)). O `nodusctl` precisa estar no PATH (ou em `NODUSCTL=/caminho/nodusctl`, que **precisa ser absoluto**: o app roda com o diretório atual em `src-tauri/`) **na mesma versão major.minor** (hoje `0.1.x`).

```bash
cd desktop
npm install
NODUSCTL=../target/release/nodusctl npm run tauri dev
```

Antes do primeiro `npm install`, rode `scripts/disk-report.sh` na raiz do workspace (`node_modules` e `src-tauri/target` pesam centenas de MB).

## Testes (rodam no CI, `.github/workflows/ci-desktop.yml`)
- `npm test`: parsing do contrato (`tests/`, fixtures inventadas).
- `npm run build` e `npm run test:e2e`: a tela num navegador sem janela, com um backend Tauri de mentira que responde o `invoke` (`e2e/harness.mjs`); prova a tela e o que ela pede, não o Rust.
- `cargo test` em `src-tauri/`: allowlist, versão e execução do binário (um script de mentira).

## Limites desta fatia
Sem `cargo-deny` para o `src-tauri` (as licenças das dependências do Tauri precisam de uma política própria; a da CLI só aceita MIT, Apache-2.0 e Unicode-3.0). Ícone é um placeholder. Sem empacotamento e sem binário distribuído (ADR 0010). Uma única tela. Sem tempo limite para o `nodusctl` (um processo travado trava a leitura).
