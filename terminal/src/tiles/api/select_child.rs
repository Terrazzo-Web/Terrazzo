#![cfg(feature = "server")]

use std::sync::Arc;

use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use super::transform::try_transform_first;
use crate::tiles::id::TileId;

pub fn select_child(
    array_id: TileId,
    selected_child: Option<TileId>,
) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.clone().unwrap_or_default();
    let tree = try_transform_first(tree, &mut |tree| {
        let Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
            floating_nodes,
        } = tree
        else {
            return Ok(None);
        };
        if *id != array_id {
            return Ok(None);
        }
        if let Some(selected_child) = selected_child
            && !nodes.iter().any(|node| node.id() == selected_child)
        {
            return Err(TilesStateError::TileIdNotFound(selected_child));
        }
        Ok(Some(Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            title: title.clone(),
            selected: selected_child.or(*selected),
            nodes: nodes.clone(),
            floating_nodes: floating_nodes.clone(),
        })))
    })?
    .ok_or(TilesStateError::TileIdNotFound(array_id))?;
    *lock = tree.clone().into();
    Ok(tree)
}
