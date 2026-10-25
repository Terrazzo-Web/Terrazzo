#![cfg(feature = "server")]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use nameth::NamedEnumValues as _;
use nameth::nameth;

use super::Tiles;
use crate::tiles::id::TileId;

pub static STATE: Mutex<Option<HashMap<TileId, Vec<Box<dyn Fn(TileId) + Send + Sync>>>>> =
    Mutex::new(None);

pub static TREE: Mutex<Option<Arc<Tiles>>> = Mutex::new(None);

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

static FILE: Mutex<Option<Arc<std::path::Path>>> = Mutex::new(None);

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

/// Called with TREE locked, keeping disk writes in mutation order.
pub(super) fn save(tree: &Tiles) -> Result<(), TilesStateError> {
    let file = FILE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let Some(path) = file.as_deref() else {
        return Ok(());
    };
    write_tree(path, tree)
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
