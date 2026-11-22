pub mod actions;
pub mod nodusctl;

use nodusctl::CmdError;
use std::path::PathBuf;

/// A versão desta GUI (a do `Cargo.toml`), no formato de `nodusctl --version` para reaproveitar o `parse_version`.
fn gui_version() -> (u64, u64, u64) {
    nodusctl::parse_version(&format!("nodusctl {}", env!("CARGO_PKG_VERSION"))).expect("versão do crate no formato X.Y.Z")
}

/// O binário vem da variável `NODUSCTL` (só caminho absoluto) ou do PATH. Não há caminho embutido.
fn binary() -> Result<PathBuf, CmdError> {
    nodusctl::resolve_binary(std::env::var_os("NODUSCTL"))
}

/// Comando de leitura da janela: roda um comando de **leitura** da lista (`nodusctl::Allowed`) e devolve o JSON bruto.
/// O frontend manda só o nome; argumentos e versão são conferidos em `nodusctl::read` (ADR 0026).
#[tauri::command]
async fn read_nodusctl(command: String) -> Result<String, CmdError> {
    // `NODUS_PRESET` só é lida aqui, no Rust: o frontend nunca fornece caminho (e só o `preset_plan` a usa).
    let preset = std::env::var_os("NODUS_PRESET");
    // `NODUS_MACHINE` também: só `preset_plan` e `preset_suggest` a usam, e uma ruim não derruba as outras abas.
    let machine = std::env::var_os("NODUS_MACHINE");
    tauri::async_runtime::spawn_blocking(move || nodusctl::read(&binary()?, &command, gui_version(), preset, machine))
        .await
        .map_err(|e| CmdError { kind: "failed", message: format!("a leitura foi interrompida: {e}") })?
}

/// Segundo comando da janela, e o único que lança algo que escreve (ADR 0027): abre um terminal rodando `nodusctl <ação>`, da lista
/// fechada de `actions::Action`. O frontend manda o nome e, no máximo, um argumento: o número `n` da `history_undo` (inteiro `u32`: o
/// Tauri já recusa texto, negativo e fracionário ao desserializar) ou o `path` da `workspace_set`, que o Rust só aceita se for idêntico
/// a um item que a CLI lista na hora (ADR 0028). O arquivo do preset, o terminal e o `PATH` são lidos aqui, no Rust.
/// A GUI não confirma, não pede senha e não roda `sudo`: isso é da CLI, no terminal.
#[tauri::command]
async fn run_action(command: String, n: Option<u32>, path: Option<String>) -> Result<actions::Outcome, CmdError> {
    let env = actions::Env {
        binary: binary()?,
        preset: std::env::var_os("NODUS_PRESET"),
        machine: std::env::var_os("NODUS_MACHINE"),
        terminal: std::env::var_os("NODUS_TERMINAL"),
        path: std::env::var_os("PATH"),
    };
    tauri::async_runtime::spawn_blocking(move || actions::run(&command, n, path, env, gui_version()))
        .await
        .map_err(|e| CmdError { kind: "failed", message: format!("a ação foi interrompida: {e}") })?
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![read_nodusctl, run_action])
        .run(tauri::generate_context!())
        .expect("erro ao rodar a aplicação Tauri");
}
