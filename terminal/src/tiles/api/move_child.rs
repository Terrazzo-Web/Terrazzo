#![cfg(feature = "server")]

use std::sync::Arc;

use super::Direction;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::id::TileId;

pub fn move_child(
    array_id: TileId,
    after_child: Option<TileId>,
    moved_child: TileId,
) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let mut moved = false;
    let tree = move_child_aux(tree, array_id, after_child, moved_child, &mut moved)?;
    if !moved {
        return Err(TilesStateError::TileIdNotFound(array_id));
    }
    *lock = tree.clone().into();
    Ok(tree)
}

fn move_child_aux(
    tree: Arc<Tiles>,
    array_id: TileId,
    after_child: Option<TileId>,
    moved_child: TileId,
    moved: &mut bool,
) -> Result<Arc<Tiles>, TilesStateError> {
    if *moved {
        return Ok(tree);
    }
    Ok(match &*tree {
        Tiles::Tile { .. } => tree,
        Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
        } if *id == array_id && *direction == Direction::Tabbed => {
            let Some(from) = nodes.iter().position(|node| child_id(node) == moved_child) else {
                return Err(TilesStateError::TileIdNotFound(moved_child));
            };
            let moved_node = nodes[from].clone();
            let mut nodes = nodes.clone();
            nodes.remove(from);
            let insert_at = after_child
                .and_then(|after_child| {
                    nodes
                        .iter()
                        .position(|node| child_id(node) == after_child)
                        .map(|index| index + 1)
                })
                .unwrap_or(0);
            nodes.insert(insert_at, moved_node);
            *moved = true;
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: selected.or(Some(moved_child)),
                nodes,
            })
        }
        Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
        } => Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            title: title.clone(),
            selected: *selected,
            nodes: nodes
                .iter()
                .map(|node| move_child_aux(node.clone(), array_id, after_child, moved_child, moved))
                .collect::<Result<_, _>>()?,
        }),
    })
}

fn child_id(node: &Tiles) -> TileId {
    match node {
        Tiles::Tile(tile) => tile.id,
        Tiles::Array { id, .. } => *id,
    }
}
