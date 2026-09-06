use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use regex::Regex;
use tracing::debug;

use super::SearchError;
use crate::text_editor::fsio::FileMetadata;

pub async fn process_path(
    base: Arc<Path>,
    path: Result<PathBuf, SearchError>,
    regex: Arc<Regex>,
) -> Result<Option<FileMetadata>, SearchError> {
    let path = path?;
    if !regex.is_match(&path.to_string_lossy()) {
        debug!("Not match: {path:?}");
        return Ok(None);
    }
    let full_path = base.join(&path);
    let Some(metadata) = tokio::fs::symlink_metadata(&full_path).await.ok() else {
        debug!("Failed to load metadata for {full_path:?}");
        return Ok(None);
    };
    debug!("Match: {full_path:?} metadata={metadata:?}");
    let result = FileMetadata::make(
        path.display().to_string().into(),
        Ok(&metadata),
        &mut HashMap::new(),
        &mut HashMap::new(),
    );
    Ok(Some(result))
}
