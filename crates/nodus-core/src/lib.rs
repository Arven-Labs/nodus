//! Núcleo do nodus. Por enquanto só expõe a versão; o domínio entra nos próximos marcos da Fase 1.

pub mod inventory;
pub mod preset;
pub mod repos;
pub mod runner;
pub mod updates;

/// Versão do workspace, lida do `Cargo.toml` em tempo de compilação.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_vem_do_cargo_toml() {
        assert_eq!(VERSION, "0.0.1");
    }
}
