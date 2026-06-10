use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use nameth::nameth;
use nameth::NamedEnumValues as _;

use super::Tiles;
use crate::tiles::id::TileId;

pub static STATE: Mutex<Option<HashMap<TileId, Vec<Box<dyn Fn(TileId) + Send + Sync>>>>> =
    Mutex::new(None);

pub static TREE: Mutex<Option<Arc<Tiles>>> = Mutex::new(None);

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum TilesStateError {
    #[error("[{n}] The mutex was poisoned", n = self.name())]
    PoisonError,

    #[error("[{n}] The tile {0:?} was not found", n = self.name())]
    TileIdNotFound(TileId),

    #[error("[{n}] The tile {0:?} was found twice", n = self.name())]
    DuplicateTileId(TileId),
}
