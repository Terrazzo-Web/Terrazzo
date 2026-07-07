#![cfg(feature = "server")]

use std::sync::Arc;

use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use super::transform::try_transform_first;
use crate::tiles::id::TileId;

pub fn set_tab_title(array_id: TileId, title: String) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.clone().unwrap_or_default();
    let tree = try_transform_first(tree, &mut |tree| {
        let Tiles::Array {
            id,
            direction,
            selected,
            nodes,
            floating_nodes,
            ..
        } = tree
        else {
            return Ok::<_, TilesStateError>(None);
        };
        if *id != array_id {
            return Ok(None);
        }
        Ok(Some(Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            title: title.clone().into(),
            selected: *selected,
            nodes: nodes.clone(),
            floating_nodes: floating_nodes.clone(),
        })))
    })?
    .ok_or(TilesStateError::TileIdNotFound(array_id))?;
    *lock = tree.clone().into();
    Ok(tree)
}
