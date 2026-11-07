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
static PERSISTENCE: Mutex<Option<tokio::sync::mpsc::UnboundedSender<SaveCommand>>> =
    Mutex::new(None);

enum SaveCommand {
    Save(Arc<Tiles>),
    Flush(tokio::sync::oneshot::Sender<()>),
}

fn persistence_worker(path: Arc<Path>) -> tokio::sync::mpsc::UnboundedSender<SaveCommand> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(command) = rx.recv().await {
            match command {
                SaveCommand::Save(tree) => {
                    if let Err(error) = write_tree(&path, &tree).await {
                        tracing::warn!(?path, %error, "Failed to save tiles");
                    }
                }
                SaveCommand::Flush(reply) => {
                    let _ = reply.send(());
                }
            }
            tokio::task::yield_now().await;
        }
    });
    tx
}

async fn flush_writer(writer: &tokio::sync::mpsc::UnboundedSender<SaveCommand>) {
    let (reply, response) = tokio::sync::oneshot::channel();
    if writer.send(SaveCommand::Flush(reply)).is_ok() {
        let _ = response.await;
    }
}

/// Wait for queued saves before shutting down the Tokio runtime.
pub(crate) async fn flush() {
    let writer = PERSISTENCE.lock().unwrap().clone();
    if let Some(writer) = writer {
        flush_writer(&writer).await;
    }
}

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
pub(crate) async fn initialize(path: Option<Arc<std::path::Path>>) -> Result<(), TilesStateError> {
    let mut loaded_tree = None;
    if let Some(path) = path.as_deref() {
        match tokio::fs::read(path).await {
            Ok(bytes) => {
                let loaded: Arc<Tiles> = serde_json::from_slice(&bytes)?;
                reserve_ids(&loaded);
                loaded_tree = Some(loaded);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    let mut tree = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    if let Some(loaded) = loaded_tree {
        *tree = Some(loaded);
    }
    *PERSISTENCE
        .lock()
        .map_err(|_| TilesStateError::PoisonError)? = path.map(persistence_worker);
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

/// Queues changed layouts in mutation order before releasing the tree lock.
pub(super) struct TilesLockGuard<'a> {
    guard: MutexGuard<'a, Option<Arc<Tiles>>>,
    original: Option<Arc<Tiles>>,
    writer: Option<tokio::sync::mpsc::UnboundedSender<SaveCommand>>,
}

impl TilesLockGuard<'static> {
    pub(super) fn new() -> Result<Self, TilesStateError> {
        let guard = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
        let writer = PERSISTENCE
            .lock()
            .map_err(|_| TilesStateError::PoisonError)?
            .clone();
        Ok(Self {
            original: guard.clone(),
            guard,
            writer,
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
        let (Some(writer), Some(tree)) = (self.writer.as_ref(), self.guard.as_ref()) else {
            return;
        };
        if self
            .original
            .as_ref()
            .is_some_and(|original| Arc::ptr_eq(original, tree))
        {
            return;
        }
        // Enqueue while locked so newer layouts can never be overwritten by older saves.
        if writer.send(SaveCommand::Save(tree.clone())).is_err() {
            tracing::warn!("Tile persistence worker stopped");
        }
    }
}

async fn write_tree(path: &std::path::Path, tree: &Tiles) -> Result<(), TilesStateError> {
    let bytes = serde_json::to_vec_pretty(tree)?;
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        uuid::Uuid::new_v4().simple()
    ));
    tokio::fs::write(&temporary, bytes).await?;
    if let Err(error) = tokio::fs::rename(&temporary, path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "text-editor")]
    #[tokio::test]
    async fn editor_path_survives_disk_round_trip() {
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
        write_tree(&path, &tree).await.unwrap();
        let restored: Tiles =
            serde_json::from_slice(&tokio::fs::read(&path).await.unwrap()).unwrap();
        let Tiles::Tile(tile) = &restored else {
            panic!("expected editor tile")
        };
        assert_eq!(tile.app_config.editor_path.as_ref(), Some(&editor_path));
        assert_eq!(tree, restored);
        tokio::fs::remove_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn guard_queues_changed_layouts_in_order() {
        let path =
            std::env::temp_dir().join(format!("terrazzo-guard-{}.json", uuid::Uuid::new_v4()));
        let tree = Mutex::new(None);
        let writer = persistence_worker(path.clone().into());
        {
            let mut guard = TilesLockGuard {
                guard: tree.lock().unwrap(),
                original: None,
                writer: Some(writer.clone()),
            };
            *guard = Some(Arc::new(Tiles::default()));
        }
        // Queue a second mutation before waiting for either write.
        {
            let guard = tree.lock().unwrap();
            let mut guard = TilesLockGuard {
                original: guard.clone(),
                guard,
                writer: Some(writer.clone()),
            };
            *guard = Some(Arc::new(Tiles::default()));
        }
        flush_writer(&writer).await;
        let restored: Tiles =
            serde_json::from_slice(&tokio::fs::read(&path).await.unwrap()).unwrap();
        assert_eq!(**tree.lock().unwrap().as_ref().unwrap(), restored);
        tokio::fs::remove_file(&path).await.unwrap();
        {
            let guard = tree.lock().unwrap();
            let _guard = TilesLockGuard {
                original: guard.clone(),
                guard,
                writer: Some(writer.clone()),
            };
        }
        flush_writer(&writer).await;
        assert!(
            !tokio::fs::try_exists(&path).await.unwrap(),
            "unchanged trees should not be saved"
        );
    }

    #[tokio::test]
    async fn disk_round_trip_and_replace() {
        let path = std::env::temp_dir().join(format!(
            "terrazzo-tiles-{}-{}.json",
            std::process::id(),
            TileId::new()
        ));
        let tree = Tiles::default();
        write_tree(&path, &tree).await.unwrap();
        let restored: Tiles =
            serde_json::from_slice(&tokio::fs::read(&path).await.unwrap()).unwrap();
        assert_eq!(tree, restored);
        let tree = Tiles::Array {
            id: TileId::new(),
            direction: super::super::Direction::Tabbed,
            title: "Saved tabs".into(),
            selected: Some(tree.id()),
            nodes: vec![Arc::new(tree)],
            floating_nodes: vec![],
        };
        write_tree(&path, &tree).await.unwrap();
        let restored: Tiles =
            serde_json::from_slice(&tokio::fs::read(&path).await.unwrap()).unwrap();
        assert_eq!(tree, restored);
        reserve_ids(&restored);
        assert!(i64::from(TileId::new()) > i64::from(restored.id()));
        tokio::fs::remove_file(path).await.unwrap();
    }
}
