//! Vault resolution.
//!
//! Mirrors the desktop: the active vault is the one in the shared
//! SQLite registry (`httui_core::vaults`). On the very first run, when
//! no vault is registered, we prompt the user on stdin before the alt
//! screen takes over. From then on the binary always opens whatever
//! `active_vault` points at; switching/adding/removing happens from
//! inside the TUI (/20 ex commands).

use sqlx::sqlite::SqlitePool;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use tracing::warn;

use crate::error::{TuiError, TuiResult};

#[derive(Debug)]
pub struct ResolvedVault {
    pub vault: PathBuf,
}

/// Read the active vault from the database, prompting on stdin when
/// the registry is empty (or when the active vault path no longer
/// exists on disk).
pub async fn resolve(pool: &SqlitePool) -> TuiResult<ResolvedVault> {
    if let Some(active) = httui_core::vaults::get_active_vault(pool).await? {
        let path = PathBuf::from(&active);
        if path.is_dir() {
            return Ok(ResolvedVault { vault: path });
        }
        warn!(
            ?path,
            "active vault no longer exists on disk, prompting for a new one"
        );
    }

    let path = prompt_for_vault()?;
    let path_str = path.to_string_lossy();
    httui_core::vaults::set_active_vault(pool, &path_str).await?;
    Ok(ResolvedVault { vault: path })
}

/// Block on stdin asking for a vault directory. Called only when no
/// vault is registered yet (or the previously-active one is gone).
fn prompt_for_vault() -> TuiResult<PathBuf> {
    println!("Welcome to notes-tui.");
    println!("No vault registered yet. Enter a path to a directory of markdown notes.");
    print!("> ");
    std::io::stdout()
        .flush()
        .map_err(|e| TuiError::Terminal(format!("flush stdout: {e}")))?;

    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| TuiError::Terminal(format!("read stdin: {e}")))?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(TuiError::InvalidArg("vault path cannot be empty".into()));
    }

    let expanded = expand_tilde(trimmed);
    let path = PathBuf::from(&expanded)
        .canonicalize()
        .map_err(|e| TuiError::InvalidArg(format!("cannot resolve {expanded}: {e}")))?;
    if !path.is_dir() {
        return Err(TuiError::InvalidArg(format!(
            "{} is not a directory",
            path.display()
        )));
    }
    Ok(path)
}

/// `~/foo` → `/Users/joao/foo`. Only the leading `~/` is expanded;
/// `~user` (other-user shorthand) is intentionally not supported.
pub fn expand_tilde(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::db::init_db;
    use httui_core::vaults::set_active_vault;
    use tempfile::TempDir;

    #[tokio::test]
    async fn opens_active_from_db() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();

        set_active_vault(&pool, &vault.path().to_string_lossy())
            .await
            .unwrap();

        let resolved = resolve(&pool).await.unwrap();
        assert_eq!(resolved.vault, vault.path().to_path_buf());
    }

    #[test]
    fn expand_tilde_replaces_only_leading() {
        std::env::set_var("HOME", "/Users/test");
        assert_eq!(expand_tilde("~/foo"), "/Users/test/foo");
        assert_eq!(expand_tilde("/abs/~/path"), "/abs/~/path");
        assert_eq!(expand_tilde("relative/path"), "relative/path");
    }
}
