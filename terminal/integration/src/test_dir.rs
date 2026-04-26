use std::path::PathBuf;

use crate::RunError;

const TEST_TMPDIR_ENV: &str = "TEST_TMPDIR";

pub fn test_dir() -> Result<tempfile::TempDir, RunError> {
    let base = std::env::var_os(TEST_TMPDIR_ENV).map(PathBuf::from);
    let mut builder = tempfile::Builder::new();
    builder.prefix("terrazzo-integration-test-");
    if let Some(base) = &base {
        builder.tempdir_in(base)
    } else {
        builder.tempdir()
    }
    .map_err(|source| RunError::CreateTestDir { base, source })
}
