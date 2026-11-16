//! Config do nodus em `$XDG_CONFIG_HOME/nodus/config.toml`: hoje só a pasta de workspace escolhida
//! com `nodusctl workspace set` (ADR 0018). Mesma disciplina do histórico (SECURITY.md T5):
//! diretório 0700, arquivo 0600, sem symlink, leitura estrita e com limite, gravação atômica.

use crate::history::check_file;
use anyhow::{Context, Result, bail};
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const FORMAT_VERSION: i64 = 1;

/// O config tem poucas linhas; 64 KiB barra arquivo gigante antes do parse.
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

/// Onde fica o config. Função pura: as variáveis de ambiente entram como argumento. Só valem
/// caminhos absolutos (o XDG manda ignorar os relativos).
pub fn config_path(xdg_config_home: Option<&str>, home: Option<&str>) -> Result<PathBuf> {
    let absolute = |s: &&str| !s.is_empty() && Path::new(s).is_absolute();
    if let Some(xdg) = xdg_config_home.filter(absolute) {
        return Ok(Path::new(xdg).join("nodus/config.toml"));
    }
    if let Some(home) = home.filter(absolute) {
        return Ok(Path::new(home).join(".config/nodus/config.toml"));
    }
    bail!("não achei onde guardar a config: defina XDG_CONFIG_HOME ou HOME com um caminho absoluto")
}

/// A pasta de workspace gravada, ou `None` se não há config (ou ele não traz a chave). Formato
/// estrito: chave desconhecida, versão diferente, caminho relativo ou TOML inválido são erro.
pub fn read_workspace(path: &Path) -> Result<Option<PathBuf>> {
    if !check_file(path)? {
        return Ok(None);
    }
    let file = File::open(path).with_context(|| format!("abrindo {}", path.display()))?;
    // Lê no máximo um byte além do limite: não confia no tamanho informado pelo sistema de arquivos.
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("lendo {}", path.display()))?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        bail!("{} é maior que 64 KiB", path.display());
    }
    let text =
        String::from_utf8(bytes).with_context(|| format!("{} não é UTF-8", path.display()))?;
    parse(&text).with_context(|| format!("lendo {}", path.display()))
}

fn parse(text: &str) -> Result<Option<PathBuf>> {
    let table: toml::Table = toml::from_str(text).context("TOML inválido")?;
    if let Some(key) = table
        .keys()
        .find(|key| !matches!(key.as_str(), "version" | "workspace"))
    {
        bail!("chave desconhecida: {key:?}");
    }
    match table.get("version") {
        Some(toml::Value::Integer(FORMAT_VERSION)) => {}
        Some(toml::Value::Integer(other)) => {
            bail!("versão {other} não suportada (este nodus só lê a {FORMAT_VERSION})")
        }
        Some(_) => bail!("`version` precisa ser um número inteiro"),
        None => bail!("falta `version`"),
    }
    match table.get("workspace") {
        None => Ok(None),
        Some(toml::Value::String(value)) => {
            let path = Path::new(value);
            if !path.is_absolute() {
                bail!("`workspace` precisa ser um caminho absoluto: {value:?}");
            }
            Ok(Some(path.to_path_buf()))
        }
        Some(_) => bail!("`workspace` precisa ser texto"),
    }
}

/// Grava a pasta de workspace. Atômico: escreve num temporário (0600) ao lado e troca por `rename`,
/// então uma falha no meio deixa o config antigo inteiro, nunca pela metade. Recusa symlink e
/// permissão mais larga que 0600 no config que já existe.
pub fn write_workspace(path: &Path, workspace: &Path) -> Result<()> {
    if !workspace.is_absolute() {
        bail!(
            "a pasta do workspace precisa ser um caminho absoluto: {}",
            workspace.display()
        );
    }
    let value = workspace
        .to_str()
        .with_context(|| format!("caminho fora de UTF-8: {}", workspace.display()))?;
    let mut table = toml::Table::new();
    table.insert("version".into(), toml::Value::Integer(FORMAT_VERSION));
    table.insert("workspace".into(), toml::Value::String(value.into()));
    let body = format!(
        "# Config do nodus. Gerado por `nodusctl workspace set`; edite à vontade.\n{}",
        toml::to_string(&table).context("montando o config")?
    );

    let dir = path.parent().context("caminho do config sem pasta")?;
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("criando {}", dir.display()))?;
    check_file(path)?;
    let tmp = dir.join(format!(".config.toml.{}.tmp", std::process::id()));
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o644)
        .open(&tmp)
        .with_context(|| format!("criando {}", tmp.display()))?;
    let written =
        commit(file, &body, &tmp, path).with_context(|| format!("gravando {}", path.display()));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// Escreve no temporário, força para o disco e só então troca pelo config (o `rename` no mesmo
