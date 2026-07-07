#![cfg(feature = "server")]

use std::sync::Arc;

use super::Direction;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use super::transform::try_transform_first;
use crate::tiles::id::TileId;

pub fn move_child(
    array_id: TileId,
    after_child: Option<TileId>,
    moved_child: TileId,
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
        if *id != array_id || *direction != Direction::Tabbed {
            return Ok(None);
        }
        let Some(from) = nodes.iter().position(|node| node.id() == moved_child) else {
            return Err(TilesStateError::TileIdNotFound(moved_child));
        };
        let moved_node = nodes[from].clone();
        let mut nodes = nodes.clone();
        nodes.remove(from);
        let insert_at = after_child
            .and_then(|after_child| {
                nodes
                    .iter()
                    .position(|node| node.id() == after_child)
                    .map(|index| index + 1)
            })
            .unwrap_or(0);
        nodes.insert(insert_at, moved_node);
        Ok(Some(Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            title: title.clone(),
            selected: selected.or(Some(moved_child)),
            nodes,
            floating_nodes: floating_nodes.clone(),
        })))
    })?
    .ok_or(TilesStateError::TileIdNotFound(array_id))?;
    *lock = tree.clone().into();
    Ok(tree)
}
