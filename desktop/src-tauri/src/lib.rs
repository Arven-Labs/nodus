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

/// Único comando que a janela expõe: roda um comando de **leitura** da lista (`nodusctl::Allowed`) e devolve o JSON bruto.
/// O frontend manda só o nome; argumentos e versão são conferidos em `nodusctl::read` (ADR 0026).
#[tauri::command]
async fn read_nodusctl(command: String) -> Result<String, CmdError> {
    tauri::async_runtime::spawn_blocking(move || nodusctl::read(&binary()?, &command, gui_version()))
        .await
        .map_err(|e| CmdError { kind: "failed", message: format!("a leitura foi interrompida: {e}") })?
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![read_nodusctl])
        .run(tauri::generate_context!())
        .expect("erro ao rodar a aplicação Tauri");
}
