#![cfg(feature = "server")]

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use nameth::NamedEnumValues as _;
use nameth::nameth;

use super::Tiles;
use crate::tiles::id::TileId;

pub static STATE: Mutex<Option<HashMap<TileId, Vec<Box<dyn Fn(TileId) + Send + Sync>>>>> =
    Mutex::new(None);

pub static TREE: Mutex<Option<Arc<Tiles>>> = Mutex::new(None);
static FILE: Mutex<Option<Arc<std::path::Path>>> = Mutex::new(None);

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum TilesStateError {
    #[error("Tile persistence failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid tile JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("[{n}] The mutex was poisoned", n = self.name())]
    PoisonError,

    #[error("[{n}] The tile {0:?} was not found", n = self.name())]
    TileIdNotFound(TileId),

    #[error("[{n}] The tile {0:?} was found twice", n = self.name())]
    DuplicateTileId(TileId),

    #[error(
        "[{n}] Cannot move tile {moved:?} into its descendant array {destination:?}",
        n = self.name()
    )]
    MoveIntoDescendant { moved: TileId, destination: TileId },
}

/// Load before serving requests; missing files start with the default layout.
pub(crate) fn initialize(path: Option<Arc<std::path::Path>>) -> Result<(), TilesStateError> {
    let mut tree = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    if let Some(path) = path.as_deref() {
        match std::fs::read(path) {
            Ok(bytes) => {
                let loaded: Arc<Tiles> = serde_json::from_slice(&bytes)?;
                reserve_ids(&loaded);
                *tree = Some(loaded);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    *FILE.lock().map_err(|_| TilesStateError::PoisonError)? = path;
    Ok(())
}

fn reserve_ids(tree: &Tiles) {
    tree.id().reserve();
    if let Tiles::Array {
        nodes,
        floating_nodes,
        ..
    } = tree
    {
        for node in nodes {
            reserve_ids(node);
        }
        for node in floating_nodes {
            reserve_ids(&node.tile);
        }
    }
}

/// Saves changed layouts before releasing the tree lock.
pub(super) struct TilesLockGuard<'a> {
    guard: MutexGuard<'a, Option<Arc<Tiles>>>,
    original: Option<Arc<Tiles>>,
    file: Option<Arc<Path>>,
}

impl TilesLockGuard<'static> {
    pub(super) fn new() -> Result<Self, TilesStateError> {
        let guard = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
        let file = FILE
            .lock()
            .map_err(|_| TilesStateError::PoisonError)?
            .clone();
        Ok(Self {
            original: guard.clone(),
            guard,
            file,
        })
    }
}

impl Deref for TilesLockGuard<'_> {
    type Target = Option<Arc<Tiles>>;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl DerefMut for TilesLockGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl Drop for TilesLockGuard<'_> {
    fn drop(&mut self) {
        let (Some(path), Some(tree)) = (self.file.as_deref(), self.guard.as_ref()) else {
            return;
        };
        if self
            .original
            .as_ref()
            .is_some_and(|original| Arc::ptr_eq(original, tree))
        {
            return;
        }
        // Drop cannot return an error; retain the in-memory layout and report failures.
        if let Err(error) = write_tree(path, tree) {
            tracing::warn!(?path, %error, "Failed to save tiles");
        }
    }
}

fn write_tree(path: &std::path::Path, tree: &Tiles) -> Result<(), TilesStateError> {
    let bytes = serde_json::to_vec_pretty(tree)?;
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "text-editor")]
    #[test]
    fn editor_path_survives_disk_round_trip() {
        use crate::text_editor::file_path::FilePath;
        let path =
            std::env::temp_dir().join(format!("terrazzo-editor-{}.json", uuid::Uuid::new_v4()));
        let editor_path = FilePath {
            base: Arc::<Path>::from(Path::new("/workspace/project")),
            file: Arc::<Path>::from(Path::new("src/main.rs")),
        };
        let tree = Tiles::Tile(super::super::Tile {
            id: TileId::new(),
            app: crate::tiles::app::App::TextEditor,
            remote: Default::default(),
            title: "Editor".into(),
            app_config: crate::tiles::app::AppConfig {
                editor_path: Some(editor_path.clone()),
            },
        });
        write_tree(&path, &tree).unwrap();
        let restored: Tiles = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let Tiles::Tile(tile) = &restored else {
            panic!("expected editor tile")
        };
        assert_eq!(tile.app_config.editor_path.as_ref(), Some(&editor_path));
        assert_eq!(tree, restored);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn guard_saves_on_drop() {
        let path =
            std::env::temp_dir().join(format!("terrazzo-guard-{}.json", uuid::Uuid::new_v4()));
        let tree = Mutex::new(None);
        {
            let mut guard = TilesLockGuard {
                guard: tree.lock().unwrap(),
                original: None,
                file: Some(path.clone().into()),
            };
            *guard = Some(Arc::new(Tiles::default()));
            assert!(!path.exists());
        }
        let restored: Tiles = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(**tree.lock().unwrap().as_ref().unwrap(), restored);
        std::fs::remove_file(&path).unwrap();
        {
            let guard = tree.lock().unwrap();
            let _guard = TilesLockGuard {
                original: guard.clone(),
                guard,
                file: Some(path.clone().into()),
            };
        }
        assert!(!path.exists(), "unchanged trees should not be saved");
    }

    #[test]
    fn disk_round_trip_and_replace() {
        let path = std::env::temp_dir().join(format!(
            "terrazzo-tiles-{}-{}.json",
            std::process::id(),
            TileId::new()
        ));
        let tree = Tiles::default();
        write_tree(&path, &tree).unwrap();
        let restored: Tiles = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(tree, restored);
        let tree = Tiles::Array {
            id: TileId::new(),
            direction: super::super::Direction::Tabbed,
            title: "Saved tabs".into(),
            selected: Some(tree.id()),
            nodes: vec![Arc::new(tree)],
            floating_nodes: vec![],
        };
        write_tree(&path, &tree).unwrap();
        let restored: Tiles = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(tree, restored);
        reserve_ids(&restored);
        assert!(i64::from(TileId::new()) > i64::from(restored.id()));
        std::fs::remove_file(path).unwrap();
    }
}
