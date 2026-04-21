use anyhow::{Context, Result};
use std::path::Path;
use uuid::Uuid;

const FILENAME: &str = "user_id.txt";

/// Load the stable per-install user id, generating one if missing.
///
/// Shared-DB multi-user support depends on this. Each install has its own
/// uuid stored locally outside the database so that it survives wiping the
/// DB, and two installs pointing at the same Postgres never share it.
pub fn load_or_create(app_data_dir: &Path) -> Result<String> {
    let path = app_data_dir.join(FILENAME);
    if path.exists() {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    std::fs::create_dir_all(app_data_dir).ok();
    let id = Uuid::new_v4().to_string();
    std::fs::write(&path, &id)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(id)
}
