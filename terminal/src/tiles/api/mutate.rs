#![cfg(feature = "server")]

use std::sync::Arc;

use super::Tile;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use super::transform::try_transform_first;
use crate::tiles::id::TileId;

pub fn mutate_node(
    id_to_mutate: TileId,
    mutation: impl FnOnce(&Tile) -> Tile,
) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.clone().unwrap_or_default();
    let mut maybe_mutation = Some(mutation);
    let tree = try_transform_first(tree, &mut |tree| {
        let Tiles::Tile(tile) = tree else {
            return Ok::<_, TilesStateError>(None);
        };
        if tile.id != id_to_mutate {
            return Ok(None);
        }
        let mutation = maybe_mutation
            .take()
            .ok_or(TilesStateError::DuplicateTileId(id_to_mutate))?;
        Ok(Some(Arc::new(Tiles::Tile(mutation(tile)))))
    })?
    .ok_or(TilesStateError::TileIdNotFound(id_to_mutate))?;
    *lock = tree.clone().into();
    Ok(tree)
}
