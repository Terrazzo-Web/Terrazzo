#![cfg(feature = "server")]

use std::sync::Arc;

use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::id::TileId;

pub fn remove_node(id_to_remove: TileId) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.clone().unwrap_or_default();
    let mut maybe_id_to_remove = Some(id_to_remove);
    let tree = remove_node_aux(tree, &mut maybe_id_to_remove).unwrap_or_default();
    if let Some(id_to_remove) = maybe_id_to_remove {
        return Err(TilesStateError::TileIdNotFound(id_to_remove));
    }
    *lock = tree.clone().into();
    return Ok(tree);
}

fn remove_node_aux(
    tree: Arc<Tiles>,
    maybe_id_to_remove: &mut Option<TileId>,
) -> Option<Arc<Tiles>> {
    let Some(id_to_remove) = maybe_id_to_remove else {
        return Some(tree);
    };
    match &*tree {
        Tiles::Tile(node) if node.id == *id_to_remove => {
            *maybe_id_to_remove = None;
            None
        }
        Tiles::Array { id, .. } if *id == *id_to_remove => {
            *maybe_id_to_remove = None;
            None
        }
        Tiles::Tile { .. } => Some(tree),
        Tiles::Array {
            id,
            direction,
            nodes,
        } => {
            let nodes = nodes
                .iter()
                .filter_map(|node| remove_node_aux(node.clone(), maybe_id_to_remove))
                .collect::<Vec<_>>();
            if nodes.len() <= 1 {
                nodes.into_iter().next()
            } else {
                Some(Arc::new(Tiles::Array {
                    id: *id,
                    direction: *direction,
                    nodes,
                }))
            }
        }
    }
}
