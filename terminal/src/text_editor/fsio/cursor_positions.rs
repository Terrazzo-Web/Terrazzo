#![cfg(feature = "server")]

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use super::CursorPosition;
use crate::text_editor::file_path::FilePath;

static CURSOR_POSITIONS: LazyLock<Mutex<HashMap<CursorPositionKey, CursorPosition>>> =
    LazyLock::new(Mutex::default);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CursorPositionKey {
    path: PathBuf,
}

impl CursorPositionKey {
    fn new(path: &FilePath<Arc<Path>>) -> Self {
        Self {
            path: path.full_path(),
        }
    }
}

pub fn load(path: FilePath<Arc<Path>>) -> Option<CursorPosition> {
    CURSOR_POSITIONS
        .lock()
        .expect("cursor_positions")
        .get(&CursorPositionKey::new(&path))
        .copied()
}

pub fn store(path: FilePath<Arc<Path>>, position: CursorPosition) {
    let _ = CURSOR_POSITIONS
        .lock()
        .expect("cursor_positions")
        .insert(CursorPositionKey::new(&path), position);
}
