//! Núcleo do nodus. Por enquanto só expõe a versão; o domínio entra nos próximos marcos da Fase 1.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_vem_do_cargo_toml() {
        assert_eq!(VERSION, "0.0.1");
    }
}
