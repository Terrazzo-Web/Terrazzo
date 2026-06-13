#![cfg(feature = "server")]

use std::sync::Arc;

use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::id::TileId;

pub fn add_tab(
    array_id: TileId,
    after_child: Option<TileId>,
) -> Result<(Arc<Tiles>, TileId), TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let new_id = TileId::new();
    let mut inserted = false;
    let tree = add_tab_aux(tree, array_id, after_child, new_id, &mut inserted)?;
    if !inserted {
        return Err(TilesStateError::TileIdNotFound(array_id));
    }
    *lock = tree.clone().into();
    Ok((tree, new_id))
}

fn add_tab_aux(
    tree: Arc<Tiles>,
    array_id: TileId,
    after_child: Option<TileId>,
    new_id: TileId,
    inserted: &mut bool,
) -> Result<Arc<Tiles>, TilesStateError> {
    if *inserted {
        return Ok(tree);
    }
    Ok(match &*tree {
        Tiles::Tile { .. } => tree,
        Tiles::Array {
            id,
            direction,
            selected,
            nodes,
        } => Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            selected: *selected,
            nodes: nodes
                .iter()
                .map(|node| add_tab_aux(node.clone(), array_id, after_child, new_id, inserted))
                .collect::<Result<_, _>>()?,
        }),
    })
}
