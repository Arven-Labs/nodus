//! Apoio aos testes: diretório temporário único, apagado no fim. Nunca toca no `$HOME` real.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static CONTADOR: AtomicUsize = AtomicUsize::new(0);

pub struct TmpDir(PathBuf);

impl TmpDir {
    pub fn novo() -> Self {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("nodus-cli-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        TmpDir(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
