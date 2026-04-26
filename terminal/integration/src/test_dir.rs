use std::path::PathBuf;

use crate::RunError;

pub const TEST_TMPDIR_ENV: &str = "TEST_TMPDIR";
pub const TEST_DIR_NAME: &str = "terrazzo-integration-test";

pub fn test_dir() -> Result<PathBuf, RunError> {
    let base = std::env::var_os(TEST_TMPDIR_ENV).map(PathBuf::from);
    let path = base
        .clone()
        .unwrap_or_else(std::env::temp_dir)
        .join(TEST_DIR_NAME);
    if path.exists() {
        std::fs::remove_dir_all(&path).map_err(|source| RunError::CreateTestDir {
            base: base.clone(),
            source,
        })?;
    }
    std::fs::create_dir_all(&path).map_err(|source| RunError::CreateTestDir { base, source })?;
    Ok(path)
}
