//! Config do nodus em `$XDG_CONFIG_HOME/nodus/config.toml`: a pasta de workspace escolhida com
//! `nodusctl workspace set` (ADR 0018) e as listas de itens ignorados no `preset adopt` (ADR 0020).
//! Mesma disciplina do histórico (SECURITY.md T5): diretório 0700, arquivo 0600, sem symlink,
//! leitura estrita e com limite, gravação atômica. Toda escrita lê o config, muda o campo certo e
//! grava tudo de volta, para uma escrita não apagar os outros campos.

use crate::history::check_file;
use anyhow::{Context, Result, bail};
use nodus_core::preset::{is_valid_flatpak_id, is_valid_pacman_name};
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const FORMAT_VERSION: i64 = 1;

/// O que o config guarda. Sem arquivo, é o valor vazio.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Config {
    /// Pasta gravada com `workspace set`.
    pub workspace: Option<PathBuf>,
    /// Nomes de pacman que o `adopt` e o `suggest` não mostram mais.
    pub ignore_pacman: Vec<String>,
    /// IDs de app Flatpak que o `adopt` e o `suggest` não mostram mais.
    pub ignore_flatpak: Vec<String>,
}

impl Config {
    fn is_empty(&self) -> bool {
        self.workspace.is_none() && self.ignore_pacman.is_empty() && self.ignore_flatpak.is_empty()
    }
}

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

/// A pasta de workspace gravada, ou `None` se não há config (ou ele não traz a chave).
pub fn read_workspace(path: &Path) -> Result<Option<PathBuf>> {
    Ok(read(path)?.workspace)
}

