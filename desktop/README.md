# nodus desktop

Interface do nodus (Fase 1.5, [ADR 0026](../project/adr/0026-gui-desktop.md)): Tauri 2 + React/TypeScript. **Só leitura**: a janela roda `nodusctl <comando> --json` e mostra o resultado. Hoje há oito abas: Inventário, Repositórios, Dotfiles, Atualizações, Workspace, Histórico, Preset e Sugestões; as ações que escrevem só abrem um terminal com o comando da CLI (ADR 0027).

## Como funciona
- O frontend manda ao backend só o **nome** de um comando (`read_nodusctl`, `{ command: "inventory" }`).
- O Rust (`src-tauri/src/nodusctl.rs`) recusa o que não está na lista, monta os argumentos (`inventory --json`) sem shell, confere que `nodusctl --version` tem o mesmo `major.minor` desta interface e só então roda. A saída tem limite de 16 MiB.
- O frontend lê o JSON com parsing estrito (`src/lib/inventory.ts`): falta de campo ou tipo errado vira uma mensagem de erro, e nomes vindos da máquina são mostrados como texto, nunca como HTML.
- A GUI não roda `sudo`, não escreve no HOME e não chama nenhum comando que escreve.
- Telas (abas): Inventário (`inventory`) e Repositórios (`repos`, sem `--fetch`; a pasta é a que a CLI resolve: `NODUS_WORKSPACE`, `workspace set` ou a detectada). Dotfiles (`dotfiles`, só o relatório: `dotfiles adopt` e `dotfiles link` escrevem no HOME e a GUI não os pede). Compare (`compare --all`, só leitura): compara todos os pares de snapshots da pasta-fonte (`<dotfiles_dir>/.nodus/snapshots/`), um bloco por par, e não usa `NODUS_PRESET` nem `NODUS_MACHINE`; com menos de dois snapshots legíveis mostra só o estado vazio. A aba tem o botão "Gravar snapshot no terminal" (`snapshot`, ADR 0027): sem argumento da tela (o rótulo é o hostname, decidido pela CLI), sem `NODUS_PRESET` nem `NODUS_MACHINE`, e a CLI não pergunta nada; depois, "Atualizar" relê a comparação (o commit do arquivo é do dono). Cada aba só roda o seu comando quando é aberta. Cada chamada ao `nodusctl` tem prazo (30 s); estourou, o app mata o `nodusctl` e os processos que ele lançou e mostra "demorou demais".

## Rodar (só no PC do dono, à mão; exceção do `desktop/`, emendas 1 e 2 do ADR 0026)
**Não rodar na VM de desenvolvimento, nem pelo `!` do agente** (ADR 0017 do workspace). Pré-requisitos no Arch: `webkit2gtk-4.1`, `base-devel`, `librsvg`, `libappindicator`, `xdotool`, Node e Rust ([pré-requisitos do Tauri 2](https://v2.tauri.app/start/prerequisites/)). O `nodusctl` precisa estar no PATH (ou em `NODUSCTL=/caminho/nodusctl`, que **precisa ser absoluto**: o app roda com o diretório atual em `src-tauri/`) **na mesma versão major.minor** (hoje `0.1.x`).

```bash
cd desktop
npm install
NODUSCTL=../target/release/nodusctl npm run tauri dev
```

**Aba Status e `exceptions`** (ADR 0033, emenda 4): por máquina, a aba mostra o trecho `[machines.<rótulo>]` para colar no preset (o mesmo que `nodusctl preset status` imprime no terminal; só leitura, sem botão). Ela lê o campo `exceptions` do `--json`, que o `nodusctl` só tem a partir do PR #218: com um `nodusctl` mais antigo a aba dá erro citando `machines[0].exceptions`. Confira o binário instalado com `cargo install --list`.

**`NODUS_MACHINE`** (opcional, lida só no Rust): o rótulo da máquina para as exceções `[machines.<rótulo>]` do preset (ADR 0033). Se definida, vira `--machine <rótulo>` em `preset plan` e `preset suggest` (abas Preset e Sugestões), nos botões "Instalar no terminal" (`preset apply`) e "Adotar no terminal" (`preset adopt`) e na releitura do "Voltar a sugerir". Sem ela, a CLI usa o hostname (só se o preset tem `[machines]`). Use quando o hostname tiver maiúsculas ou outro caractere fora de `[a-z0-9._-]`: a CLI erra pedindo `--machine` e a janela não tem como passá-lo. Regra do valor: 1 a 63 caracteres de `[a-z0-9._-]`, começando por letra ou dígito; valor ruim (inclusive vazio) é erro nas abas e ações que a usam, e não derruba as outras (Status e Inventário, por exemplo). Exemplo: `NODUS_PRESET=/home/eu/preset.toml NODUS_MACHINE=notebook NODUSCTL=../target/release/nodusctl npm run tauri dev`.

Antes do primeiro `npm install`, rode `scripts/disk-report.sh` na raiz do workspace (`node_modules` e `src-tauri/target` pesam centenas de MB).

## Testes (rodam no CI, `.github/workflows/ci-desktop.yml`)
- `npm test`: parsing do contrato (`tests/`, fixtures inventadas).
- `npm run build` e `npm run test:e2e`: a tela num navegador sem janela, com um backend Tauri de mentira que responde o `invoke` (`e2e/harness.mjs`); prova a tela e o que ela pede, não o Rust.
- `cargo test` em `src-tauri/`: allowlist, versão e execução do binário (um script de mentira).

## Limites desta fatia
Sem `cargo-deny` para o `src-tauri` (as licenças das dependências do Tauri precisam de uma política própria; a da CLI só aceita MIT, Apache-2.0 e Unicode-3.0). Ícone é um placeholder. Sem empacotamento e sem binário distribuído (ADR 0010). Uma única tela. Sem tempo limite para o `nodusctl` (um processo travado trava a leitura).