/// diretório é atômico).
fn commit(mut file: File, body: &str, tmp: &Path, path: &Path) -> std::io::Result<()> {
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(tmp, path)
}

/// Apaga o config (a única chave que ele tem é o workspace). `false` se não havia config.
pub fn clear_workspace(path: &Path) -> Result<bool> {
    if !check_file(path)? {
        return Ok(false);
    }
    std::fs::remove_file(path).with_context(|| format!("apagando {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TmpDir;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};

    fn modo(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// Escreve um config à mão, com a permissão certa (0600), para testar a leitura.
    fn config_com(tmp: &TmpDir, conteudo: &str) -> PathBuf {
        let path = tmp.path().join("config.toml");
        fs::write(&path, conteudo).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        path
    }

    // --- caminho (função pura: as variáveis de ambiente entram como argumento) ---

    #[test]
    fn caminho_usa_xdg_config_home_quando_absoluto() {
        let p = config_path(Some("/x/cfg"), Some("/home/u")).unwrap();
        assert_eq!(p, PathBuf::from("/x/cfg/nodus/config.toml"));
    }

    #[test]
    fn caminho_ignora_xdg_vazio_ou_relativo() {
        for xdg in ["", "relativo/cfg"] {
            let p = config_path(Some(xdg), Some("/home/u")).unwrap();
            assert_eq!(
                p,
                PathBuf::from("/home/u/.config/nodus/config.toml"),
                "{xdg:?}"
            );
        }
    }

    #[test]
    fn caminho_cai_no_home_sem_xdg() {
        let p = config_path(None, Some("/home/u")).unwrap();
        assert_eq!(p, PathBuf::from("/home/u/.config/nodus/config.toml"));
    }

    #[test]
    fn caminho_sem_xdg_e_sem_home_e_erro_que_diz_as_variaveis() {
        for home in [None, Some(""), Some("relativo")] {
            let erro = config_path(None, home).unwrap_err().to_string();
            assert!(erro.contains("XDG_CONFIG_HOME"), "{home:?}: {erro}");
            assert!(erro.contains("HOME"), "{home:?}: {erro}");
        }
    }

    // --- gravar e ler ---

    #[test]
    fn write_cria_diretorio_0700_e_arquivo_0600_e_le_de_volta() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        write_workspace(&path, Path::new("/home/u/ws")).unwrap();
        assert_eq!(modo(&path), 0o600);
        assert_eq!(modo(path.parent().unwrap()), 0o700);
        assert_eq!(
            read_workspace(&path).unwrap(),
            Some(PathBuf::from("/home/u/ws"))
        );
    }

    #[test]
    fn write_substitui_o_valor_anterior() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        write_workspace(&path, Path::new("/a")).unwrap();
        write_workspace(&path, Path::new("/b")).unwrap();
        assert_eq!(read_workspace(&path).unwrap(), Some(PathBuf::from("/b")));
    }

    #[test]
    fn write_nao_deixa_temporario_sobrando() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        write_workspace(&path, Path::new("/a")).unwrap();
        write_workspace(&path, Path::new("/b")).unwrap();
        let nomes: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(nomes, ["config.toml"]);
    }

    #[test]
    fn caminho_com_aspas_e_barra_invertida_volta_igual() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        let estranho = Path::new("/tmp/a\"b\\c d");
        write_workspace(&path, estranho).unwrap();
        assert_eq!(read_workspace(&path).unwrap(), Some(estranho.to_path_buf()));
    }

    #[test]
    fn write_recusa_caminho_relativo_e_nao_cria_nada() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        let erro = format!(
            "{:#}",
            write_workspace(&path, Path::new("rel/ws")).unwrap_err()
        );
        assert!(erro.contains("absoluto"), "{erro}");
        assert!(!path.exists());
    }

    #[test]
    fn write_recusa_symlink_e_nao_escreve_no_alvo() {
        let tmp = TmpDir::novo();
        let alvo = tmp.path().join("alvo.toml");
        fs::write(&alvo, "").unwrap();
        fs::set_permissions(&alvo, fs::Permissions::from_mode(0o600)).unwrap();
        let path = tmp.path().join("config.toml");
        symlink(&alvo, &path).unwrap();
        let erro = format!("{:#}", write_workspace(&path, Path::new("/a")).unwrap_err());
        assert!(erro.contains("symlink"), "{erro}");
        assert_eq!(fs::read_to_string(&alvo).unwrap(), "");
    }

    #[test]
    fn write_e_read_recusam_permissao_mais_larga_que_0600() {
        for larga in [0o644, 0o640, 0o606, 0o660] {
            let tmp = TmpDir::novo();
            let path = tmp.path().join("config.toml");
            fs::write(&path, "version = 1\n").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(larga)).unwrap();
            let erro_write = format!("{:#}", write_workspace(&path, Path::new("/a")).unwrap_err());
            let erro_read = format!("{:#}", read_workspace(&path).unwrap_err());
            assert!(erro_write.contains("permissão"), "{larga:o}: {erro_write}");
            assert!(erro_read.contains("permissão"), "{larga:o}: {erro_read}");
        }
    }

    #[test]
    fn read_recusa_o_que_nao_e_arquivo_comum() {
        let tmp = TmpDir::novo();
        let erro = format!("{:#}", read_workspace(tmp.path()).unwrap_err());
        assert!(erro.contains("arquivo comum"), "{erro}");
    }

    // --- ler: formato estrito ---

    #[test]
    fn read_de_arquivo_inexistente_e_none() {
        let tmp = TmpDir::novo();
        assert_eq!(
            read_workspace(&tmp.path().join("nodus/config.toml")).unwrap(),
            None
        );
    }

    #[test]
    fn read_de_config_so_com_a_versao_e_none() {
        let tmp = TmpDir::novo();
        let path = config_com(&tmp, "version = 1\n");
        assert_eq!(read_workspace(&path).unwrap(), None);
    }

    #[test]
    fn read_recusa_conteudo_invalido_e_diz_o_arquivo() {
        let casos = [
            (
                "chave desconhecida",
                "version = 1\nworkspace = \"/a\"\nextra = 1\n",
            ),
            ("versão diferente", "version = 2\nworkspace = \"/a\"\n"),
            ("sem versão", "workspace = \"/a\"\n"),
            (
                "versão não numérica",
                "version = \"1\"\nworkspace = \"/a\"\n",
            ),
            ("caminho relativo", "version = 1\nworkspace = \"rel/ws\"\n"),
            ("caminho vazio", "version = 1\nworkspace = \"\"\n"),
            ("caminho não é texto", "version = 1\nworkspace = 3\n"),
            ("TOML inválido", "version = = 1\n"),
        ];
        for (nome, conteudo) in casos {
            let tmp = TmpDir::novo();
            let path = config_com(&tmp, conteudo);
            let erro = format!("{:#}", read_workspace(&path).unwrap_err());
            assert!(erro.contains("config.toml"), "{nome}: {erro}");
        }
    }

    #[test]
    fn read_recusa_arquivo_acima_do_limite() {
        let tmp = TmpDir::novo();
        let grande = format!("version = 1\n# {}\n", "x".repeat(65 * 1024));
        let path = config_com(&tmp, &grande);
        let erro = format!("{:#}", read_workspace(&path).unwrap_err());
        assert!(erro.contains("64 KiB"), "{erro}");
    }

    #[test]
    fn read_recusa_arquivo_que_nao_e_utf8() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("config.toml");
        fs::write(&path, [0x76, 0xff, 0xfe]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let erro = format!("{:#}", read_workspace(&path).unwrap_err());
        assert!(erro.contains("UTF-8"), "{erro}");
    }

    // --- apagar ---

    #[test]
    fn clear_remove_o_arquivo() {
        let tmp = TmpDir::novo();
        let path = tmp.path().join("nodus/config.toml");
        write_workspace(&path, Path::new("/a")).unwrap();
        assert!(clear_workspace(&path).unwrap());
        assert!(!path.exists());
        assert_eq!(read_workspace(&path).unwrap(), None);
    }

    #[test]
    fn clear_sem_arquivo_diz_que_nao_havia() {
        let tmp = TmpDir::novo();
        assert!(!clear_workspace(&tmp.path().join("nodus/config.toml")).unwrap());
    }

    #[test]
    fn clear_recusa_symlink_e_nao_apaga_o_alvo() {
        let tmp = TmpDir::novo();
        let alvo = tmp.path().join("alvo.toml");
        fs::write(&alvo, "version = 1\n").unwrap();
        fs::set_permissions(&alvo, fs::Permissions::from_mode(0o600)).unwrap();
        let path = tmp.path().join("config.toml");
        symlink(&alvo, &path).unwrap();
        let erro = format!("{:#}", clear_workspace(&path).unwrap_err());
        assert!(erro.contains("symlink"), "{erro}");
        assert!(alvo.exists());
    }
}