/// Lê o config; sem arquivo, devolve o config vazio. Formato estrito: chave desconhecida, versão
/// diferente, caminho relativo, nome inválido ou repetido nas listas, ou TOML inválido são erro.
pub fn read(path: &Path) -> Result<Config> {
    if !check_file(path)? {
        return Ok(Config::default());
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

fn parse(text: &str) -> Result<Config> {
    let table: toml::Table = toml::from_str(text).context("TOML inválido")?;
    if let Some(key) = table.keys().find(|key| {
        !matches!(
            key.as_str(),
            "version" | "workspace" | "ignore_pacman" | "ignore_flatpak"
        )
    }) {
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
    let workspace = match table.get("workspace") {
        None => None,
        Some(toml::Value::String(value)) => {
            let path = Path::new(value);
            if !path.is_absolute() {
                bail!("`workspace` precisa ser um caminho absoluto: {value:?}");
            }
            Some(path.to_path_buf())
        }
        Some(_) => bail!("`workspace` precisa ser texto"),
    };
    Ok(Config {
        workspace,
        ignore_pacman: name_list(&table, "ignore_pacman", is_valid_pacman_name)?,
        ignore_flatpak: name_list(&table, "ignore_flatpak", is_valid_flatpak_id)?,
    })
}

/// Uma lista de nomes do config: cada item é texto, passa na regra de nome do preset e não repete.
fn name_list(table: &toml::Table, key: &str, is_valid: fn(&str) -> bool) -> Result<Vec<String>> {
    let items = match table.get(key) {
        None => return Ok(Vec::new()),
        Some(toml::Value::Array(items)) => items,
        Some(_) => bail!("`{key}` precisa ser uma lista de nomes"),
    };
    let mut names: Vec<String> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let number = index + 1;
        let toml::Value::String(name) = item else {
            bail!("`{key}`: o item {number} precisa ser texto");
        };
        if !is_valid(name) {
            bail!("`{key}`: nome inválido no item {number}: {name:?}");
        }
        if names.contains(name) {
            bail!("`{key}`: nome repetido: {name:?}");
        }
        names.push(name.clone());
    }
    Ok(names)
}

/// Grava a pasta de workspace, mantendo o resto do config.
pub fn write_workspace(path: &Path, workspace: &Path) -> Result<()> {
    if !workspace.is_absolute() {
        bail!(
            "a pasta do workspace precisa ser um caminho absoluto: {}",
            workspace.display()
        );
    }
    let config = Config {
        workspace: Some(workspace.to_path_buf()),
        ..Config::default()
    };
    store(path, &config)
}

/// Esquece a pasta de workspace, mantendo as listas. `false` se não havia pasta gravada.
pub fn clear_workspace(path: &Path) -> Result<bool> {
    let mut config = read(path)?;
    if config.workspace.is_none() {
        return Ok(false);
    }
    config.workspace = None;
    store(path, &config)?;
    Ok(true)
}

/// Acrescenta nomes às listas de ignorados, sem repetir e na ordem em que chegam. Nome inválido é
/// erro e nada é gravado; se não há nada novo, o arquivo nem é tocado.
pub fn add_ignored(path: &Path, pacman: &[String], flatpak: &[String]) -> Result<()> {
    if let Some(name) = pacman.iter().find(|name| !is_valid_pacman_name(name)) {
        bail!("nome de pacman inválido: {name:?}");
    }
    if let Some(id) = flatpak.iter().find(|id| !is_valid_flatpak_id(id)) {
        bail!("ID de Flatpak inválido: {id:?}");
    }
    let mut config = read(path)?;
    let added =
        push_new(&mut config.ignore_pacman, pacman) | push_new(&mut config.ignore_flatpak, flatpak);
    if !added {
        return Ok(());
    }
    store(path, &config)
}

/// `true` se entrou algum nome.
fn push_new(list: &mut Vec<String>, names: &[String]) -> bool {
    let mut added = false;
    for name in names {
        if !list.contains(name) {
            list.push(name.clone());
            added = true;
        }
    }
    added
}

/// Tira nomes das listas de ignorados (de qualquer uma que os tenha). Se algum nome não está
/// ignorado, é erro e nada é gravado.
pub fn remove_ignored(path: &Path, names: &[String]) -> Result<()> {
    let mut config = read(path)?;
    let missing: Vec<&str> = names
        .iter()
        .filter(|name| {
            !config.ignore_pacman.contains(name) && !config.ignore_flatpak.contains(name)
        })
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        bail!("não está na lista de ignorados: {}", missing.join(", "));
    }
    config.ignore_pacman.retain(|name| !names.contains(name));
    config.ignore_flatpak.retain(|name| !names.contains(name));
    store(path, &config)
}

/// Grava o config inteiro, ou apaga o arquivo se não sobrou nada nele. Atômico: escreve num
/// temporário (0600) ao lado e troca por `rename`, então uma falha no meio deixa o config antigo
/// inteiro, nunca pela metade. Recusa symlink e permissão mais larga que 0600 no que já existe.
fn store(path: &Path, config: &Config) -> Result<()> {
    if config.is_empty() {
        if check_file(path)? {
            std::fs::remove_file(path).with_context(|| format!("apagando {}", path.display()))?;
        }
        return Ok(());
    }
    let mut table = toml::Table::new();
    table.insert("version".into(), toml::Value::Integer(FORMAT_VERSION));
    if let Some(workspace) = &config.workspace {
        let value = workspace
            .to_str()
            .with_context(|| format!("caminho fora de UTF-8: {}", workspace.display()))?;
        table.insert("workspace".into(), toml::Value::String(value.into()));
    }
    for (key, names) in [
        ("ignore_pacman", &config.ignore_pacman),
        ("ignore_flatpak", &config.ignore_flatpak),
    ] {
        if !names.is_empty() {
            let items = names.iter().cloned().map(toml::Value::String).collect();
            table.insert(key.into(), toml::Value::Array(items));
        }
    }
    let body = format!(
        "# Config do nodus. Gerado por `nodusctl`; edite à vontade.\n{}",
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
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("criando {}", tmp.display()))?;
    let written =
        commit(file, &body, &tmp, path).with_context(|| format!("gravando {}", path.display()));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// Escreve no temporário, força para o disco e só então troca pelo destino (o `rename` no mesmo
/// diretório é atômico). Também usado pelo `write_preset`.
pub(crate) fn commit(mut file: File, body: &str, tmp: &Path, path: &Path) -> std::io::Result<()> {
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(tmp, path)
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

    // --- ignorados: listas opcionais no mesmo config (ADR 0020) ---

    fn nomes(lista: &[&str]) -> Vec<String> {
        lista.iter().map(|s| s.to_string()).collect()
    }

    fn caminho(tmp: &TmpDir) -> PathBuf {
        tmp.path().join("nodus/config.toml")
    }

    #[test]
    fn le_as_duas_listas_de_ignorados() {
        let tmp = TmpDir::novo();
        let path = config_com(
            &tmp,
            "version = 1\nignore_pacman = [\"a\", \"b\"]\nignore_flatpak = [\"org.x.A\"]\n",
        );
        assert_eq!(
            read(&path).unwrap(),
            Config {
                workspace: None,
                ignore_pacman: nomes(&["a", "b"]),
                ignore_flatpak: nomes(&["org.x.A"]),
            }
        );
    }

    #[test]
    fn config_sem_as_listas_ou_sem_arquivo_le_listas_vazias() {
        let tmp = TmpDir::novo();
        let path = config_com(&tmp, "version = 1\nworkspace = \"/a\"\n");
        assert_eq!(
            read(&path).unwrap(),
            Config {
                workspace: Some(PathBuf::from("/a")),
                ignore_pacman: vec![],
                ignore_flatpak: vec![],
            }
        );
        assert_eq!(read(&caminho(&tmp)).unwrap(), Config::default());
    }

    #[test]
    fn read_workspace_so_devolve_a_pasta_mesmo_com_listas() {
        let tmp = TmpDir::novo();
        let path = config_com(
            &tmp,
            "version = 1\nworkspace = \"/a\"\nignore_pacman = [\"x\"]\n",
        );
        assert_eq!(read_workspace(&path).unwrap(), Some(PathBuf::from("/a")));
    }

    #[test]
    fn add_ignored_cria_o_config_sem_repetir_nem_reordenar() {
        let tmp = TmpDir::novo();
        let path = caminho(&tmp);
        add_ignored(&path, &nomes(&["a", "b"]), &nomes(&["org.x.A"])).unwrap();
        add_ignored(&path, &nomes(&["b", "c", "c"]), &[]).unwrap();
        let config = read(&path).unwrap();
        assert_eq!(config.ignore_pacman, nomes(&["a", "b", "c"]));
        assert_eq!(config.ignore_flatpak, nomes(&["org.x.A"]));
        assert_eq!(modo(&path), 0o600);
        assert_eq!(modo(path.parent().unwrap()), 0o700);
    }

    #[test]
    fn add_ignored_sem_nada_novo_nao_cria_o_arquivo_e_o_conteudo_segue_igual() {
        let tmp = TmpDir::novo();
        let path = caminho(&tmp);
        add_ignored(&path, &[], &[]).unwrap();
        assert!(!path.exists());
        add_ignored(&path, &nomes(&["a"]), &[]).unwrap();
        let antes = fs::read_to_string(&path).unwrap();
        add_ignored(&path, &nomes(&["a"]), &[]).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), antes);
    }

    #[test]
    fn add_ignored_recusa_nome_invalido_e_nao_grava() {
        let tmp = TmpDir::novo();
        let path = caminho(&tmp);
        assert!(add_ignored(&path, &nomes(&["-Rns"]), &[]).is_err());
        assert!(add_ignored(&path, &[], &nomes(&["semponto"])).is_err());
        assert!(add_ignored(&path, &nomes(&["a\"b"]), &[]).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn write_workspace_preserva_as_listas_de_ignorados() {
        let tmp = TmpDir::novo();
        let path = caminho(&tmp);
        add_ignored(&path, &nomes(&["a"]), &nomes(&["org.x.A"])).unwrap();
        write_workspace(&path, Path::new("/ws")).unwrap();
        assert_eq!(
            read(&path).unwrap(),
            Config {
                workspace: Some(PathBuf::from("/ws")),
                ignore_pacman: nomes(&["a"]),
                ignore_flatpak: nomes(&["org.x.A"]),
            }
        );
    }

    #[test]
    fn add_ignored_preserva_a_pasta_de_workspace() {
        let tmp = TmpDir::novo();
        let path = caminho(&tmp);
        write_workspace(&path, Path::new("/ws")).unwrap();
        add_ignored(&path, &nomes(&["a"]), &[]).unwrap();
        assert_eq!(read_workspace(&path).unwrap(), Some(PathBuf::from("/ws")));
    }

    #[test]
    fn clear_workspace_preserva_as_listas_e_so_apaga_o_arquivo_se_nao_sobra_nada() {
        let tmp = TmpDir::novo();
        let path = caminho(&tmp);
        add_ignored(&path, &nomes(&["a"]), &[]).unwrap();
        write_workspace(&path, Path::new("/ws")).unwrap();
        assert!(clear_workspace(&path).unwrap());
        assert!(path.exists());
        let config = read(&path).unwrap();
        assert_eq!(config.workspace, None);
        assert_eq!(config.ignore_pacman, nomes(&["a"]));
        // Sem pasta gravada, `clear` diz que não havia e não mexe nas listas.
        assert!(!clear_workspace(&path).unwrap());
        assert_eq!(read(&path).unwrap().ignore_pacman, nomes(&["a"]));
    }

    #[test]
    fn remove_ignored_tira_dos_dois_tipos_e_apaga_o_arquivo_quando_nao_sobra_nada() {
        let tmp = TmpDir::novo();
        let path = caminho(&tmp);
        add_ignored(&path, &nomes(&["a", "b"]), &nomes(&["org.x.A"])).unwrap();
        remove_ignored(&path, &nomes(&["a"])).unwrap();
        let config = read(&path).unwrap();
        assert_eq!(config.ignore_pacman, nomes(&["b"]));
        assert_eq!(config.ignore_flatpak, nomes(&["org.x.A"]));
        remove_ignored(&path, &nomes(&["b", "org.x.A"])).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn remove_ignored_com_nome_que_falta_e_erro_e_nao_grava() {
        let tmp = TmpDir::novo();
        let path = caminho(&tmp);
        add_ignored(&path, &nomes(&["a", "b"]), &[]).unwrap();
        let antes = fs::read_to_string(&path).unwrap();
        let erro = format!(
            "{:#}",
            remove_ignored(&path, &nomes(&["a", "nao-esta", "outro"])).unwrap_err()
        );
        assert!(
            erro.contains("nao-esta") && erro.contains("outro"),
            "{erro}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), antes);
    }

    #[test]
    fn remove_ignored_sem_config_e_erro() {
        let tmp = TmpDir::novo();
        let path = caminho(&tmp);
        assert!(remove_ignored(&path, &nomes(&["a"])).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn read_recusa_lista_de_ignorados_invalida_e_diz_o_arquivo() {
        let casos = [
            ("nome de pacman inválido", "ignore_pacman = [\"-Rns\"]\n"),
            (
                "id de flatpak inválido",
                "ignore_flatpak = [\"semponto\"]\n",
            ),
            ("nome repetido", "ignore_pacman = [\"a\", \"a\"]\n"),
            ("não é lista", "ignore_pacman = \"a\"\n"),
            ("item que não é texto", "ignore_pacman = [1]\n"),
        ];
        for (nome, linha) in casos {
            let tmp = TmpDir::novo();
            let path = config_com(&tmp, &format!("version = 1\n{linha}"));
            let erro = format!("{:#}", read(&path).unwrap_err());
            assert!(erro.contains("config.toml"), "{nome}: {erro}");
        }
    }
}
